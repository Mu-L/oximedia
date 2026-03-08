//! SRT encryption support using AES.
//!
//! Provides AES-128/192/256 encryption for SRT payloads.

#![allow(dead_code)]

use crate::error::{NetError, NetResult};
use bytes::{Bytes, BytesMut};

/// AES encryption context.
#[derive(Debug, Clone)]
pub struct AesContext {
    /// Encryption key.
    key: Vec<u8>,
    /// Key size (16, 24, or 32 bytes).
    key_size: usize,
    /// Salt for key derivation.
    salt: [u8; 16],
    /// Current key index (for key rotation).
    key_index: u8,
}

impl AesContext {
    /// Creates a new AES context from a passphrase.
    ///
    /// # Errors
    ///
    /// Returns an error if the key size is invalid.
    pub fn from_passphrase(passphrase: &str, key_size: usize) -> NetResult<Self> {
        if ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Invalid key size"));
        }

        let salt = generate_salt();
        let key = derive_key(passphrase.as_bytes(), &salt, key_size);

        Ok(Self {
            key,
            key_size,
            salt,
            key_index: 0,
        })
    }

    /// Creates a new AES context from a raw key.
    ///
    /// # Errors
    ///
    /// Returns an error if the key size is invalid.
    pub fn from_key(key: &[u8]) -> NetResult<Self> {
        let key_size = key.len();
        if ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Invalid key size"));
        }

        Ok(Self {
            key: key.to_vec(),
            key_size,
            salt: [0; 16],
            key_index: 0,
        })
    }

    /// Returns the key size in bytes.
    #[must_use]
    pub const fn key_size(&self) -> usize {
        self.key_size
    }

    /// Returns the salt.
    #[must_use]
    pub const fn salt(&self) -> &[u8; 16] {
        &self.salt
    }

    /// Encrypts a payload.
    ///
    /// # Errors
    ///
    /// Returns an error if encryption fails.
    pub fn encrypt(&self, plaintext: &[u8], iv: &[u8; 16]) -> NetResult<Bytes> {
        // Simple CTR mode encryption (for demonstration)
        // In production, use a proper crypto library like aes-gcm
        let mut ciphertext = BytesMut::with_capacity(plaintext.len());
        let mut counter_block = *iv;

        for chunk in plaintext.chunks(16) {
            let keystream = aes_encrypt_block(&self.key, &counter_block);

            for (i, &byte) in chunk.iter().enumerate() {
                ciphertext.extend_from_slice(&[byte ^ keystream[i]]);
            }

            increment_counter(&mut counter_block);
        }

        Ok(ciphertext.freeze())
    }

    /// Decrypts a payload.
    ///
    /// # Errors
    ///
    /// Returns an error if decryption fails.
    pub fn decrypt(&self, ciphertext: &[u8], iv: &[u8; 16]) -> NetResult<Bytes> {
        // CTR mode decryption is the same as encryption
        self.encrypt(ciphertext, iv)
    }

    /// Rotates to the next key index.
    pub fn rotate_key(&mut self) {
        self.key_index = self.key_index.wrapping_add(1);
    }

    /// Returns the current key index.
    #[must_use]
    pub const fn key_index(&self) -> u8 {
        self.key_index
    }
}

/// Derives an encryption key from a passphrase using PBKDF2-like derivation.
fn derive_key(passphrase: &[u8], salt: &[u8], key_len: usize) -> Vec<u8> {
    // Simple key derivation (in production use PBKDF2)
    let mut key = Vec::with_capacity(key_len);
    let mut hash = hash_bytes(passphrase, salt);

    while key.len() < key_len {
        key.extend_from_slice(&hash);
        hash = hash_bytes(&hash, salt);
    }

    key.truncate(key_len);
    key
}

/// Generates a random salt.
fn generate_salt() -> [u8; 16] {
    // In production, use a cryptographically secure RNG
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345);

    let mut salt = [0u8; 16];
    let mut state = seed;

    for byte in &mut salt {
        state = lcg_next(state);
        *byte = (state & 0xFF) as u8;
    }

    salt
}

/// Simple LCG for pseudo-random number generation.
const fn lcg_next(state: u64) -> u64 {
    state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

/// Simple hash function (for demonstration).
fn hash_bytes(data: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let mut state: u64 = 0x517c_c1b7_2722_0a95;

    for &byte in salt {
        state = state.wrapping_mul(31).wrapping_add(u64::from(byte));
    }

    for &byte in data {
        state = state.wrapping_mul(31).wrapping_add(u64::from(byte));
    }

    for (i, chunk) in hash.chunks_mut(8).enumerate() {
        state = lcg_next(state);
        let bytes = state.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len().min(8)]);
        state = state.wrapping_add(u64::try_from(i).unwrap_or(0));
    }

    hash
}

