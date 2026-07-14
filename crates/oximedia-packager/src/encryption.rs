//! Encryption support for adaptive streaming.

use crate::config::EncryptionMethod;
use crate::error::{PackagerError, PackagerResult};
use bytes::{BufMut, BytesMut};
use rand::TryRng;

#[cfg(feature = "encryption")]
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
#[cfg(feature = "encryption")]
use aes::Aes128;
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

    /// Encrypt with SAMPLE-AES.
    fn encrypt_sample_aes(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        // SAMPLE-AES encrypts only certain samples, not the entire stream
        // This is a simplified implementation
        self.encrypt_aes128(data)
    }

    /// Decrypt with SAMPLE-AES.
    fn decrypt_sample_aes(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        self.decrypt_aes128(data)
    }

    /// Encrypt with Common Encryption (CENC).
    fn encrypt_cenc(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        // CENC uses AES-128 CTR mode
        // This is a placeholder implementation
        self.encrypt_aes128(data)
    }

    /// Decrypt with Common Encryption (CENC).
    fn decrypt_cenc(&self, data: &[u8]) -> PackagerResult<Vec<u8>> {
        self.decrypt_aes128(data)
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
}
