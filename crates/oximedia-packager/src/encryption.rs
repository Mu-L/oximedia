//! Encryption support for adaptive streaming.

use crate::config::EncryptionMethod;
use crate::error::{PackagerError, PackagerResult};
use bytes::{BufMut, BytesMut};
use rand::TryRng;

#[cfg(feature = "encryption")]
use aes::cipher::{
    BlockCipherDecrypt, BlockCipherEncrypt, BlockModeDecrypt, BlockModeEncrypt, KeyInit, KeyIvInit,
};
#[cfg(feature = "encryption")]
use aes::{Aes128, Aes128Dec};
#[cfg(feature = "encryption")]
use cbc::{Decryptor, Encryptor};

/// Encryption key information.
#[derive(Debug, Clone)]
pub struct KeyInfo {
    /// Encryption key (16 bytes for AES-128).
    pub key: Vec<u8>,
    /// Initialization vector.
    pub iv: Vec<u8>,
    /// Key URI (for HLS).
    pub uri: Option<String>,
    /// Key format (for HLS).
    pub format: Option<String>,
    /// Key format versions.
    pub format_versions: Option<String>,
}

impl KeyInfo {
    /// Create new key info.
    #[must_use]
    pub fn new(key: Vec<u8>, iv: Vec<u8>) -> Self {
        Self {
            key,
            iv,
            uri: None,
            format: None,
            format_versions: None,
        }
    }

    /// Set the key URI.
    #[must_use]
    pub fn with_uri(mut self, uri: String) -> Self {
        self.uri = Some(uri);
        self
    }

    /// Set the key format.
    #[must_use]
    pub fn with_format(mut self, format: String) -> Self {
        self.format = Some(format);
        self
    }

    /// Validate key info.
    pub fn validate(&self) -> PackagerResult<()> {
        if self.key.len() != 16 {
            return Err(PackagerError::EncryptionError(
                "Key must be 16 bytes for AES-128".to_string(),
            ));
        }

        if self.iv.len() != 16 {
            return Err(PackagerError::EncryptionError(
                "IV must be 16 bytes".to_string(),
            ));
        }

        Ok(())
    }
}

/// Encryption handler.
pub struct EncryptionHandler {
    method: EncryptionMethod,
    key_info: Option<KeyInfo>,
}

impl EncryptionHandler {
    /// Create a new encryption handler.
    #[must_use]
    pub fn new(method: EncryptionMethod) -> Self {
        Self {
            method,
            key_info: None,
        }
    }

    /// Set key information.
    pub fn set_key_info(&mut self, key_info: KeyInfo) -> PackagerResult<()> {
        key_info.validate()?;
        self.key_info = Some(key_info);
        Ok(())
    }