/// Encrypts a single AES block (simplified for demonstration).
fn aes_encrypt_block(key: &[u8], block: &[u8; 16]) -> [u8; 16] {
    // This is a simplified placeholder
    // In production, use the 'aes' crate with proper AES implementation
    let mut output = *block;

    // Simple XOR with key material (not real AES!)
    for (i, byte) in output.iter_mut().enumerate() {
        *byte ^= key[i % key.len()];
    }

    // Add some mixing
    for _ in 0..4 {
        mix_block(&mut output);
    }

    output
}

/// Mixes block bytes (simplified).
fn mix_block(block: &mut [u8; 16]) {
    for i in 0..16 {
        block[i] = block[i].wrapping_add(block[(i + 1) % 16]);
        block[i] = block[i].rotate_left(3);
    }
}

/// Increments a counter block.
fn increment_counter(counter: &mut [u8; 16]) {
    for byte in counter.iter_mut().rev() {
        *byte = byte.wrapping_add(1);
        if *byte != 0 {
            break;
        }
    }
}

/// Key material exchange information.
#[derive(Debug, Clone)]
pub struct KeyMaterial {
    /// Key encryption key.
    pub kek: Vec<u8>,
    /// Salt.
    pub salt: [u8; 16],
    /// Key length.
    pub key_len: u8,
}

impl KeyMaterial {
    /// Creates new key material.
    #[must_use]
    pub fn new(key_len: u8) -> Self {
        let salt = generate_salt();
        let kek = vec![0u8; key_len as usize];

        Self { kek, salt, key_len }
    }

    /// Encodes key material for transmission.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(17 + self.kek.len());
        buf.push(self.key_len);
        buf.extend_from_slice(&self.salt);
        buf.extend_from_slice(&self.kek);
        buf
    }

    /// Decodes key material from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is invalid.
    pub fn decode(data: &[u8]) -> NetResult<Self> {
        if data.len() < 17 {
            return Err(NetError::parse(0, "Key material too short"));
        }

        let key_len = data[0];
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&data[1..17]);
        let kek = data[17..].to_vec();

        Ok(Self { kek, salt, key_len })
    }
}

/// Number of PBKDF2-like iterations for key derivation.
const KDF_ITERATIONS: u32 = 2048;

/// SRT key-exchange version.
const KM_VERSION: u8 = 1;

/// Derives a session key from a passphrase and salt using an iterative
/// PBKDF2-like approach (SHA-256 family, for demonstration).
///
/// The function runs `iterations` rounds of the internal hash function and
/// XORs the blocks together, producing `key_len` bytes of output.
#[must_use]
pub fn derive_session_key(
    passphrase: &[u8],
    salt: &[u8],
    key_len: usize,
    iterations: u32,
) -> Vec<u8> {
    // PRF block function: HMAC-SHA256-like using hash_bytes
    let mut output = vec![0u8; key_len];
    let blocks_needed = key_len.div_ceil(32);

    for block_idx in 0..blocks_needed {
        // U_1 = PRF(password, salt || INT(i))
        let mut int_bytes = [0u8; 4];
        int_bytes[0] = ((block_idx + 1) >> 24) as u8;
        int_bytes[1] = ((block_idx + 1) >> 16) as u8;
        int_bytes[2] = ((block_idx + 1) >> 8) as u8;
        int_bytes[3] = (block_idx + 1) as u8;

        let mut combined_salt = salt.to_vec();
        combined_salt.extend_from_slice(&int_bytes);

        let mut u = hash_bytes(passphrase, &combined_salt);
        let mut t = u;

        for _ in 1..iterations {
            u = hash_bytes(passphrase, &u);
            for (a, b) in t.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }

        let start = block_idx * 32;
        let end = (start + 32).min(key_len);
        output[start..end].copy_from_slice(&t[..end - start]);
    }

    output
}

/// AES key wrapping (RFC 3394) - simplified for demonstration.
///
/// In production code this should use a proper AES-WRAP implementation.
fn aes_key_wrap(kek: &[u8], plaintext_key: &[u8]) -> Vec<u8> {
    // Simplified: XOR-based wrapping for test purposes
    let mut wrapped = Vec::with_capacity(plaintext_key.len() + 8);
    // Integrity check value (8 bytes)
    wrapped.extend_from_slice(&[0xA6u8; 8]);
    for (i, &byte) in plaintext_key.iter().enumerate() {
        wrapped.push(byte ^ kek[i % kek.len()]);
    }
    wrapped
}

