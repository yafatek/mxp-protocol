//! HMAC-SHA256 implementation built on the in-house SHA-256.

use super::sha256::Sha256;

const BLOCK_SIZE: usize = 64;

#[derive(Clone)]
pub struct HmacSha256 {
    inner: Sha256,
    outer: Sha256,
}

impl HmacSha256 {
    #[must_use]
    pub fn new(key: &[u8]) -> Self {
        let mut key_block = [0u8; BLOCK_SIZE];

        if key.len() > BLOCK_SIZE {
            let hashed = Sha256::digest(key);
            key_block[..hashed.len()].copy_from_slice(&hashed);
        } else {
            key_block[..key.len()].copy_from_slice(key);
        }

        let mut o_key_pad = [0u8; BLOCK_SIZE];
        let mut i_key_pad = [0u8; BLOCK_SIZE];
        for (idx, byte) in key_block.iter().enumerate() {
            o_key_pad[idx] = byte ^ 0x5c;
            i_key_pad[idx] = byte ^ 0x36;
        }

        let mut inner = Sha256::new();
        inner.update(&i_key_pad);

        let mut outer = Sha256::new();
        outer.update(&o_key_pad);

        Self { inner, outer }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    #[must_use]
    pub fn finalize(mut self) -> [u8; 32] {
        let inner_hash = self.inner.finalize();
        self.outer.update(&inner_hash);
        self.outer.finalize()
    }

    #[must_use]
    pub fn compute(key: &[u8], data: &[u8]) -> [u8; 32] {
        let mut hmac = Self::new(key);
        hmac.update(data);
        hmac.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc_4231_case_1() {
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        assert_eq!(hex(&HmacSha256::compute(&key, data)), expected);
    }

    #[test]
    fn rfc_4231_case_2() {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let expected = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
        assert_eq!(hex(&HmacSha256::compute(key, data)), expected);
    }

    #[test]
    fn rfc_4231_case_3() {
        let key = [0xAau8; 20];
        let data = [0xDdu8; 50];
        let expected = "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe";
        assert_eq!(hex(&HmacSha256::compute(&key, &data)), expected);
    }
}