    /// Check if encryption is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.method != EncryptionMethod::None
    }

    /// Get encryption method.
    #[must_use]
    pub fn method(&self) -> EncryptionMethod {
        self.method
    }

    /// Encrypt data.
    pub fn encrypt(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        if !self.is_enabled() {
            return Ok(data.to_vec());
        }

        match self.method {
            EncryptionMethod::None => Ok(data.to_vec()),
            EncryptionMethod::Aes128 => self.encrypt_aes128(data),
            EncryptionMethod::SampleAes => self.encrypt_sample_aes(data),
            EncryptionMethod::Cenc => self.encrypt_cenc(data),
        }
    }

    /// Decrypt data.
    pub fn decrypt(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        if !self.is_enabled() {
            return Ok(data.to_vec());
        }

        match self.method {
            EncryptionMethod::None => Ok(data.to_vec()),
            EncryptionMethod::Aes128 => self.decrypt_aes128(data),
            EncryptionMethod::SampleAes => self.decrypt_sample_aes(data),
            EncryptionMethod::Cenc => self.decrypt_cenc(data),
        }
    }

    /// Encrypt with AES-128 CBC.
    #[cfg(feature = "encryption")]
    fn encrypt_aes128(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;

        type Aes128CbcEnc = Encryptor<Aes128>;

        let cipher = Aes128CbcEnc::new_from_slices(&key_info.key, &key_info.iv)
            .map_err(|e| PackagerError::EncryptionError(format!("Failed to create cipher: {e}")))?;

        // Allocate buffer with space for PKCS7 padding (at most one extra block)
        let msg_len = data.len();
        let mut buf = vec![0u8; msg_len + 16];
        buf[..msg_len].copy_from_slice(data);

        let encrypted = cipher
            .encrypt_padded::<block_padding::Pkcs7>(&mut buf, msg_len)
            .map_err(|e| PackagerError::EncryptionError(format!("Encryption failed: {e}")))?;

        Ok(encrypted.to_vec())
    }

    /// Encrypt with AES-128 CBC (when encryption feature is disabled).
    #[cfg(not(feature = "encryption"))]
    fn encrypt_aes128(&self, _data: &[u8]) -> PackagerResult<Vec<u8>> {
        Err(PackagerError::EncryptionError(
            "Encryption feature not enabled".to_string(),
        ))
    }

    /// Decrypt with AES-128 CBC.
    #[cfg(feature = "encryption")]
    fn decrypt_aes128(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;

        type Aes128CbcDec = Decryptor<Aes128>;

        let cipher = Aes128CbcDec::new_from_slices(&key_info.key, &key_info.iv)
            .map_err(|e| PackagerError::EncryptionError(format!("Failed to create cipher: {e}")))?;

        let mut buf = data.to_vec();
        let decrypted = cipher
            .decrypt_padded::<block_padding::Pkcs7>(&mut buf)
            .map_err(|e| PackagerError::EncryptionError(format!("Decryption failed: {e}")))?;

        Ok(decrypted.to_vec())
    }

    /// Decrypt with AES-128 CBC (when encryption feature is disabled).
    #[cfg(not(feature = "encryption"))]
    fn decrypt_aes128(&self, _data: &[u8]) -> PackagerResult<Vec<u8>> {
        Err(PackagerError::EncryptionError(
            "Encryption feature not enabled".to_string(),
        ))
    }

    /// Encrypt with SAMPLE-AES (HLS), i.e. the CENC `cbcs` pattern-encryption
    /// scheme (ISO/IEC 23001-7 §9.6): AES-128-CBC applied over a repeating
    /// 1-crypt / 9-skip 16-byte block pattern. Only every 10th block is
    /// encrypted; the intervening blocks and any trailing partial (< 16-byte)
    /// block are left in the clear. The CBC chain spans only the encrypted
    /// blocks (the per-sample IV seeds the first, each ciphertext block seeds
    /// the next). This is what FairPlay / Shaka / hls.js expect; full-buffer
    /// CBC (the previous behaviour) could not be decrypted by any of them.
    ///
    /// TODO(0.2.x): NAL-unit-aware subsample mapping (a clear leader per NAL)
    /// needs a bitstream parser; this applies the pattern over the whole
    /// sample buffer, which is correct for already-elementary media payloads.
    #[cfg(feature = "encryption")]
    fn encrypt_sample_aes(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;
        sample_aes_cbcs_encrypt(&key_info.key, &key_info.iv, data)
    }

    /// SAMPLE-AES encryption stub when the `encryption` feature is disabled.
    #[cfg(not(feature = "encryption"))]
    fn encrypt_sample_aes(&self, _data: &[u8]) -> PackagerResult<Vec<u8>> {
        Err(PackagerError::EncryptionError(
            "Encryption feature not enabled".to_string(),
        ))
    }

    /// Decrypt SAMPLE-AES (`cbcs` pattern) — exact inverse of
    /// [`Self::encrypt_sample_aes`].
    #[cfg(feature = "encryption")]
    fn decrypt_sample_aes(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;
        sample_aes_cbcs_decrypt(&key_info.key, &key_info.iv, data)
    }

    /// SAMPLE-AES decryption stub when the `encryption` feature is disabled.
    #[cfg(not(feature = "encryption"))]
    fn decrypt_sample_aes(&self, _data: &[u8]) -> PackagerResult<Vec<u8>> {
        Err(PackagerError::EncryptionError(
            "Encryption feature not enabled".to_string(),
        ))
    }

    /// Encrypt with Common Encryption `cenc` (ISO/IEC 23001-7): full-sample
    /// AES-128 in CTR mode. The 16-byte IV is used as the initial 128-bit
    /// big-endian counter block, incremented once per 16-byte block. CTR is the
    /// spec-correct cipher for full-sample `cenc` and is what Widevine /
    /// PlayReady / dash.js / Shaka expect; the previous full-buffer CBC could
    /// not be decrypted by any CENC client.
    #[cfg(feature = "encryption")]
    fn encrypt_cenc(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;
        aes128_ctr_apply(&key_info.key, &key_info.iv, data)
    }

    /// CENC encryption stub when the `encryption` feature is disabled.
    #[cfg(not(feature = "encryption"))]
    fn encrypt_cenc(&self, _data: &[u8]) -> PackagerResult<Vec<u8>> {
        Err(PackagerError::EncryptionError(
            "Encryption feature not enabled".to_string(),
        ))
    }

    /// Decrypt `cenc` (AES-128-CTR). CTR is symmetric, so this applies the same
    /// keystream as [`Self::encrypt_cenc`].
    #[cfg(feature = "encryption")]
    fn decrypt_cenc(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;
        aes128_ctr_apply(&key_info.key, &key_info.iv, data)
    }

    /// CENC decryption stub when the `encryption` feature is disabled.
    #[cfg(not(feature = "encryption"))]
    fn decrypt_cenc(&self, _data: &[u8]) -> PackagerResult<Vec<u8>> {
        Err(PackagerError::EncryptionError(
            "Encryption feature not enabled".to_string(),
        ))
    }

    /// Generate HLS EXT-X-KEY tag.
    pub fn generate_hls_key_tag(&self) -> PackagerResult<String> {
        if !self.is_enabled() {
            return Ok(String::new());
        }

        let key_info = self
            .key_info
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key info not set".to_string()))?;

        let method = match self.method {
            EncryptionMethod::Aes128 => "AES-128",
            EncryptionMethod::SampleAes => "SAMPLE-AES",
            _ => {
                return Err(PackagerError::EncryptionError(
                    "Unsupported method for HLS".to_string(),
                ))
            }
        };

        let uri = key_info
            .uri
            .as_ref()
            .ok_or_else(|| PackagerError::EncryptionError("Key URI not set".to_string()))?;

        let iv_hex = hex::encode(&key_info.iv);

        let mut tag = format!("#EXT-X-KEY:METHOD={method},URI=\"{uri}\",IV=0x{iv_hex}");

        if let Some(format) = &key_info.format {
            tag.push_str(&format!(",KEYFORMAT=\"{format}\""));
        }

        if let Some(versions) = &key_info.format_versions {
            tag.push_str(&format!(",KEYFORMATVERSIONS=\"{versions}\""));
        }

        Ok(tag)
    }

    /// Get key info.
    #[must_use]
    pub fn key_info(&self) -> Option<&KeyInfo> {
        self.key_info.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Low-level AES primitives backing the CENC `cenc` (CTR) and `cbcs`
// (pattern-CBC) paths. These operate on raw 16-byte AES blocks (via the `aes`
// crate's block cipher, which selects AES-NI at runtime) so the CTR keystream
// and the crypt/skip pattern can be expressed exactly per ISO/IEC 23001-7,
// without the fixed full-buffer padding of a high-level CBC block mode.
// ---------------------------------------------------------------------------

/// AES block size in bytes.
#[cfg(feature = "encryption")]
const AES_BLOCK: usize = 16;

/// CENC `cbcs` default pattern: encrypt 1 block, skip 9 (ISO/IEC 23001-7 §9.6 —
/// the pattern mandated by Apple FairPlay and used for HLS SAMPLE-AES video).
#[cfg(feature = "encryption")]
const CBCS_CRYPT_BLOCKS: usize = 1;
#[cfg(feature = "encryption")]
const CBCS_SKIP_BLOCKS: usize = 9;

/// Encrypt a single 16-byte AES-128 block.
#[cfg(feature = "encryption")]
fn aes128_encrypt_block(cipher: &Aes128, block: &[u8; AES_BLOCK]) -> [u8; AES_BLOCK] {
    let mut b = aes::Block::from(*block);
    cipher.encrypt_block(&mut b);
    let mut out = [0u8; AES_BLOCK];
    out.copy_from_slice(b.as_slice());
    out
}

/// Decrypt a single 16-byte AES-128 block.
#[cfg(feature = "encryption")]
fn aes128_decrypt_block(cipher: &Aes128Dec, block: &[u8; AES_BLOCK]) -> [u8; AES_BLOCK] {
    let mut b = aes::Block::from(*block);
    cipher.decrypt_block(&mut b);
    let mut out = [0u8; AES_BLOCK];
    out.copy_from_slice(b.as_slice());
    out
}

/// Increment a 16-byte big-endian counter block in place (CTR mode).
#[cfg(feature = "encryption")]
fn increment_be_counter(counter: &mut [u8; AES_BLOCK]) {
    for byte in counter.iter_mut().rev() {
        *byte = byte.wrapping_add(1);
        if *byte != 0 {
            break;
        }
    }
}

/// Apply the AES-128-CTR keystream to `data`.
///
/// CTR encryption and decryption are the same operation (XOR with the
/// keystream), so this backs both directions of the `cenc` scheme. `iv` is the
/// 16-byte initial counter block; it is incremented as a 128-bit big-endian
/// integer once per 16-byte block.
#[cfg(feature = "encryption")]
fn aes128_ctr_apply(key: &[u8], iv: &[u8], data: &[u8]) -> PackagerResult<Vec<u8>> {
    if iv.len() != AES_BLOCK {
        return Err(PackagerError::EncryptionError(format!(
            "CENC (CTR) IV must be 16 bytes, got {}",
            iv.len()
        )));
    }
    let cipher = Aes128::new_from_slice(key).map_err(|e| {
        PackagerError::EncryptionError(format!("Failed to create AES-128 cipher: {e}"))
    })?;

    let mut counter = [0u8; AES_BLOCK];
    counter.copy_from_slice(iv);

    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(AES_BLOCK) {
        let keystream = aes128_encrypt_block(&cipher, &counter);
        for (&byte, &ks) in chunk.iter().zip(keystream.iter()) {
            out.push(byte ^ ks);
        }
        increment_be_counter(&mut counter);
    }
    Ok(out)
}

/// Encrypt `data` with the CENC `cbcs` pattern (AES-128-CBC, 1:9 crypt/skip).
///
/// Every `CBCS_CRYPT_BLOCKS`-of-`(CBCS_CRYPT_BLOCKS + CBCS_SKIP_BLOCKS)` whole
/// 16-byte block is CBC-encrypted; the rest, plus any trailing partial block,
/// are left clear. The CBC chaining register is seeded once with the per-sample
/// IV and advanced only by encrypted blocks.
#[cfg(feature = "encryption")]
fn sample_aes_cbcs_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> PackagerResult<Vec<u8>> {
    if iv.len() != AES_BLOCK {
        return Err(PackagerError::EncryptionError(format!(
            "SAMPLE-AES (cbcs) IV must be 16 bytes, got {}",
            iv.len()
        )));
    }
    let cipher = Aes128::new_from_slice(key).map_err(|e| {
        PackagerError::EncryptionError(format!("Failed to create AES-128 cipher: {e}"))
    })?;

    let cycle = CBCS_CRYPT_BLOCKS + CBCS_SKIP_BLOCKS;
    let full_blocks = data.len() / AES_BLOCK;

    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; AES_BLOCK];
    prev.copy_from_slice(iv);

    for block_idx in 0..full_blocks {
        let start = block_idx * AES_BLOCK;
        let mut block = [0u8; AES_BLOCK];
        block.copy_from_slice(&data[start..start + AES_BLOCK]);

        if block_idx % cycle < CBCS_CRYPT_BLOCKS {
            let mut xored = [0u8; AES_BLOCK];
            for ((dst, &b), &p) in xored.iter_mut().zip(block.iter()).zip(prev.iter()) {
                *dst = b ^ p;
            }
            let ct = aes128_encrypt_block(&cipher, &xored);
            out.extend_from_slice(&ct);
            prev = ct;
        } else {
            out.extend_from_slice(&block);
        }
    }

    // Trailing partial block (< 16 bytes) is always left clear per spec.
    out.extend_from_slice(&data[full_blocks * AES_BLOCK..]);
    Ok(out)
}