/// AES key unwrapping (RFC 3394) - simplified for demonstration.
fn aes_key_unwrap(kek: &[u8], wrapped_key: &[u8]) -> NetResult<Vec<u8>> {
    if wrapped_key.len() < 8 {
        return Err(NetError::protocol("Wrapped key too short"));
    }
    // Check integrity check value
    let icv = &wrapped_key[..8];
    if icv != [0xA6u8; 8] {
        return Err(NetError::protocol("Key wrap integrity check failed"));
    }
    let payload = &wrapped_key[8..];
    let mut key = Vec::with_capacity(payload.len());
    for (i, &byte) in payload.iter().enumerate() {
        key.push(byte ^ kek[i % kek.len()]);
    }
    Ok(key)
}

/// Key schedule for SRT: holds both even and odd session keys.
#[derive(Debug, Clone)]
pub struct KeySchedule {
    /// Even key (key index 0).
    even_key: Vec<u8>,
    /// Odd key (key index 1).
    odd_key: Vec<u8>,
    /// Key size in bytes (16, 24, or 32).
    key_size: usize,
    /// Current active key index (0 = even, 1 = odd).
    active_index: u8,
}

impl KeySchedule {
    /// Creates a new key schedule from a passphrase and salt.
    ///
    /// # Errors
    ///
    /// Returns an error if the key size is invalid.
    pub fn from_passphrase(passphrase: &str, salt: &[u8; 14], key_size: usize) -> NetResult<Self> {
        if ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Invalid key size for key schedule"));
        }

        // Derive two separate keys: even uses salt with 0x00 prefix, odd with 0x01 prefix
        let mut even_salt = vec![0x00u8];
        even_salt.extend_from_slice(salt);
        let mut odd_salt = vec![0x01u8];
        odd_salt.extend_from_slice(salt);

        let even_key =
            derive_session_key(passphrase.as_bytes(), &even_salt, key_size, KDF_ITERATIONS);
        let odd_key =
            derive_session_key(passphrase.as_bytes(), &odd_salt, key_size, KDF_ITERATIONS);

        Ok(Self {
            even_key,
            odd_key,
            key_size,
            active_index: 0,
        })
    }

    /// Returns the currently active key.
    #[must_use]
    pub fn active_key(&self) -> &[u8] {
        if self.active_index == 0 {
            &self.even_key
        } else {
            &self.odd_key
        }
    }

    /// Returns the even key.
    #[must_use]
    pub fn even_key(&self) -> &[u8] {
        &self.even_key
    }

    /// Returns the odd key.
    #[must_use]
    pub fn odd_key(&self) -> &[u8] {
        &self.odd_key
    }

    /// Returns the active key index (0 = even, 1 = odd).
    #[must_use]
    pub const fn active_index(&self) -> u8 {
        self.active_index
    }

    /// Switches to the other key (key rotation).
    pub fn rotate(&mut self) {
        self.active_index = 1 - self.active_index;
    }

    /// Returns the key size.
    #[must_use]
    pub const fn key_size(&self) -> usize {
        self.key_size
    }
}

/// SRT packet for encryption/decryption operations.
#[derive(Debug, Clone)]
pub struct SrtPacketBuffer {
    /// Packet sequence number (used for IV generation).
    pub seq_no: u32,
    /// Encryption flag (0 = clear, 1 = even key, 2 = odd key).
    pub encryption_flag: u8,
    /// Packet payload.
    pub payload: Vec<u8>,
}

impl SrtPacketBuffer {
    /// Creates a new packet buffer.
    #[must_use]
    pub fn new(seq_no: u32, encryption_flag: u8, payload: Vec<u8>) -> Self {
        Self {
            seq_no,
            encryption_flag,
            payload,
        }
    }
}

/// SRT crypto context: manages the full lifecycle of SRT encryption.
///
/// This includes key derivation, key schedule, key material (KM) exchange,
/// and packet-level encrypt/decrypt operations.
#[derive(Debug)]
pub struct SrtCryptoContext {
    /// Key schedule holding even/odd session keys.
    key_schedule: KeySchedule,
    /// Salt (14 bytes, per SRT spec).
    salt: [u8; 14],
    /// AES context used for packet encryption.
    aes: AesContext,
    /// Total packets encrypted (for IV derivation).
    packet_count: u64,
}

impl SrtCryptoContext {
    /// Creates a new crypto context from a passphrase.
    ///
    /// # Errors
    ///
    /// Returns an error if `key_size` is not 16, 24, or 32.
    pub fn from_passphrase(passphrase: &str, key_size: usize) -> NetResult<Self> {
        if ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Invalid key size for SrtCryptoContext"));
        }

        let raw_salt = generate_salt();
        let mut salt = [0u8; 14];
        salt.copy_from_slice(&raw_salt[..14]);

        let key_schedule = KeySchedule::from_passphrase(passphrase, &salt, key_size)?;
        let aes = AesContext::from_key(key_schedule.active_key())?;

