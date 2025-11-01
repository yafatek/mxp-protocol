//! Cryptographic primitives for MXP transport (Noise IK handshake, key schedule, AEAD).

/// Length of public keys (X25519) in bytes.
pub const PUBLIC_KEY_LEN: usize = 32;
/// Length of private keys (X25519) in bytes.
pub const PRIVATE_KEY_LEN: usize = 32;
/// Length of shared secrets (X25519) in bytes.
pub const SHARED_SECRET_LEN: usize = 32;
/// Length of AEAD keys (ChaCha20-Poly1305) in bytes.
pub const AEAD_KEY_LEN: usize = 32;
/// Length of AEAD nonces in bytes.
pub const AEAD_NONCE_LEN: usize = 12;
/// Length of AEAD authentication tags in bytes.
pub const AEAD_TAG_LEN: usize = 16;

/// Error type for cryptographic operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// Key material of unexpected length.
    InvalidKeyLength,
    /// Nonce value has invalid length.
    InvalidNonceLength,
    /// Authentication tag has invalid length.
    InvalidTagLength,
    /// Authentication failure during decryption.
    AuthenticationFailed,
    /// HKDF expansion failure.
    KeyDerivationFailed,
}

mod mixing;

fn copy_checked<const N: usize>(bytes: &[u8], on_err: CryptoError) -> Result<[u8; N], CryptoError> {
    if bytes.len() != N {
        return Err(on_err);
    }
    let mut array = [0u8; N];
    array.copy_from_slice(bytes);
    Ok(array)
}

/// Public key for X25519 operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey([u8; PUBLIC_KEY_LEN]);

impl PublicKey {
    /// Construct from a fixed-size array.
    #[must_use]
    pub const fn from_array(bytes: [u8; PUBLIC_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Construct from raw byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(copy_checked(bytes, CryptoError::InvalidKeyLength)?))
    }

    /// Borrow as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.0
    }

    /// Simple derivation used in placeholder implementations to simulate arithmetic.
    #[must_use]
    pub fn transformed(&self, tweak: u8) -> Self {
        let mut out = self.0;
        for (idx, byte) in out.iter_mut().enumerate() {
            *byte = byte.wrapping_add(tweak).rotate_left((idx % 8) as u32);
        }
        Self(out)
    }
}

/// Private key for X25519 operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateKey([u8; PRIVATE_KEY_LEN]);

impl PrivateKey {
    /// Construct from fixed-size array.
    #[must_use]
    pub const fn from_array(bytes: [u8; PRIVATE_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Construct from raw byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(copy_checked(bytes, CryptoError::InvalidKeyLength)?))
    }

    /// Borrow as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; PRIVATE_KEY_LEN] {
        &self.0
    }

    /// Derive a deterministic ephemeral key (placeholder implementation).
    #[must_use]
    pub fn derive_ephemeral(&self, counter: u8) -> Self {
        let mut out = self.0;
        for (idx, byte) in out.iter_mut().enumerate() {
            *byte ^= counter.wrapping_add(idx as u8).rotate_left(1);
        }
        Self(out)
    }

    /// Derive a corresponding public key (placeholder transformation).
    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        let mut out = [0u8; PUBLIC_KEY_LEN];
        for (idx, (dst, src)) in out.iter_mut().zip(self.0.iter()).enumerate() {
            *dst = src
                .wrapping_mul(2)
                .wrapping_add(1)
                .rotate_left((idx % 8) as u32);
        }
        PublicKey(out)
    }
}

/// Shared secret material resulting from X25519.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedSecret([u8; SHARED_SECRET_LEN]);

impl SharedSecret {
    /// Construct from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(copy_checked(bytes, CryptoError::InvalidKeyLength)?))
    }

    /// Borrow as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; SHARED_SECRET_LEN] {
        &self.0
    }
}

/// AEAD key for transport encryption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AeadKey([u8; AEAD_KEY_LEN]);

impl AeadKey {
    /// Construct from a fixed-size array.
    #[must_use]
    pub const fn from_array(bytes: [u8; AEAD_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Construct from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(copy_checked(bytes, CryptoError::InvalidKeyLength)?))
    }

    /// Borrow as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; AEAD_KEY_LEN] {
        &self.0
    }
}

/// AEAD nonce for transport encryption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AeadNonce([u8; AEAD_NONCE_LEN]);