/// Decrypt `data` produced by [`sample_aes_cbcs_encrypt`] (exact inverse).
#[cfg(feature = "encryption")]
fn sample_aes_cbcs_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> PackagerResult<Vec<u8>> {
    if iv.len() != AES_BLOCK {
        return Err(PackagerError::EncryptionError(format!(
            "SAMPLE-AES (cbcs) IV must be 16 bytes, got {}",
            iv.len()
        )));
    }
    let cipher = Aes128Dec::new_from_slice(key).map_err(|e| {
        PackagerError::EncryptionError(format!("Failed to create AES-128 cipher: {e}"))
    })?;

    let cycle = CBCS_CRYPT_BLOCKS + CBCS_SKIP_BLOCKS;
    let full_blocks = data.len() / AES_BLOCK;

    let mut out = Vec::with_capacity(data.len());
    let mut prev = [0u8; AES_BLOCK];
    prev.copy_from_slice(iv);

    for block_idx in 0..full_blocks {
        let start = block_idx * AES_BLOCK;
        let mut block = [0u8; AES_BLOCK];
        block.copy_from_slice(&data[start..start + AES_BLOCK]);

        if block_idx % cycle < CBCS_CRYPT_BLOCKS {
            let dec = aes128_decrypt_block(&cipher, &block);
            for (&d, &p) in dec.iter().zip(prev.iter()) {
                out.push(d ^ p);
            }
            prev = block;
        } else {
            out.extend_from_slice(&block);
        }
    }

    out.extend_from_slice(&data[full_blocks * AES_BLOCK..]);
    Ok(out)
}

