//! Poly1305 one-time MAC per RFC 8439.
//!
//! Clean implementation using straightforward 130-bit arithmetic.

/// Compute Poly1305 MAC tag.
pub fn poly1305_tag(msg: &[u8], key: &[u8; 32]) -> [u8; 16] {
    // Parse and clamp r
    let mut r_bytes = [0u8; 16];
    r_bytes.copy_from_slice(&key[0..16]);
    r_bytes[3] &= 15;
    r_bytes[7] &= 15;
    r_bytes[11] &= 15;
    r_bytes[15] &= 15;
    r_bytes[4] &= 252;
    r_bytes[8] &= 252;
    r_bytes[12] &= 252;
    let r = u128::from_le_bytes(r_bytes);

    // Parse s
    let s = u128::from_le_bytes(key[16..32].try_into().unwrap());

    // Process message blocks
    let mut accumulator = U130::zero();

    for chunk in msg.chunks(16) {
        // Create block with padding
        let mut block_bytes = [0u8; 17];
        block_bytes[..chunk.len()].copy_from_slice(chunk);
        block_bytes[chunk.len()] = 1;

        // Convert to U130
        let block = U130::from_bytes(&block_bytes);

        // accumulator = (accumulator + block) * r mod P
        accumulator = accumulator.add(block).mul_mod_p(r);
    }

    // Add s and return lower 128 bits
    accumulator.add_u128(s).to_bytes()
}

// 130-bit unsigned integer (for poly1305 arithmetic)
#[derive(Clone, Copy, Debug)]
struct U130 {
    lo: u128, // bits 0-127
    hi: u8,   // bits 128-129 (only 2 bits used)
}

impl U130 {
    const fn zero() -> Self {
        U130 { lo: 0, hi: 0 }
    }

    fn from_bytes(bytes: &[u8; 17]) -> Self {
        let lo = u128::from_le_bytes(bytes[0..16].try_into().unwrap());
        let hi = bytes[16] & 3; // Only use 2 bits
        U130 { lo, hi }
    }

    fn add(self, other: U130) -> U130 {
        let (lo, carry) = self.lo.overflowing_add(other.lo);
        let hi = self.hi + other.hi + (carry as u8);
        U130 { lo, hi }
    }

    fn add_u128(self, val: u128) -> U130 {
        let (lo, carry) = self.lo.overflowing_add(val);
        let hi = self.hi + (carry as u8);
        U130 { lo, hi }
    }

    // Multiply by u128 and reduce modulo P = 2^130 - 5
    fn mul_mod_p(self, r: u128) -> U130 {
        // Multiply self * r
        // self = hi * 2^128 + lo
        // self * r = hi * 2^128 * r + lo * r

        // Compute lo * r (gives up to 256 bits)
        let (prod_lo, prod_hi) = mul_u128(self.lo, r);

        // Compute hi * r (this is small, hi is at most 3)
        let hi_contrib = (self.hi as u128) * r;

        // Now we have: prod_lo (128 bits) + prod_hi * 2^128 + hi_contrib * 2^128
        // = prod_lo + (prod_hi + hi_contrib) * 2^128

        let high = prod_hi + hi_contrib;

        // Reduce modulo 2^130 - 5
        // high contains bits from position 128 onward
        // Bits 128-129 stay, bits 130+ get multiplied by 5

        let hi = (high & 3) as u8; // Bits 128-129
        let overflow = high >> 2; // Bits 130+

        // overflow * 2^130 ≡ overflow * 5 (mod 2^130 - 5)
        let correction = overflow.wrapping_mul(5);

        let (lo, carry) = prod_lo.overflowing_add(correction);
        let hi = hi + (carry as u8);

        // Final reduction if hi >= 4
        if hi >= 4 {
            let extra = ((hi >> 2) as u128) * 5;
            let lo = lo.wrapping_add(extra);
            U130 { lo, hi: hi & 3 }
        } else {
            U130 { lo, hi }
        }
    }

    fn to_bytes(self) -> [u8; 16] {
        // Final reduction: if value >= 2^130 - 5, subtract (2^130 - 5)
        // which is equivalent to: if (value + 5) >= 2^130, use (value + 5) & (2^130-1)

        // Try adding 5
        let (test_lo, carry) = self.lo.overflowing_add(5);
        let test_hi = self.hi.wrapping_add(carry as u8);

        // If test_hi >= 4, overflow happened, meaning self + 5 >= 2^130
        // In this case, self >= 2^130 - 5, so we should use the reduced value
        // The reduced value is (self + 5) mod 2^130 = test_lo with test_hi &3
        let should_reduce = test_hi >= 4;

        let result = if should_reduce {
            // Use the sum (which wraps at 2^130)
            test_lo
        } else {
            // Original value is fine
            self.lo
        };

        result.to_le_bytes()
    }
}

// Multiply two u128 values, returning (low 128 bits, high 128 bits)
fn mul_u128(a: u128, b: u128) -> (u128, u128) {
    // Split into 64-bit parts
    let a_lo = a as u64 as u128;
    let a_hi = (a >> 64) as u64 as u128;
    let b_lo = b as u64 as u128;
    let b_hi = (b >> 64) as u64 as u128;

    // Compute partial products
    let p00 = a_lo * b_lo;
    let p01 = a_lo * b_hi;
    let p10 = a_hi * b_lo;
    let p11 = a_hi * b_hi;

    // Combine: result = p00 + (p01 << 64) + (p10 << 64) + (p11 << 128)
    let mid = p01 + p10;
    let (lo, carry) = p00.overflowing_add(mid << 64);
    let hi = p11 + (mid >> 64) + (carry as u128);

    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn rfc_8439_vector() {
        // RFC 8439 Section 2.5.2 test vector
        let key = [
            0x85, 0xd6, 0xbe, 0x78, 0x57, 0x55, 0x6d, 0x33, 0x7f, 0x44, 0x52, 0xfe, 0x42, 0xd5,
            0x06, 0xa8, 0x01, 0x03, 0x80, 0x8a, 0xfb, 0x0d, 0xb2, 0xfd, 0x4a, 0xbf, 0xf6, 0xaf,
            0x41, 0x49, 0xf5, 0x1b,
        ];
        let msg = b"Cryptographic Forum Research Group";
        let tag = poly1305_tag(msg, &key);
        let expected = "a8061dc1305136c6c22b8baf0c0127a9";
        assert_eq!(hex(&tag), expected);
    }

    #[test]
    fn empty_message_returns_s() {
        let key = [0u8; 32];
        let tag = poly1305_tag(&[], &key);
        assert_eq!(&tag, &key[16..]);
    }
}