        Ok(Self {
            key_schedule,
            salt,
            aes,
            packet_count: 0,
        })
    }

    /// Creates a crypto context from explicit even/odd session keys.
    ///
    /// # Errors
    ///
    /// Returns an error if the key vectors don't match `key_size`.
    pub fn from_keys(even_key: Vec<u8>, odd_key: Vec<u8>, salt: [u8; 14]) -> NetResult<Self> {
        let key_size = even_key.len();
        if key_size != odd_key.len() || ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Mismatched or invalid key sizes"));
        }

        let key_schedule = KeySchedule {
            even_key: even_key.clone(),
            odd_key,
            key_size,
            active_index: 0,
        };
        let aes = AesContext::from_key(&even_key)?;

        Ok(Self {
            key_schedule,
            salt,
            aes,
            packet_count: 0,
        })
    }

    /// Returns the salt.
    #[must_use]
    pub const fn salt(&self) -> &[u8; 14] {
        &self.salt
    }

    /// Returns the key schedule.
    #[must_use]
    pub const fn key_schedule(&self) -> &KeySchedule {
        &self.key_schedule
    }

    /// Returns the number of packets encrypted so far.
    #[must_use]
    pub const fn packet_count(&self) -> u64 {
        self.packet_count
    }

    /// Derives the IV from the salt and packet sequence number.
    ///
    /// SRT spec: IV = salt XOR (seq_no in last 4 bytes, zero-padded to 16 bytes).
    #[must_use]
    fn derive_iv(&self, seq_no: u32) -> [u8; 16] {
        let mut iv = [0u8; 16];
        // Copy salt into first 14 bytes
        iv[..14].copy_from_slice(&self.salt);
        // XOR last 4 bytes with seq_no (big-endian)
        iv[12] ^= ((seq_no >> 24) & 0xFF) as u8;
        iv[13] ^= ((seq_no >> 16) & 0xFF) as u8;
        iv[14] = ((seq_no >> 8) & 0xFF) as u8;
        iv[15] = (seq_no & 0xFF) as u8;
        iv
    }

    /// Encrypts an SRT packet payload in-place using XOR (stub for testing).
    ///
    /// In production this would use AES-CTR with the derived IV.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is empty or encryption fails.
    pub fn encrypt_packet(&mut self, packet: &mut SrtPacketBuffer) -> NetResult<()> {
        if packet.payload.is_empty() {
            return Err(NetError::protocol("Cannot encrypt empty payload"));
        }

        let key = self.key_schedule.active_key().to_vec();
        let iv = self.derive_iv(packet.seq_no);

        // Use the AES context for CTR-mode encryption
        let encrypted = self.aes.encrypt(&packet.payload, &iv)?;
        packet.payload = encrypted.to_vec();

        // Set encryption flag: 1 = even key, 2 = odd key
        packet.encryption_flag = self.key_schedule.active_index() + 1;
        self.packet_count += 1;

        let _ = key; // used implicitly via aes context
        Ok(())
    }

    /// Decrypts an SRT packet payload using XOR (stub for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the packet is not encrypted or decryption fails.
    pub fn decrypt_packet(&self, packet: &mut SrtPacketBuffer) -> NetResult<()> {
        if packet.encryption_flag == 0 {
            return Err(NetError::protocol("Packet is not encrypted"));
        }
        if packet.payload.is_empty() {
            return Err(NetError::protocol("Cannot decrypt empty payload"));
        }

        // Select key based on encryption flag
        let key = if packet.encryption_flag == 1 {
            self.key_schedule.even_key()
        } else {
            self.key_schedule.odd_key()
        };

        let iv = self.derive_iv(packet.seq_no);

        // CTR mode decryption = encryption
        let ctx = AesContext::from_key(key)?;
        let decrypted = ctx.decrypt(&packet.payload, &iv)?;
        packet.payload = decrypted.to_vec();
        packet.encryption_flag = 0;
        Ok(())
    }

    /// Rotates to the next key and refreshes the AES context.
    ///
    /// # Errors
    ///
    /// Returns an error if the new key is invalid.
    pub fn rotate_key(&mut self) -> NetResult<()> {
        self.key_schedule.rotate();
        self.aes = AesContext::from_key(self.key_schedule.active_key())?;
        Ok(())
    }

    /// Builds a `KeyMaterial` packet for key exchange.
    ///
    /// Wraps both session keys using the Key Encryption Key (KEK) derived
    /// from the passphrase.
    ///
    /// # Errors
    ///
    /// Returns an error if wrapping fails.
    pub fn build_key_material(&self, kek: &[u8]) -> NetResult<KeyMaterialPacket> {
        let wrapped_even = aes_key_wrap(kek, self.key_schedule.even_key());
        let wrapped_odd = aes_key_wrap(kek, self.key_schedule.odd_key());

        Ok(KeyMaterialPacket {
            version: KM_VERSION,
            key_size: self.key_schedule.key_size() as u8,
            salt: self.salt,
            wrapped_even_key: wrapped_even,
            wrapped_odd_key: wrapped_odd,
        })
    }

    /// Loads session keys from a `KeyMaterialPacket` using the KEK.
    ///
    /// # Errors
    ///
    /// Returns an error if unwrapping fails or key sizes mismatch.
    pub fn load_key_material(&mut self, km: &KeyMaterialPacket, kek: &[u8]) -> NetResult<()> {
        let even_key = aes_key_unwrap(kek, &km.wrapped_even_key)?;
        let odd_key = aes_key_unwrap(kek, &km.wrapped_odd_key)?;

        if even_key.len() != km.key_size as usize || odd_key.len() != km.key_size as usize {
            return Err(NetError::protocol("Unwrapped key size mismatch"));
        }

        self.key_schedule.even_key = even_key.clone();
        self.key_schedule.odd_key = odd_key;
        self.key_schedule.key_size = km.key_size as usize;
        self.salt = km.salt;
        self.aes = AesContext::from_key(&even_key)?;
        Ok(())
    }
}