/// Number of PBKDF2-HMAC-SHA256 rounds used by [`KeyGenerator::from_passphrase`].
///
/// `100_000` rounds meets the current OWASP minimum recommendation for
/// PBKDF2-HMAC-SHA256 password-based key derivation.
pub const PBKDF2_ITERATIONS: u32 = 100_000;

/// Key generator for creating encryption keys.
///
/// All keys and IVs are generated from the operating system's cryptographically
/// secure random number source (via [`rand::rngs::SysRng`], backed by `getrandom`).
pub struct KeyGenerator;

impl KeyGenerator {
    /// Generate a cryptographically secure random AES-128 key.
    ///
    /// Uses the OS CSPRNG ([`rand::rngs::SysRng`]) to fill 16 bytes of key
    /// material. Each call returns an independent, unpredictable key.
    ///
    /// # Errors
    /// Returns [`PackagerError::EncryptionError`] if the operating system's
    /// random number source is unavailable.
    pub fn generate_aes128_key() -> PackagerResult<Vec<u8>> {
        let mut key = vec![0u8; 16];
        rand::rngs::SysRng.try_fill_bytes(&mut key).map_err(|e| {
            PackagerError::EncryptionError(format!(
                "Failed to generate secure random AES-128 key: {e}"
            ))
        })?;
        Ok(key)
    }

