//! Minimal `ChaCha20` implementation supporting the IETF 96-bit nonce variant.

const CONSTANTS: [u32; 4] = [
    0x6170_7865, // "expa"
    0x3320_646e, // "nd 3"
    0x7962_2d32, // "2-by"
    0x6b20_6574, // "te k"
];

#[inline]
fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(16);

    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(12);

    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(8);

    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(7);
}

fn chacha20_rounds(state: &mut [u32; 16]) {
    for _ in 0..10 {
        // Column rounds
        quarter_round(state, 0, 4, 8, 12);
        quarter_round(state, 1, 5, 9, 13);
        quarter_round(state, 2, 6, 10, 14);
        quarter_round(state, 3, 7, 11, 15);
        // Diagonal rounds
        quarter_round(state, 0, 5, 10, 15);
        quarter_round(state, 1, 6, 11, 12);
        quarter_round(state, 2, 7, 8, 13);
        quarter_round(state, 3, 4, 9, 14);
    }
}

fn initialize_state(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u32; 16] {
    let mut state = [0u32; 16];

    state[..4].copy_from_slice(&CONSTANTS);

    for (idx, chunk) in key.chunks_exact(4).enumerate() {
        state[4 + idx] = u32::from_le_bytes(chunk.try_into().unwrap());
    }

    state[12] = counter;
    state[13] = u32::from_le_bytes(nonce[0..4].try_into().unwrap());
    state[14] = u32::from_le_bytes(nonce[4..8].try_into().unwrap());
    state[15] = u32::from_le_bytes(nonce[8..12].try_into().unwrap());

    state
}

pub fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut working_state = initialize_state(key, counter, nonce);
    let initial_state = working_state;
    chacha20_rounds(&mut working_state);

    for idx in 0..16 {
        working_state[idx] = working_state[idx].wrapping_add(initial_state[idx]);
    }

    let mut block = [0u8; 64];
    for (idx, chunk) in block.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&working_state[idx].to_le_bytes());
    }

    block
}

pub fn chacha20_xor(key: &[u8; 32], counter: u32, nonce: &[u8; 12], data: &mut [u8]) {
    let mut block_counter = counter;
    let mut offset = 0;

    while offset < data.len() {
        let block = chacha20_block(key, block_counter, nonce);
        block_counter = block_counter.wrapping_add(1);

        let take = (data.len() - offset).min(64);
        for (dst, src) in data[offset..offset + take]
            .iter_mut()
            .zip(block.iter().take(take))
        {
            *dst ^= src;
        }
        offset += take;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn test_block_output() {
        let key = [0u8; 32];
        let nonce = [0u8; 12];
        let block = chacha20_block(&key, 0, &nonce);
        let expected = "
            76b8e0ada0f13d90405d6ae55386bd28
            bdd219b8a08ded1aa836efcc8b770dc7
            da41597c5157488d7724e03fb8d84a37
            6a43b8f41518a11cc387b669b2ee6586";
        assert_eq!(hex(&block), expected.split_whitespace().collect::<String>());
    }

    #[test]
    fn test_xor_stream() {
        let key = [0u8; 32];
        let nonce = [0u8; 12];
        let mut data = [0u8; 64];
        chacha20_xor(&key, 0, &nonce, &mut data);
        let block = chacha20_block(&key, 0, &nonce);
        assert_eq!(data.to_vec(), block.to_vec());
    }

    #[test]
    fn rfc_8439_keystream_block1() {
        let key = [
            0x1c, 0x92, 0x40, 0xa5, 0xeb, 0x55, 0xd3, 0x8a, 0xf3, 0x33, 0x88, 0x86, 0x04, 0xf6,
            0xb5, 0xf0, 0x47, 0x39, 0x17, 0xc1, 0x40, 0x2b, 0x80, 0x09, 0x9d, 0xca, 0x5c, 0xbc,
            0x20, 0x70, 0x75, 0xc0,
        ];
        let nonce = [
            0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        ];
        let expected = "
            2dcef27007e87f804db410a6fd92309d
            3ff239dd502e92ef85280365a419e1d7
            22188f38033c2f87c4be8f214a0d4790
            d0d6cf555f6e6c20bb961b0c5c20b4a2";
        let ks = chacha20_block(&key, 1, &nonce);
        assert_eq!(
            hex(&ks[..64]),
            expected.split_whitespace().collect::<String>()
        );
    }
}