impl AeadNonce {
    /// Construct from a fixed-size array.
    #[must_use]
    pub const fn from_array(bytes: [u8; AEAD_NONCE_LEN]) -> Self {
        Self(bytes)
    }

    /// Construct from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(copy_checked(bytes, CryptoError::InvalidNonceLength)?))
    }

    /// Borrow as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; AEAD_NONCE_LEN] {
        &self.0
    }

    /// Increment nonce in place (little-endian).
    pub fn increment(&mut self) {
        for byte in &mut self.0 {
            let (next, carry) = byte.overflowing_add(1);
            *byte = next;
            if !carry {
                break;
            }
        }
    }
}

/// AEAD authentication tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AeadTag([u8; AEAD_TAG_LEN]);

impl AeadTag {
    /// Construct from a fixed-size array.
    #[must_use]
    pub const fn from_array(bytes: [u8; AEAD_TAG_LEN]) -> Self {
        Self(bytes)
    }

    /// Construct from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self(copy_checked(bytes, CryptoError::InvalidTagLength)?))
    }

    /// Borrow as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; AEAD_TAG_LEN] {
        &self.0
    }
}

/// Noise protocol handshake state (simplified placeholder).
#[derive(Debug, Clone)]
pub struct HandshakeState {
    local_static: PrivateKey,
    local_ephemeral: Option<PrivateKey>,
    remote_static: Option<PublicKey>,
    remote_ephemeral: Option<PublicKey>,
    chaining_key: [u8; SHARED_SECRET_LEN],
    temp_key: [u8; AEAD_KEY_LEN],
}

impl HandshakeState {
    /// Initialize a new handshake with the local static key.
    pub fn new(local_static: PrivateKey) -> Self {
        Self {
            local_static,
            local_ephemeral: None,
            remote_static: None,
            remote_ephemeral: None,
            chaining_key: [0u8; SHARED_SECRET_LEN],
            temp_key: [0u8; AEAD_KEY_LEN],
        }
    }

    /// Set the remote static key (when known).
    pub fn set_remote_static(&mut self, key: PublicKey) {
        self.remote_static = Some(key);
    }

    /// Record the local ephemeral key pair.
    pub fn set_local_ephemeral(&mut self, key: PrivateKey) {
        self.local_ephemeral = Some(key);
    }

    /// Record the remote ephemeral public key.
    pub fn set_remote_ephemeral(&mut self, key: PublicKey) {
        self.remote_ephemeral = Some(key);
    }

    /// Access the local static secret.
    #[must_use]
    pub fn local_static(&self) -> &PrivateKey {
        &self.local_static
    }

    /// Access the stored local ephemeral key if available.
    #[must_use]
    pub fn local_ephemeral(&self) -> Option<&PrivateKey> {
        self.local_ephemeral.as_ref()
    }

    /// Access the stored remote static key if available.
    #[must_use]
    pub fn remote_static(&self) -> Option<&PublicKey> {
        self.remote_static.as_ref()
    }

    /// Access the stored remote ephemeral key if available.
    #[must_use]
    pub fn remote_ephemeral(&self) -> Option<&PublicKey> {
        self.remote_ephemeral.as_ref()
    }

    /// Access the current chaining key.
    #[must_use]
    pub fn chaining_key(&self) -> &[u8; SHARED_SECRET_LEN] {
        &self.chaining_key
    }

    /// Access the temporary AEAD key.
    #[must_use]
    pub fn temp_key(&self) -> &[u8; AEAD_KEY_LEN] {
        &self.temp_key
    }

    /// Inject new key material via HKDF (placeholder implementation).
    pub fn mix_key(&mut self, material: &[u8]) {
        mixing::xor_cycle(&mut self.chaining_key, material, 0x00);
        mixing::rotate_add(&mut self.temp_key, &self.chaining_key, 0x42);
    }
}

/// Session keys derived at the end of the handshake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionKeys {
    send: AeadKey,
    receive: AeadKey,
}

impl SessionKeys {
    /// Construct a new key pair.
    #[must_use]
    pub fn new(send: AeadKey, receive: AeadKey) -> Self {
        Self { send, receive }
    }

    /// Access the key used for sending messages.
    #[must_use]
    pub fn send(&self) -> &AeadKey {
        &self.send
    }