    /// Generate a cryptographically secure random initialization vector.
    ///
    /// # Errors
    /// Returns [`PackagerError::EncryptionError`] if the operating system's
    /// random number source is unavailable.
    pub fn generate_iv() -> PackagerResult<Vec<u8>> {
        let mut iv = vec![0u8; 16];
        rand::rngs::SysRng.try_fill_bytes(&mut iv).map_err(|e| {
            PackagerError::EncryptionError(format!("Failed to generate secure random IV: {e}"))
        })?;
        Ok(iv)
    }

    /// Derive a 16-byte AES-128 key from a passphrase using PBKDF2-HMAC-SHA256.
    ///
    /// `salt` should be a unique, random value (at least 16 bytes recommended)
    /// generated once per key and stored/transmitted alongside the derived key
    /// (a salt is not secret, but it must not be reused across unrelated keys).
    /// Callers can generate one via [`Self::generate_iv`] or any other CSPRNG
    /// source. Uses [`PBKDF2_ITERATIONS`] rounds.
    ///
    /// This function is deterministic: the same `passphrase` and `salt` always
    /// produce the same key, while different salts (or passphrases) produce
    /// different keys.
    ///
    /// # Errors
    /// Returns [`PackagerError::EncryptionError`] if the underlying HMAC
    /// cannot be initialized (this only happens for pathological key sizes
    /// and should not occur in practice for `str` passphrases).
    pub fn from_passphrase(passphrase: &str, salt: &[u8]) -> PackagerResult<Vec<u8>> {
        let mut key = [0u8; 16];
        pbkdf2::pbkdf2::<pbkdf2::hmac::Hmac<sha2::Sha256>>(
            passphrase.as_bytes(),
            salt,
            PBKDF2_ITERATIONS,
            &mut key,
        )
        .map_err(|e| {
            PackagerError::EncryptionError(format!("Failed to derive key from passphrase: {e}"))
        })?;

        Ok(key.to_vec())
    }
}

/// DRM preparation hooks.
pub trait DrmProvider {
    /// Get DRM system ID.
    fn system_id(&self) -> &str;

    /// Generate PSSH box data.
    fn generate_pssh(&self, key_id: &[u8]) -> PackagerResult<Vec<u8>>;

