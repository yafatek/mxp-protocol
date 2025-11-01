//! ChaCha20-Poly1305 AEAD per RFC 8439 using the local primitives.

use super::chacha20::{chacha20_block, chacha20_xor};
use super::poly1305::poly1305_tag;
use super::{AeadKey, AeadNonce, AeadTag, CryptoError};

fn poly_key(key: &AeadKey, nonce: &AeadNonce) -> [u8; 32] {
    let block = chacha20_block(key.as_bytes(), 0, nonce.as_bytes());
    let mut poly = [0u8; 32];
    poly.copy_from_slice(&block[..32]);
    poly
}

fn compute_mac(poly_key: &[u8; 32], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let mut mac_data =
        Vec::with_capacity(aad.len().div_ceil(16) * 16 + ciphertext.len().div_ceil(16) * 16 + 16);

    mac_data.extend_from_slice(aad);
    if aad.len() % 16 != 0 {
        mac_data.resize(aad.len().div_ceil(16) * 16, 0);
    }

    mac_data.extend_from_slice(ciphertext);
    if ciphertext.len() % 16 != 0 {
        mac_data.resize(mac_data.len() + (16 - (ciphertext.len() % 16)) % 16, 0);
    }

    mac_data.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    mac_data.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());

    poly1305_tag(&mac_data, poly_key)
}

pub fn seal(key: &AeadKey, nonce: &AeadNonce, plaintext: &[u8], aad: &[u8]) -> (Vec<u8>, AeadTag) {
    let poly = poly_key(key, nonce);

    let mut ciphertext = plaintext.to_vec();
    chacha20_xor(key.as_bytes(), 1, nonce.as_bytes(), &mut ciphertext);

    let tag_bytes = compute_mac(&poly, aad, &ciphertext);
    (ciphertext, AeadTag::from_array(tag_bytes))
}

pub fn open(
    key: &AeadKey,
    nonce: &AeadNonce,
    ciphertext: &[u8],
    aad: &[u8],
    tag: &AeadTag,
) -> Result<Vec<u8>, CryptoError> {
    let poly = poly_key(key, nonce);
    let expected = compute_mac(&poly, aad, ciphertext);

    let mut diff = 0u8;
    for (a, b) in expected.iter().zip(tag.as_bytes()) {
        diff |= a ^ b;
    }
    if diff != 0 {
        return Err(CryptoError::AuthenticationFailed);
    }

    let mut plaintext = ciphertext.to_vec();
    chacha20_xor(key.as_bytes(), 1, nonce.as_bytes(), &mut plaintext);
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc_8439_seal_open() {
        let key = AeadKey::from_array([
            0x1c, 0x92, 0x40, 0xa5, 0xeb, 0x55, 0xd3, 0x8a, 0xf3, 0x33, 0x88, 0x86, 0x04, 0xf6,
            0xb5, 0xf0, 0x47, 0x39, 0x17, 0xc1, 0x40, 0x2b, 0x80, 0x09, 0x9d, 0xca, 0x5c, 0xbc,
            0x20, 0x70, 0x75, 0xc0,
        ]);
        let nonce = AeadNonce::from_array([
            0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        ]);
        let aad = [
            0xf3, 0x33, 0x88, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4e, 0x91,
        ];
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";

        let (cipher, tag) = seal(&key, &nonce, plaintext, &aad);
        let expected_cipher = "
            61af9619629b5fe123d030e198fc44f1
            5a9f5cb37041f4cff1406645c77580a4
            5138e05e231b16befe9ec6476a4467f3
            bfa3a3317f010a46dee43b75335594cd
            322d8e466d4593808881b50ba24484c4
            40b0022a078c22ac3ebcdcf4fc2ec745
            9181e92bf5a2b861d69939022c244335
            624a";
        let expected_tag = "734ea95abc2315836b07bd2d2d52b12b";

        assert_eq!(
            hex(&cipher),
            expected_cipher.split_whitespace().collect::<String>()
        );
        assert_eq!(hex(tag.as_bytes()), expected_tag);

        let opened = open(&key, &nonce, &cipher, &aad, &tag).expect("decrypt");
        assert_eq!(opened.as_slice(), plaintext);

        let mut tampered = cipher.clone();
        tampered[0] ^= 0x01;
        let err = open(&key, &nonce, &tampered, &aad, &tag).unwrap_err();
        assert!(matches!(err, CryptoError::AuthenticationFailed));
    }
}