/// Key Material Packet (KM) as defined by the SRT spec.
///
/// This packet is sent during the handshake to exchange session keys.
#[derive(Debug, Clone)]
pub struct KeyMaterialPacket {
    /// KM version (always 1).
    pub version: u8,
    /// Key size in bytes.
    pub key_size: u8,
    /// Salt (14 bytes).
    pub salt: [u8; 14],
    /// AES-wrapped even session key.
    pub wrapped_even_key: Vec<u8>,
    /// AES-wrapped odd session key.
    pub wrapped_odd_key: Vec<u8>,
}

impl KeyMaterialPacket {
    /// Encodes the packet to bytes for transmission.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.version);
        buf.push(self.key_size);
        buf.extend_from_slice(&self.salt);
        let even_len = self.wrapped_even_key.len() as u16;
        buf.push((even_len >> 8) as u8);
        buf.push((even_len & 0xFF) as u8);
        buf.extend_from_slice(&self.wrapped_even_key);
        let odd_len = self.wrapped_odd_key.len() as u16;
        buf.push((odd_len >> 8) as u8);
        buf.push((odd_len & 0xFF) as u8);
        buf.extend_from_slice(&self.wrapped_odd_key);
        buf
    }

    /// Decodes a packet from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is too short or malformed.
    pub fn decode(data: &[u8]) -> NetResult<Self> {
        if data.len() < 18 {
            return Err(NetError::parse(0, "KeyMaterialPacket too short"));
        }
        let version = data[0];
        let key_size = data[1];
        let mut salt = [0u8; 14];
        salt.copy_from_slice(&data[2..16]);

        let even_len = (u16::from(data[16]) << 8 | u16::from(data[17])) as usize;
        if data.len() < 18 + even_len + 2 {
            return Err(NetError::parse(0, "KeyMaterialPacket truncated (even key)"));
        }
        let wrapped_even_key = data[18..18 + even_len].to_vec();

        let pos = 18 + even_len;
        let odd_len = (u16::from(data[pos]) << 8 | u16::from(data[pos + 1])) as usize;
        if data.len() < pos + 2 + odd_len {
            return Err(NetError::parse(0, "KeyMaterialPacket truncated (odd key)"));
        }
        let wrapped_odd_key = data[pos + 2..pos + 2 + odd_len].to_vec();

        Ok(Self {
            version,
            key_size,
            salt,
            wrapped_even_key,
            wrapped_odd_key,
        })
    }
}

/// Password-based authentication for SRT connections.
///
/// Derives a Key Encryption Key (KEK) from a passphrase, which is then used
/// to wrap/unwrap the actual session keys in the key material exchange.
#[derive(Debug, Clone)]
pub struct PassphraseAuth {
    /// Derived Key Encryption Key.
    kek: Vec<u8>,
    /// Salt used for KEK derivation.
    kek_salt: [u8; 16],
    /// Key size (16, 24, or 32 bytes).
    key_size: usize,
}

impl PassphraseAuth {
    /// Creates a new `PassphraseAuth` from a passphrase.
    ///
    /// Derives the KEK using a PBKDF2-like scheme.
    ///
    /// # Errors
    ///
    /// Returns an error if `key_size` is not 16, 24, or 32.
    pub fn new(passphrase: &str, key_size: usize) -> NetResult<Self> {
        if ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Invalid key size for PassphraseAuth"));
        }