    /// Get license server URL.
    fn license_url(&self) -> Option<String>;
}

/// Widevine DRM provider (placeholder).
pub struct WidevineDrmProvider {
    license_url: String,
}

impl WidevineDrmProvider {
    /// Create a new Widevine DRM provider.
    #[must_use]
    pub fn new(license_url: String) -> Self {
        Self { license_url }
    }
}

impl DrmProvider for WidevineDrmProvider {
    fn system_id(&self) -> &'static str {
        "edef8ba9-79d6-4ace-a3c8-27dcd51d21ed" // Widevine system ID
    }

    fn generate_pssh(&self, key_id: &[u8]) -> PackagerResult<Vec<u8>> {
        let mut pssh = BytesMut::new();

        // PSSH box header
        pssh.put_u32(0); // Size placeholder
        pssh.put_slice(b"pssh");
        pssh.put_u32(0); // Version and flags

        // System ID (Widevine)
        let system_id = hex::decode(self.system_id().replace('-', ""))
            .map_err(|_| PackagerError::DrmFailed("Invalid system ID".to_string()))?;
        pssh.put_slice(&system_id);

        // Key ID count and IDs
        pssh.put_u32(1);
        pssh.put_slice(key_id);

        // Data size and data (empty for now)
        pssh.put_u32(0);

        // Update size
        let size = pssh.len();
        pssh[0..4].copy_from_slice(&(size as u32).to_be_bytes());

        Ok(pssh.to_vec())
    }

    fn license_url(&self) -> Option<String> {
        Some(self.license_url.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let key = KeyGenerator::generate_aes128_key().expect("RNG should succeed in test");
        assert_eq!(key.len(), 16);
    }

    #[test]
    fn test_key_generation_is_random_not_derived_from_timestamp() {
        // Two successive calls must produce different keys (probabilistic
        // uniqueness check for a CSPRNG). With the old timestamp-derived
        // implementation, calls in quick succession could collide or be
        // trivially brute-forced; a CSPRNG must not.
        let key1 = KeyGenerator::generate_aes128_key().expect("RNG should succeed in test");
        let key2 = KeyGenerator::generate_aes128_key().expect("RNG should succeed in test");
        assert_eq!(key1.len(), 16);
        assert_eq!(key2.len(), 16);
        assert_ne!(key1, key2, "successive CSPRNG keys must not collide");
    }

    #[test]
    fn test_iv_generation_is_random() {
        let iv1 = KeyGenerator::generate_iv().expect("RNG should succeed in test");
        let iv2 = KeyGenerator::generate_iv().expect("RNG should succeed in test");
        assert_eq!(iv1.len(), 16);
        assert_eq!(iv2.len(), 16);
        assert_ne!(iv1, iv2, "successive CSPRNG IVs must not collide");
    }

    #[test]
    fn test_from_passphrase_is_deterministic_for_same_salt() {
        let salt = b"a fixed test salt of 16B";
        let key1 = KeyGenerator::from_passphrase("correct horse battery staple", salt)
            .expect("KDF should succeed in test");
        let key2 = KeyGenerator::from_passphrase("correct horse battery staple", salt)
            .expect("KDF should succeed in test");
        assert_eq!(key1.len(), 16);
        assert_eq!(
            key1, key2,
            "PBKDF2 must be deterministic for same passphrase+salt"
        );
    }

    #[test]
    fn test_from_passphrase_differs_for_different_salt() {
        let key1 = KeyGenerator::from_passphrase("correct horse battery staple", b"salt-one")
            .expect("KDF should succeed in test");
        let key2 = KeyGenerator::from_passphrase("correct horse battery staple", b"salt-two")
            .expect("KDF should succeed in test");
        assert_ne!(key1, key2, "different salts must derive different keys");
    }

    #[test]
    fn test_from_passphrase_differs_for_different_passphrase() {
        let salt = b"a fixed test salt of 16B";
        let key1 = KeyGenerator::from_passphrase("password one", salt).expect("KDF should succeed");
        let key2 = KeyGenerator::from_passphrase("password two", salt).expect("KDF should succeed");
        assert_ne!(
            key1, key2,
            "different passphrases must derive different keys"
        );
    }

    #[test]
    fn test_key_info_validation() {
        let key = vec![0u8; 16];
        let iv = vec![0u8; 16];
        let key_info = KeyInfo::new(key, iv);

        assert!(key_info.validate().is_ok());
    }

    #[test]
    fn test_key_info_invalid_key_size() {
        let key = vec![0u8; 8]; // Wrong size
        let iv = vec![0u8; 16];
        let key_info = KeyInfo::new(key, iv);

        assert!(key_info.validate().is_err());
    }

    #[test]
    fn test_encryption_handler_creation() {
        let handler = EncryptionHandler::new(EncryptionMethod::Aes128);
        assert!(handler.is_enabled());
    }

    #[test]
    fn test_hls_key_tag_generation() {
        let key = vec![0u8; 16];
        let iv = vec![0u8; 16];
        let key_info = KeyInfo::new(key, iv).with_uri("https://example.com/key".to_string());

        let mut handler = EncryptionHandler::new(EncryptionMethod::Aes128);
        handler
            .set_key_info(key_info)
            .expect("should succeed in test");

        let tag = handler
            .generate_hls_key_tag()
            .expect("should succeed in test");
        assert!(tag.contains("AES-128"));
        assert!(tag.contains("https://example.com/key"));
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_aes128_encrypt_decrypt_roundtrip() {
        let key = vec![
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        // exactly 32 bytes of plaintext
        let plaintext = b"Hello OxiMedia AES-128 test data";

        let key_info = KeyInfo::new(key, iv);
        let mut handler = EncryptionHandler::new(EncryptionMethod::Aes128);
        handler
            .set_key_info(key_info)
            .expect("set_key_info should succeed in test");

        let ciphertext = handler
            .encrypt(plaintext)
            .expect("encrypt should succeed in test");
        assert_ne!(
            &ciphertext[..plaintext.len()],
            plaintext.as_ref(),
            "ciphertext must differ from plaintext"
        );
        assert_eq!(ciphertext.len() % 16, 0, "ciphertext must be block-aligned");

        let decrypted = handler
            .decrypt(&ciphertext)
            .expect("decrypt should succeed in test");
        assert_eq!(
            &decrypted[..plaintext.len()],
            plaintext.as_ref(),
            "AES-128 round-trip must recover original plaintext"
        );
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_cenc_aes128_ctr_nist_known_answer() {
        // NIST SP 800-38A, F.5.1 CTR-AES128.Encrypt. The 16-byte IV is the
        // initial counter block, incremented as a full 128-bit big-endian
        // integer across the byte boundary (...feff -> ...ff00).
        let key = vec![
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv = vec![
            0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd,
            0xfe, 0xff,
        ];
        let plaintext = vec![
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a, 0xae, 0x2d, 0x8a, 0x57, 0x1e, 0x03, 0xac, 0x9c, 0x9e, 0xb7, 0x6f, 0xac,
            0x45, 0xaf, 0x8e, 0x51, 0x30, 0xc8, 0x1c, 0x46, 0xa3, 0x5c, 0xe4, 0x11, 0xe5, 0xfb,
            0xc1, 0x19, 0x1a, 0x0a, 0x52, 0xef, 0xf6, 0x9f, 0x24, 0x45, 0xdf, 0x4f, 0x9b, 0x17,
            0xad, 0x2b, 0x41, 0x7b, 0xe6, 0x6c, 0x37, 0x10,
        ];
        let expected = vec![
            0x87, 0x4d, 0x61, 0x91, 0xb6, 0x20, 0xe3, 0x26, 0x1b, 0xef, 0x68, 0x64, 0x99, 0x0d,
            0xb6, 0xce, 0x98, 0x06, 0xf6, 0x6b, 0x79, 0x70, 0xfd, 0xff, 0x86, 0x17, 0x18, 0x7b,
            0xb9, 0xff, 0xfd, 0xff, 0x5a, 0xe4, 0xdf, 0x3e, 0xdb, 0xd5, 0xd3, 0x5e, 0x5b, 0x4f,
            0x09, 0x02, 0x0d, 0xb0, 0x3e, 0xab, 0x1e, 0x03, 0x1d, 0xda, 0x2f, 0xbe, 0x03, 0xd1,
            0x79, 0x21, 0x70, 0xa0, 0xf3, 0x00, 0x9c, 0xee,
        ];

        let key_info = KeyInfo::new(key, iv);
        let mut handler = EncryptionHandler::new(EncryptionMethod::Cenc);
        handler
            .set_key_info(key_info)
            .expect("set_key_info should succeed in test");

        let ciphertext = handler.encrypt(&plaintext).expect("cenc encrypt");
        assert_eq!(
            ciphertext, expected,
            "cenc must be real AES-128-CTR (NIST SP 800-38A F.5.1), not CBC"
        );

        let decrypted = handler.decrypt(&ciphertext).expect("cenc decrypt");
        assert_eq!(
            decrypted, plaintext,
            "CTR round-trip must recover plaintext"
        );
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_cenc_ctr_preserves_length_unlike_cbc() {
        // CTR is a stream cipher: ciphertext length == plaintext length, even
        // for non-block-aligned input. Full-buffer CBC (the old behaviour)
        // would pad up to the next 16-byte boundary.
        let key = vec![0x24u8; 16];
        let iv = vec![0x68u8; 16];
        let plaintext = b"cenc CTR keeps exact length"; // 27 bytes, not a multiple of 16

        let key_info = KeyInfo::new(key, iv);
        let mut handler = EncryptionHandler::new(EncryptionMethod::Cenc);
        handler
            .set_key_info(key_info)
            .expect("set_key_info should succeed in test");

        let ciphertext = handler.encrypt(plaintext).expect("cenc encrypt");
        assert_eq!(
            ciphertext.len(),
            plaintext.len(),
            "CTR must not change length"
        );
        assert_ne!(&ciphertext[..], plaintext.as_ref());

        let decrypted = handler.decrypt(&ciphertext).expect("cenc decrypt");
        assert_eq!(decrypted, plaintext.to_vec());
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_sample_aes_cbcs_pattern_and_roundtrip() {
        // 25 whole 16-byte blocks + a 7-byte tail. With the 1:9 pattern only
        // blocks 0, 10, 20 are encrypted; every other whole block and the tail
        // stay clear. This is the defining property the old full-buffer CBC
        // violated (it encrypted everything).
        let key = vec![0x11u8; 16];
        let iv = vec![0x22u8; 16];
        let total = 25 * 16 + 7;
        let plaintext: Vec<u8> = (0..total).map(|i| (i as u8).wrapping_mul(7)).collect();

        let key_info = KeyInfo::new(key, iv);
        let mut handler = EncryptionHandler::new(EncryptionMethod::SampleAes);
        handler
            .set_key_info(key_info)
            .expect("set_key_info should succeed in test");

        let ciphertext = handler.encrypt(&plaintext).expect("cbcs encrypt");
        assert_eq!(
            ciphertext.len(),
            plaintext.len(),
            "cbcs pattern encryption must preserve length (no padding)"
        );

        for block_idx in 0..25 {
            let range = block_idx * 16..block_idx * 16 + 16;
            if block_idx % 10 == 0 {
                assert_ne!(
                    ciphertext[range.clone()],
                    plaintext[range],
                    "crypt block {block_idx} must be encrypted"
                );
            } else {
                assert_eq!(
                    ciphertext[range.clone()],
                    plaintext[range],
                    "skip block {block_idx} must remain clear"
                );
            }
        }
        assert_eq!(
            ciphertext[25 * 16..],
            plaintext[25 * 16..],
            "trailing partial block must remain clear"
        );

        let decrypted = handler.decrypt(&ciphertext).expect("cbcs decrypt");
        assert_eq!(
            decrypted, plaintext,
            "cbcs pattern round-trip must recover plaintext"
        );
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_sample_aes_differs_from_full_cbc() {
        // Guard against a regression to full-buffer CBC: with input longer than
        // one pattern cycle, at least one whole block must remain identical to
        // the plaintext (a skipped block), which full-buffer CBC never produces.
        let key = vec![0x33u8; 16];
        let iv = vec![0x44u8; 16];
        let plaintext = vec![0xA5u8; 16 * 12]; // 12 whole blocks > 1 cycle (10)

        let key_info = KeyInfo::new(key, iv);
        let mut handler = EncryptionHandler::new(EncryptionMethod::SampleAes);
        handler
            .set_key_info(key_info)
            .expect("set_key_info should succeed in test");

        let ciphertext = handler.encrypt(&plaintext).expect("cbcs encrypt");
        let has_clear_block =
            (0..12).any(|b| ciphertext[b * 16..b * 16 + 16] == plaintext[b * 16..b * 16 + 16]);
        assert!(
            has_clear_block,
            "cbcs must leave skip blocks in the clear (not full-buffer CBC)"
        );
    }
}
