//! Minimal SHA-256 implementation with no external dependencies.

const BLOCK_SIZE: usize = 64;
const STATE_WORDS: usize = 8;
const WORKSPACE_WORDS: usize = 64;

const INITIAL_STATE: [u32; STATE_WORDS] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

const ROUND_CONSTANTS: [u32; WORKSPACE_WORDS] = [
    0x428A_2F98,
    0x7137_4491,
    0xB5C0_FBCF,
    0xE9B5_DBA5,
    0x3956_C25B,
    0x59F1_11F1,
    0x923F_82A4,
    0xAB1C_5ED5,
    0xD807_AA98,
    0x1283_5B01,
    0x2431_85BE,
    0x550C_7DC3,
    0x72BE_5D74,
    0x80DE_B1FE,
    0x9BDC_06A7,
    0xC19B_F174,
    0xE49B_69C1,
    0xEFBE_4786,
    0x0FC1_9DC6,
    0x240C_A1CC,
    0x2DE9_2C6F,
    0x4A74_84AA,
    0x5CB0_A9DC,
    0x76F9_88DA,
    0x983E_5152,
    0xA831_C66D,
    0xB003_27C8,
    0xBF59_7FC7,
    0xC6E0_0BF3,
    0xD5A7_9147,
    0x06CA_6351,
    0x1429_2967,
    0x27B7_0A85,
    0x2E1B_2138,
    0x4D2C_6DFC,
    0x5338_0D13,
    0x650A_7354,
    0x766A_0ABB,
    0x81C2_C92E,
    0x9272_2C85,
    0xA2BF_E8A1,
    0xA81A_664B,
    0xC24B_8B70,
    0xC76C_51A3,
    0xD192_E819,
    0xD699_0624,
    0xF40E_3585,
    0x106A_A070,
    0x19A4_C116,
    0x1E37_6C08,
    0x2748_774C,
    0x34B0_BCB5,
    0x391C_0CB3,
    0x4ED8_AA4A,
    0x5B9C_CA4F,
    0x682E_6FF3,
    0x748F_82EE,
    0x78A5_636F,
    0x84C8_7814,
    0x8CC7_0208,
    0x90BE_FFFA,
    0xA450_6CEB,
    0xBEF9_A3F7,
    0xC671_78F2,
];

#[derive(Clone)]
pub struct Sha256 {
    state: [u32; STATE_WORDS],
    buffer: [u8; BLOCK_SIZE],
    buffer_len: usize,
    bit_len: u64,
}

impl Sha256 {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: INITIAL_STATE,
            buffer: [0u8; BLOCK_SIZE],
            buffer_len: 0,
            bit_len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        self.bit_len = self.bit_len.wrapping_add((data.len() as u64) * 8);

        let mut remaining = data;
        while !remaining.is_empty() {
            let space = BLOCK_SIZE - self.buffer_len;
            let take = space.min(remaining.len());
            let (head, tail) = remaining.split_at(take);
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(head);
            self.buffer_len += take;
            remaining = tail;

            if self.buffer_len == BLOCK_SIZE {
                process_block(&mut self.state, &self.buffer);
                self.buffer_len = 0;
            }
        }
    }

    #[must_use]
    pub fn finalize(mut self) -> [u8; 32] {
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > BLOCK_SIZE - 8 {
            for byte in &mut self.buffer[self.buffer_len..] {
                *byte = 0;
            }
            process_block(&mut self.state, &self.buffer);
            self.buffer = [0u8; BLOCK_SIZE];
            self.buffer_len = 0;
        }

        for byte in &mut self.buffer[self.buffer_len..BLOCK_SIZE - 8] {
            *byte = 0;
        }

        self.buffer[BLOCK_SIZE - 8..].copy_from_slice(&self.bit_len.to_be_bytes());
        process_block(&mut self.state, &self.buffer);

        let mut out = [0u8; 32];
        for (chunk, value) in out.chunks_exact_mut(4).zip(self.state.iter()) {
            chunk.copy_from_slice(&value.to_be_bytes());
        }
        out
    }

    #[must_use]
    pub fn digest(data: &[u8]) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data);
        hasher.finalize()
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(always)]
fn small_sigma0(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

#[inline(always)]
fn small_sigma1(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
}

#[inline(always)]
fn big_sigma0(x: u32) -> u32 {
    x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
}

#[inline(always)]
fn big_sigma1(x: u32) -> u32 {
    x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
}

#[inline(always)]
fn choice(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

#[inline(always)]
fn majority(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

fn process_block(state: &mut [u32; STATE_WORDS], block: &[u8; BLOCK_SIZE]) {
    let mut w = [0u32; WORKSPACE_WORDS];
    for (idx, chunk) in block.chunks_exact(4).enumerate() {
        w[idx] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }

    for t in 16..WORKSPACE_WORDS {
        let s0 = small_sigma0(w[t - 15]);
        let s1 = small_sigma1(w[t - 2]);
        w[t] = w[t - 16]
            .wrapping_add(s0)
            .wrapping_add(w[t - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for t in 0..WORKSPACE_WORDS {
        let t1 = h
            .wrapping_add(big_sigma1(e))
            .wrapping_add(choice(e, f, g))
            .wrapping_add(ROUND_CONSTANTS[t])
            .wrapping_add(w[t]);
        let t2 = big_sigma0(a).wrapping_add(majority(a, b, c));

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn digest_empty() {
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(hex(&Sha256::digest(b"")), expected);
    }

    #[test]
    fn digest_abc() {
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(hex(&Sha256::digest(b"abc")), expected);
    }

    #[test]
    fn digest_longer_message() {
        let message = b"The quick brown fox jumps over the lazy dog";
        let expected = "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592";
        assert_eq!(hex(&Sha256::digest(message)), expected);
    }

    #[test]
    fn incremental_vs_single_shot() {
        let mut hasher = Sha256::new();
        hasher.update(b"hello ");
        hasher.update(b"world");
        let incremental = hasher.finalize();
        let single = Sha256::digest(b"hello world");
        assert_eq!(incremental, single);
    }
}