        let kek_salt = generate_salt();
        let kek = derive_session_key(passphrase.as_bytes(), &kek_salt, key_size, KDF_ITERATIONS);

        Ok(Self {
            kek,
            kek_salt,
            key_size,
        })
    }

    /// Creates a `PassphraseAuth` with a known salt (for reproducing KEK on the peer).
    ///
    /// # Errors
    ///
    /// Returns an error if `key_size` is not 16, 24, or 32.
    pub fn with_salt(passphrase: &str, key_size: usize, salt: [u8; 16]) -> NetResult<Self> {
        if ![16, 24, 32].contains(&key_size) {
            return Err(NetError::protocol("Invalid key size for PassphraseAuth"));
        }

        let kek = derive_session_key(passphrase.as_bytes(), &salt, key_size, KDF_ITERATIONS);

        Ok(Self {
            kek,
            kek_salt: salt,
            key_size,
        })
    }

    /// Returns the derived KEK.
    #[must_use]
    pub fn kek(&self) -> &[u8] {
        &self.kek
    }

    /// Returns the KEK salt.
    #[must_use]
    pub const fn kek_salt(&self) -> &[u8; 16] {
        &self.kek_salt
    }

    /// Returns the key size.
    #[must_use]
    pub const fn key_size(&self) -> usize {
        self.key_size
    }

    /// Authenticates by verifying that two `PassphraseAuth` instances with the
    /// same passphrase produce the same KEK when given the same salt.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self.kek == other.kek
    }

    /// Wraps session keys using this KEK, returning a `KeyMaterialPacket`.
    ///
    /// # Errors
    ///
    /// Returns an error if key sizes mismatch.
    pub fn wrap_keys(&self, even_key: &[u8], odd_key: &[u8]) -> NetResult<KeyMaterialPacket> {
        if even_key.len() != self.key_size || odd_key.len() != self.key_size {
            return Err(NetError::protocol("Key size mismatch in wrap_keys"));
        }

        let wrapped_even = aes_key_wrap(&self.kek, even_key);
        let wrapped_odd = aes_key_wrap(&self.kek, odd_key);
        let mut salt = [0u8; 14];
        salt.copy_from_slice(&self.kek_salt[..14]);

        Ok(KeyMaterialPacket {
            version: KM_VERSION,
            key_size: self.key_size as u8,
            salt,
            wrapped_even_key: wrapped_even,
            wrapped_odd_key: wrapped_odd,
        })
    }

    /// Unwraps session keys from a `KeyMaterialPacket` using this KEK.
    ///
    /// # Errors
    ///
    /// Returns an error if unwrapping fails.
    pub fn unwrap_keys(&self, km: &KeyMaterialPacket) -> NetResult<(Vec<u8>, Vec<u8>)> {
        let even = aes_key_unwrap(&self.kek, &km.wrapped_even_key)?;
        let odd = aes_key_unwrap(&self.kek, &km.wrapped_odd_key)?;
        Ok((even, odd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_context_from_passphrase() {
        let ctx = AesContext::from_passphrase("test_password", 16).expect("should succeed in test");
        assert_eq!(ctx.key_size(), 16);
    }

    #[test]
    fn test_aes_context_invalid_key_size() {
        let result = AesContext::from_passphrase("test", 15);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_decrypt() {
        let ctx = AesContext::from_passphrase("password", 16).expect("should succeed in test");
        let plaintext = b"Hello, SRT!";
        let iv = [1u8; 16];

        let ciphertext = ctx.encrypt(plaintext, &iv).expect("should succeed in test");
        assert_ne!(ciphertext.as_ref(), plaintext);

        let decrypted = ctx
            .decrypt(&ciphertext, &iv)
            .expect("should succeed in test");
        assert_eq!(decrypted.as_ref(), plaintext);
    }

    #[test]
    fn test_key_rotation() {
        let mut ctx = AesContext::from_passphrase("test", 16).expect("should succeed in test");
        assert_eq!(ctx.key_index(), 0);
        ctx.rotate_key();
        assert_eq!(ctx.key_index(), 1);
    }

    #[test]
    fn test_key_material() {
        let km = KeyMaterial::new(16);
        let encoded = km.encode();
        let decoded = KeyMaterial::decode(&encoded).expect("should succeed in test");
        assert_eq!(decoded.key_len, 16);
    }

    #[test]
    fn test_derive_key() {
        let key1 = derive_key(b"password", b"salt", 16);
        let key2 = derive_key(b"password", b"salt", 16);
        assert_eq!(key1, key2);

        let key3 = derive_key(b"different", b"salt", 16);
        assert_ne!(key1, key3);
    }

    // --- derive_session_key tests ---

    #[test]
    fn test_derive_session_key_deterministic() {
        let key1 = derive_session_key(b"mysecret", b"somesalt", 16, 10);
        let key2 = derive_session_key(b"mysecret", b"somesalt", 16, 10);
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 16);
    }

    #[test]
    fn test_derive_session_key_different_passphrase() {
        let key1 = derive_session_key(b"passA", b"salt", 16, 10);
        let key2 = derive_session_key(b"passB", b"salt", 16, 10);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_derive_session_key_different_salt() {
        let key1 = derive_session_key(b"pass", b"saltA", 16, 10);
        let key2 = derive_session_key(b"pass", b"saltB", 16, 10);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_derive_session_key_32_bytes() {
        let key = derive_session_key(b"pass", b"salt", 32, 10);
        assert_eq!(key.len(), 32);
    }

    // --- KeySchedule tests ---

    #[test]
    fn test_key_schedule_from_passphrase() {
        let salt = [0x01u8; 14];
        let ks =
            KeySchedule::from_passphrase("testpass", &salt, 16).expect("should succeed in test");
        assert_eq!(ks.key_size(), 16);
        assert_eq!(ks.active_index(), 0);
        // Even and odd keys should be different
        assert_ne!(ks.even_key(), ks.odd_key());
    }

    #[test]
    fn test_key_schedule_rotate() {
        let salt = [0x02u8; 14];
        let mut ks =
            KeySchedule::from_passphrase("testpass", &salt, 16).expect("should succeed in test");
        let initial_active = ks.active_key().to_vec();
        ks.rotate();
        let after_rotate = ks.active_key().to_vec();
        assert_ne!(initial_active, after_rotate);
        ks.rotate();
        assert_eq!(ks.active_key(), initial_active.as_slice());
    }

    #[test]
    fn test_key_schedule_invalid_key_size() {
        let salt = [0u8; 14];
        let result = KeySchedule::from_passphrase("pass", &salt, 15);
        assert!(result.is_err());
    }

    // --- SrtCryptoContext tests ---

    #[test]
    fn test_srt_crypto_context_creation() {
        let ctx = SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");
        assert_eq!(ctx.packet_count(), 0);
        assert_eq!(ctx.key_schedule().key_size(), 16);
    }

    #[test]
    fn test_srt_crypto_context_invalid_key_size() {
        let result = SrtCryptoContext::from_passphrase("secret", 20);
        assert!(result.is_err());
    }

    #[test]
    fn test_srt_crypto_context_encrypt_decrypt() {
        let mut ctx =
            SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");
        let plaintext = b"Hello, SRT world!".to_vec();

        let mut packet = SrtPacketBuffer::new(1, 0, plaintext.clone());
        ctx.encrypt_packet(&mut packet)
            .expect("should succeed in test");

        // After encryption: payload changed, flag set
        assert_ne!(packet.payload, plaintext);
        assert_ne!(packet.encryption_flag, 0);
        assert_eq!(ctx.packet_count(), 1);

        // Decrypt
        ctx.decrypt_packet(&mut packet)
            .expect("should succeed in test");
        assert_eq!(packet.payload, plaintext);
        assert_eq!(packet.encryption_flag, 0);
    }

    #[test]
    fn test_srt_crypto_context_encrypt_empty_error() {
        let mut ctx =
            SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");
        let mut packet = SrtPacketBuffer::new(1, 0, vec![]);
        let result = ctx.encrypt_packet(&mut packet);
        assert!(result.is_err());
    }

    #[test]
    fn test_srt_crypto_context_decrypt_clear_error() {
        let ctx = SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");
        let mut packet = SrtPacketBuffer::new(1, 0, b"data".to_vec());
        let result = ctx.decrypt_packet(&mut packet);
        assert!(result.is_err());
    }

    #[test]
    fn test_srt_crypto_context_key_rotation() {
        let mut ctx =
            SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");
        let key_before = ctx.key_schedule().active_index();
        ctx.rotate_key().expect("should succeed in test");
        let key_after = ctx.key_schedule().active_index();
        assert_ne!(key_before, key_after);
    }

    #[test]
    fn test_srt_crypto_context_key_material_exchange() {
        let kek = vec![0xABu8; 16];
        let ctx = SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");

        let km = ctx
            .build_key_material(&kek)
            .expect("should succeed in test");
        let encoded = km.encode();
        let decoded = KeyMaterialPacket::decode(&encoded).expect("should succeed in test");

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.key_size, 16);
    }

    #[test]
    fn test_srt_crypto_context_load_key_material() {
        let kek = vec![0xCDu8; 16];
        let ctx1 = SrtCryptoContext::from_passphrase("secret", 16).expect("should succeed in test");
        let km = ctx1
            .build_key_material(&kek)
            .expect("should succeed in test");

        let even_key = vec![0xAAu8; 16];
        let odd_key = vec![0xBBu8; 16];
        let mut salt = [0u8; 14];
        salt[0] = 0x11;
        let mut ctx2 =
            SrtCryptoContext::from_keys(even_key, odd_key, salt).expect("should succeed in test");
        ctx2.load_key_material(&km, &kek)
            .expect("should succeed in test");

        // After loading, the even/odd keys should match ctx1's
        assert_eq!(
            ctx2.key_schedule().even_key(),
            ctx1.key_schedule().even_key()
        );
        assert_eq!(ctx2.key_schedule().odd_key(), ctx1.key_schedule().odd_key());
    }

    // --- PassphraseAuth tests ---

    #[test]
    fn test_passphrase_auth_creation() {
        let auth = PassphraseAuth::new("password123", 16).expect("should succeed in test");
        assert_eq!(auth.key_size(), 16);
        assert_eq!(auth.kek().len(), 16);
    }

    #[test]
    fn test_passphrase_auth_invalid_key_size() {
        let result = PassphraseAuth::new("pass", 20);
        assert!(result.is_err());
    }

    #[test]
    fn test_passphrase_auth_same_salt_produces_same_kek() {
        let salt = [0x42u8; 16];
        let auth1 =
            PassphraseAuth::with_salt("password", 16, salt).expect("should succeed in test");
        let auth2 =
            PassphraseAuth::with_salt("password", 16, salt).expect("should succeed in test");
        assert!(auth1.matches(&auth2));
    }

    #[test]
    fn test_passphrase_auth_different_passwords_different_kek() {
        let salt = [0x42u8; 16];
        let auth1 =
            PassphraseAuth::with_salt("passwordA", 16, salt).expect("should succeed in test");
        let auth2 =
            PassphraseAuth::with_salt("passwordB", 16, salt).expect("should succeed in test");
        assert!(!auth1.matches(&auth2));
    }

    #[test]
    fn test_passphrase_auth_wrap_unwrap_keys() {
        let salt = [0x11u8; 16];
        let auth = PassphraseAuth::with_salt("secret", 16, salt).expect("should succeed in test");
        let even_key = vec![0xAAu8; 16];
        let odd_key = vec![0xBBu8; 16];

        let km = auth
            .wrap_keys(&even_key, &odd_key)
            .expect("should succeed in test");
        let (unwrapped_even, unwrapped_odd) =
            auth.unwrap_keys(&km).expect("should succeed in test");

        assert_eq!(unwrapped_even, even_key);
        assert_eq!(unwrapped_odd, odd_key);
    }

    #[test]
    fn test_passphrase_auth_key_size_24() {
        let auth = PassphraseAuth::new("testpass", 24).expect("should succeed in test");
        assert_eq!(auth.kek().len(), 24);
    }

    #[test]
    fn test_passphrase_auth_key_size_32() {
        let auth = PassphraseAuth::new("testpass", 32).expect("should succeed in test");
        assert_eq!(auth.kek().len(), 32);
    }

    // --- KeyMaterialPacket tests ---

    #[test]
    fn test_km_packet_encode_decode() {
        let km = KeyMaterialPacket {
            version: 1,
            key_size: 16,
            salt: [0xABu8; 14],
            wrapped_even_key: vec![0x11u8; 24], // 16-byte key + 8-byte ICV
            wrapped_odd_key: vec![0x22u8; 24],
        };

        let encoded = km.encode();
        let decoded = KeyMaterialPacket::decode(&encoded).expect("should succeed in test");

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.key_size, 16);
        assert_eq!(decoded.salt, [0xABu8; 14]);
        assert_eq!(decoded.wrapped_even_key, km.wrapped_even_key);
        assert_eq!(decoded.wrapped_odd_key, km.wrapped_odd_key);
    }

    #[test]
    fn test_km_packet_decode_too_short() {
        let result = KeyMaterialPacket::decode(&[0u8; 5]);
        assert!(result.is_err());
    }

    // --- aes_key_wrap / unwrap tests ---

    #[test]
    fn test_key_wrap_unwrap_roundtrip() {
        let kek = vec![0x55u8; 16];
        let key = vec![0xAAu8; 16];
        let wrapped = aes_key_wrap(&kek, &key);
        let unwrapped = aes_key_unwrap(&kek, &wrapped).expect("should succeed in test");
        assert_eq!(unwrapped, key);
    }

    #[test]
    fn test_key_unwrap_bad_icv() {
        let kek = vec![0x55u8; 16];
        let mut bad = vec![0x00u8; 24]; // wrong ICV
        bad[0] = 0xFF;
        let result = aes_key_unwrap(&kek, &bad);
        assert!(result.is_err());
    }
}