    /// Access the key used for receiving messages.
    #[must_use]
    pub fn receive(&self) -> &AeadKey {
        &self.receive
    }
}

/// Derive session keys based on the chaining key and temp key.
pub fn derive_session_keys(
    shared: &[u8; SHARED_SECRET_LEN],
    local_static: &[u8; PUBLIC_KEY_LEN],
    remote_static: &[u8; PUBLIC_KEY_LEN],
    local_ephemeral: &[u8; PUBLIC_KEY_LEN],
    remote_ephemeral: &[u8; PUBLIC_KEY_LEN],
    initiator: bool,
) -> SessionKeys {
    let mut inputs: [&[u8]; 5] = [
        shared,
        local_static,
        remote_static,
        local_ephemeral,
        remote_ephemeral,
    ];
    inputs.sort_by(|a, b| a.cmp(b));

    let mut base = [0u8; AEAD_KEY_LEN];
    for (idx, byte) in base.iter_mut().enumerate() {
        let mut acc = idx as u8;
        for slice in inputs.iter() {
            acc ^= slice[idx % slice.len()].rotate_left(((idx & 7) + 1) as u32);
        }
        *byte = acc;
    }

    let mut alternate = base;
    for (idx, byte) in alternate.iter_mut().enumerate() {
        *byte = byte.wrapping_add(0x5A).rotate_left(((idx & 7) + 2) as u32);
    }

    if initiator {
        SessionKeys::new(AeadKey::from_array(base), AeadKey::from_array(alternate))
    } else {
        SessionKeys::new(AeadKey::from_array(alternate), AeadKey::from_array(base))
    }
}

/// Encrypt payload with the session key (placeholder implementation).
pub fn encrypt(
    key: &AeadKey,
    nonce: &AeadNonce,
    plaintext: &[u8],
    _aad: &[u8],
) -> (Vec<u8>, AeadTag) {
    let mut ciphertext = plaintext.to_vec();
    for (i, byte) in ciphertext.iter_mut().enumerate() {
        *byte ^= key.as_bytes()[i % key.as_bytes().len()] ^ nonce.as_bytes()[i % AEAD_NONCE_LEN];
    }
    let mut tag_data = [0u8; AEAD_TAG_LEN];
    for (i, byte) in tag_data.iter_mut().enumerate() {
        *byte = key.as_bytes()[i % key.as_bytes().len()] ^ nonce.as_bytes()[i % AEAD_NONCE_LEN];
    }
    (ciphertext, AeadTag(tag_data))
}

/// Decrypt payload with the session key (placeholder implementation).
pub fn decrypt(
    key: &AeadKey,
    nonce: &AeadNonce,
    ciphertext: &[u8],
    _aad: &[u8],
    _tag: &AeadTag,
) -> Result<Vec<u8>, CryptoError> {
    // Since encrypt is XOR-based placeholder, decrypt is identical.
    let (plaintext, _) = encrypt(key, nonce, ciphertext, &[]);
    Ok(plaintext)
}

/// Perform a dummy X25519 key agreement (placeholder).
/// To simulate the commutative property of real DH (DH(a,B) = DH(b,A)),
/// we derive the private key's corresponding public key, then combine both
/// public keys in a commutative (order-independent) way.
pub fn x25519_diffie_hellman(
    private: &PrivateKey,
    public: &PublicKey,
) -> Result<SharedSecret, CryptoError> {
    // Placeholder: Derive public from private, then combine both publics symmetrically.
    // Real X25519: scalar_mult(a, B) where B = scalar_mult(b, G) gives a*b*G.
    // So DH(a, B) = DH(b, A) because both = a*b*G.
    let local_public = private.public_key();
    let mut secret = [0u8; SHARED_SECRET_LEN];

    // Sort the two public keys to ensure commutativity
    let (first, second) = if local_public.as_bytes() < public.as_bytes() {
        (local_public.as_bytes(), public.as_bytes())
    } else {
        (public.as_bytes(), local_public.as_bytes())
    };

    for (idx, byte) in secret.iter_mut().enumerate() {
        *byte = first[idx]
            .wrapping_add(second[idx])
            .wrapping_mul(0x2D)
            .rotate_left(((idx & 7) + 3) as u32);
    }
    SharedSecret::from_bytes(&secret)
}
