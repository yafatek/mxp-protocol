//! Deterministic mixing helpers used by placeholder cryptographic routines.

/// XOR the destination slice with the source, repeating the source as needed, and
/// applying an optional tweak.
pub fn xor_cycle(dst: &mut [u8], src: &[u8], tweak: u8) {
    if src.is_empty() {
        return;
    }

    for (idx, byte) in dst.iter_mut().enumerate() {
        let src_byte = src[idx % src.len()];
        *byte ^= src_byte ^ tweak;
    }
}

/// Rotate left and add source bytes into the destination slice with a constant tweak.
pub fn rotate_add(dst: &mut [u8], src: &[u8], tweak: u8) {
    if src.is_empty() {
        return;
    }

    for (idx, byte) in dst.iter_mut().enumerate() {
        let src_byte = src[idx % src.len()];
        *byte = src_byte
            .wrapping_add(tweak)
            .rotate_left(((idx & 7) + 1) as u32);
    }
}

fn ordered<'a>(a: &'a [u8], b: &'a [u8]) -> (&'a [u8], &'a [u8]) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Combine several byte slices into a symmetric folded key material.
pub fn symmetric_fold(
    chaining_key: &[u8],
    temp_key: &[u8],
    local_static: &[u8],
    remote_static: &[u8],
    local_ephemeral: &[u8],
    remote_ephemeral: &[u8],
) -> [u8; super::AEAD_KEY_LEN] {
    let mut out = [0u8; super::AEAD_KEY_LEN];

    let (stat_a, stat_b) = ordered(local_static, remote_static);
    let (eph_a, eph_b) = ordered(local_ephemeral, remote_ephemeral);

    for idx in 0..out.len() {
        let stat = stat_a[idx % stat_a.len()] ^ stat_b[idx % stat_b.len()];
        let eph = eph_a[idx % eph_a.len()] ^ eph_b[idx % eph_b.len()];
        let tweak = ((idx as u8).wrapping_mul(37)) ^ ((idx as u8).rotate_left(4));
        let chain = chaining_key[idx % chaining_key.len()];
        let temp = temp_key[idx % temp_key.len()];

        out[idx] = (stat ^ eph ^ tweak ^ chain ^ temp)
            .rotate_left(((idx & 7) + 1) as u32);
    }

    out
}

