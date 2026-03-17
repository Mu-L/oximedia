//! Pure-Rust AES-128-CBC and AES-256-CBC encryption / decryption.
//!
//! Implements AES-CBC mode on top of the same block cipher used in `aes_ctr`.
//! Supports PKCS#7 padding.  Required by the CENC `cbcs` scheme (FairPlay).

use crate::aes_ctr::{encrypt_block, key_expand_128, key_expand_256};

// ---------------------------------------------------------------------------
// AES block *decryption* (inverse cipher)
// ---------------------------------------------------------------------------

/// AES inverse S-box (256-byte lookup table).
static INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

/// GF(2^8) multiply helper (same as aes_ctr but we need it here for InvMixColumns).
#[inline]
fn gf_mul(a: u8, b: u8) -> u8 {
    let mut result = 0u8;
    let mut aa = a;
    let mut bb = b;
    for _ in 0..8 {
        if bb & 1 != 0 {
            result ^= aa;
        }
        let hi = aa & 0x80;
        aa <<= 1;
        if hi != 0 {
            aa ^= 0x1b;
        }
        bb >>= 1;
    }
    result
}

#[inline]
fn word_byte(w: u32, i: usize) -> u8 {
    ((w >> (24 - 8 * i)) & 0xff) as u8
}

#[inline]
fn bytes_to_word(b0: u8, b1: u8, b2: u8, b3: u8) -> u32 {
    ((b0 as u32) << 24) | ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32)
}

fn inv_sub_bytes(state: &mut [u32; 4]) {
    for w in state.iter_mut() {
        let b0 = INV_SBOX[word_byte(*w, 0) as usize];
        let b1 = INV_SBOX[word_byte(*w, 1) as usize];
        let b2 = INV_SBOX[word_byte(*w, 2) as usize];
        let b3 = INV_SBOX[word_byte(*w, 3) as usize];
        *w = bytes_to_word(b0, b1, b2, b3);
    }
}

fn inv_shift_rows(state: &mut [u32; 4]) {
    let mut grid = [[0u8; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            grid[row][col] = word_byte(state[col], row);
        }
    }
    for row in 1..4 {
        grid[row].rotate_right(row);
    }
    for col in 0..4 {
        state[col] = bytes_to_word(grid[0][col], grid[1][col], grid[2][col], grid[3][col]);
    }
}

fn inv_mix_columns(state: &mut [u32; 4]) {
    for w in state.iter_mut() {
        let s0 = word_byte(*w, 0);
        let s1 = word_byte(*w, 1);
        let s2 = word_byte(*w, 2);
        let s3 = word_byte(*w, 3);

        let r0 = gf_mul(0x0e, s0) ^ gf_mul(0x0b, s1) ^ gf_mul(0x0d, s2) ^ gf_mul(0x09, s3);
        let r1 = gf_mul(0x09, s0) ^ gf_mul(0x0e, s1) ^ gf_mul(0x0b, s2) ^ gf_mul(0x0d, s3);
        let r2 = gf_mul(0x0d, s0) ^ gf_mul(0x09, s1) ^ gf_mul(0x0e, s2) ^ gf_mul(0x0b, s3);
        let r3 = gf_mul(0x0b, s0) ^ gf_mul(0x0d, s1) ^ gf_mul(0x09, s2) ^ gf_mul(0x0e, s3);

        *w = bytes_to_word(r0, r1, r2, r3);
    }
}

fn add_round_key(state: &mut [u32; 4], round_key: &[u32; 4]) {
    for (s, k) in state.iter_mut().zip(round_key.iter()) {
        *s ^= k;
    }
}

/// Decrypt a single 16-byte AES block using the provided key schedule
/// (the same forward key schedule produced by `key_expand_128` / `key_expand_256`).
pub fn decrypt_block(key_schedule: &[[u32; 4]], ciphertext: &[u8; 16]) -> [u8; 16] {
    let nr = key_schedule.len() - 1;

    let mut state = [0u32; 4];
    for col in 0..4 {
        state[col] = bytes_to_word(
            ciphertext[col * 4],
            ciphertext[col * 4 + 1],
            ciphertext[col * 4 + 2],
            ciphertext[col * 4 + 3],
        );
    }

    add_round_key(&mut state, &key_schedule[nr]);

    for round in (1..nr).rev() {
        inv_shift_rows(&mut state);
        inv_sub_bytes(&mut state);
        add_round_key(&mut state, &key_schedule[round]);
        inv_mix_columns(&mut state);
    }

    inv_shift_rows(&mut state);
    inv_sub_bytes(&mut state);
    add_round_key(&mut state, &key_schedule[0]);

    let mut output = [0u8; 16];
    for col in 0..4 {
        output[col * 4] = word_byte(state[col], 0);
        output[col * 4 + 1] = word_byte(state[col], 1);
        output[col * 4 + 2] = word_byte(state[col], 2);
        output[col * 4 + 3] = word_byte(state[col], 3);
    }
    output
}

// ---------------------------------------------------------------------------
// AES-CBC error type
// ---------------------------------------------------------------------------

use thiserror::Error;

/// Errors specific to AES-CBC operations.
#[derive(Error, Debug)]
pub enum AesCbcError {
    #[error("invalid key length: expected 16 or 32 bytes, got {0}")]
    InvalidKeyLength(usize),

    #[error("invalid IV length: expected 16 bytes, got {0}")]
    InvalidIvLength(usize),

    #[error("ciphertext length ({0}) is not a multiple of 16")]
    InvalidCiphertextLength(usize),

    #[error("invalid PKCS#7 padding")]
    InvalidPadding,
}

// ---------------------------------------------------------------------------
// AesCbc cipher
// ---------------------------------------------------------------------------

/// AES-CBC cipher for 128-bit or 256-bit keys with PKCS#7 padding.
#[derive(Debug, Clone)]
pub struct AesCbc {
    key_size_bits: usize,
    round_keys: Vec<[u32; 4]>,
}

impl AesCbc {
    /// Create a new AES-128-CBC cipher from a 16-byte key.
    pub fn new_128(key: &[u8; 16]) -> Self {
        Self {
            key_size_bits: 128,
            round_keys: key_expand_128(key),
        }
    }

    /// Create a new AES-256-CBC cipher from a 32-byte key.
    pub fn new_256(key: &[u8; 32]) -> Self {
        Self {
            key_size_bits: 256,
            round_keys: key_expand_256(key),
        }
    }

    /// Create from a variable-length key slice.
    pub fn from_key(key: &[u8]) -> Result<Self, AesCbcError> {
        match key.len() {
            16 => {
                let mut k = [0u8; 16];
                k.copy_from_slice(key);
                Ok(Self::new_128(&k))
            }
            32 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(key);
                Ok(Self::new_256(&k))
            }
            other => Err(AesCbcError::InvalidKeyLength(other)),
        }
    }

    /// Return the key size in bits.
    pub fn key_size_bits(&self) -> usize {
        self.key_size_bits
    }

    /// Encrypt `plaintext` in CBC mode with PKCS#7 padding.
    ///
    /// `iv` must be exactly 16 bytes.
    pub fn encrypt(&self, plaintext: &[u8], iv: &[u8]) -> Result<Vec<u8>, AesCbcError> {
        if iv.len() != 16 {
            return Err(AesCbcError::InvalidIvLength(iv.len()));
        }

        // Apply PKCS#7 padding
        let pad_len = 16 - (plaintext.len() % 16);
        let padded_len = plaintext.len() + pad_len;
        let mut padded = Vec::with_capacity(padded_len);
        padded.extend_from_slice(plaintext);
        padded.resize(padded_len, pad_len as u8);

        let mut output = Vec::with_capacity(padded_len);
        let mut prev_block = [0u8; 16];
        prev_block.copy_from_slice(iv);

        for chunk in padded.chunks_exact(16) {
            // XOR with previous ciphertext block (or IV)
            let mut input_block = [0u8; 16];
            for i in 0..16 {
                input_block[i] = chunk[i] ^ prev_block[i];
            }

            let cipher_block = encrypt_block(&self.round_keys, &input_block);
            output.extend_from_slice(&cipher_block);
            prev_block = cipher_block;
        }

        Ok(output)
    }

    /// Decrypt `ciphertext` in CBC mode and remove PKCS#7 padding.
    ///
    /// `iv` must be exactly 16 bytes. `ciphertext` length must be a multiple of 16.
    pub fn decrypt(&self, ciphertext: &[u8], iv: &[u8]) -> Result<Vec<u8>, AesCbcError> {
        if iv.len() != 16 {
            return Err(AesCbcError::InvalidIvLength(iv.len()));
        }
        if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
            return Err(AesCbcError::InvalidCiphertextLength(ciphertext.len()));
        }

        let mut output = Vec::with_capacity(ciphertext.len());
        let mut prev_block = [0u8; 16];
        prev_block.copy_from_slice(iv);

        for chunk in ciphertext.chunks_exact(16) {
            let mut ct_block = [0u8; 16];
            ct_block.copy_from_slice(chunk);

            let decrypted = decrypt_block(&self.round_keys, &ct_block);

            // XOR with previous ciphertext block (or IV)
            for i in 0..16 {
                output.push(decrypted[i] ^ prev_block[i]);
            }
            prev_block = ct_block;
        }

        // Remove PKCS#7 padding
        let pad_byte = output.last().copied().ok_or(AesCbcError::InvalidPadding)?;
        let pad_len = pad_byte as usize;
        if pad_len == 0 || pad_len > 16 || pad_len > output.len() {
            return Err(AesCbcError::InvalidPadding);
        }
        let pad_start = output.len() - pad_len;
        if !output[pad_start..].iter().all(|&b| b == pad_byte) {
            return Err(AesCbcError::InvalidPadding);
        }
        output.truncate(pad_start);

        Ok(output)
    }

    /// Encrypt without PKCS#7 padding. Caller must ensure `plaintext.len()` is a
    /// multiple of 16.  This is used by the CENC `cbcs` pattern encryption where
    /// each block is exactly 16 bytes and no padding is desired.
    pub fn encrypt_no_padding(&self, plaintext: &[u8], iv: &[u8]) -> Result<Vec<u8>, AesCbcError> {
        if iv.len() != 16 {
            return Err(AesCbcError::InvalidIvLength(iv.len()));
        }
        if plaintext.len() % 16 != 0 {
            return Err(AesCbcError::InvalidCiphertextLength(plaintext.len()));
        }

        let mut output = Vec::with_capacity(plaintext.len());
        let mut prev_block = [0u8; 16];
        prev_block.copy_from_slice(iv);

        for chunk in plaintext.chunks_exact(16) {
            let mut input_block = [0u8; 16];
            for i in 0..16 {
                input_block[i] = chunk[i] ^ prev_block[i];
            }
            let cipher_block = encrypt_block(&self.round_keys, &input_block);
            output.extend_from_slice(&cipher_block);
            prev_block = cipher_block;
        }

        Ok(output)
    }

    /// Decrypt without PKCS#7 unpadding (inverse of `encrypt_no_padding`).
    pub fn decrypt_no_padding(&self, ciphertext: &[u8], iv: &[u8]) -> Result<Vec<u8>, AesCbcError> {
        if iv.len() != 16 {
            return Err(AesCbcError::InvalidIvLength(iv.len()));
        }
        if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
            return Err(AesCbcError::InvalidCiphertextLength(ciphertext.len()));
        }

        let mut output = Vec::with_capacity(ciphertext.len());
        let mut prev_block = [0u8; 16];
        prev_block.copy_from_slice(iv);

        for chunk in ciphertext.chunks_exact(16) {
            let mut ct_block = [0u8; 16];
            ct_block.copy_from_slice(chunk);
            let decrypted = decrypt_block(&self.round_keys, &ct_block);
            for i in 0..16 {
                output.push(decrypted[i] ^ prev_block[i]);
            }
            prev_block = ct_block;
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ----- decrypt_block correctness ----------------------------------------

    #[test]
    fn test_aes128_encrypt_decrypt_block_roundtrip() {
        let key = [
            0x2bu8, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let plaintext: [u8; 16] = [
            0x32, 0x43, 0xf6, 0xa8, 0x88, 0x5a, 0x30, 0x8d, 0x31, 0x31, 0x98, 0xa2, 0xe0, 0x37,
            0x07, 0x34,
        ];
        let schedule = key_expand_128(&key);
        let ciphertext = encrypt_block(&schedule, &plaintext);
        let recovered = decrypt_block(&schedule, &ciphertext);
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn test_aes256_encrypt_decrypt_block_roundtrip() {
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let plaintext: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let schedule = key_expand_256(&key);
        let ciphertext = encrypt_block(&schedule, &plaintext);
        let recovered = decrypt_block(&schedule, &ciphertext);
        assert_eq!(recovered, plaintext);
    }

    // ----- AES-CBC with PKCS#7 padding roundtrips ----------------------------

    #[test]
    fn test_cbc128_roundtrip_single_block() {
        let key = [0x42u8; 16];
        let iv = [0x00u8; 16];
        let plaintext = b"Exactly16Bytes!"; // 15 bytes
        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt(plaintext, &iv)
            .expect("encrypt should succeed");
        // With PKCS#7: 15 bytes + 1 byte padding = 16 bytes ciphertext
        assert_eq!(ct.len(), 16);
        let pt = cipher.decrypt(&ct, &iv).expect("decrypt should succeed");
        assert_eq!(pt, plaintext.to_vec());
    }

    #[test]
    fn test_cbc128_roundtrip_multi_block() {
        let key = [0xABu8; 16];
        let iv = [0xCDu8; 16];
        let plaintext = b"Hello AES-128-CBC mode with multiple blocks of data for testing!";
        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt(plaintext, &iv)
            .expect("encrypt should succeed");
        let pt = cipher.decrypt(&ct, &iv).expect("decrypt should succeed");
        assert_eq!(pt, plaintext.to_vec());
    }

    #[test]
    fn test_cbc256_roundtrip() {
        let key = [0xDEu8; 32];
        let iv = [0x11u8; 16];
        let plaintext = b"AES-256-CBC encryption round-trip test with PKCS7 padding";
        let cipher = AesCbc::new_256(&key);
        let ct = cipher
            .encrypt(plaintext, &iv)
            .expect("encrypt should succeed");
        let pt = cipher.decrypt(&ct, &iv).expect("decrypt should succeed");
        assert_eq!(pt, plaintext.to_vec());
    }

    #[test]
    fn test_cbc_empty_plaintext() {
        let key = [0x00u8; 16];
        let iv = [0x00u8; 16];
        let cipher = AesCbc::new_128(&key);
        let ct = cipher.encrypt(b"", &iv).expect("encrypt should succeed");
        assert_eq!(ct.len(), 16); // one full padding block
        let pt = cipher.decrypt(&ct, &iv).expect("decrypt should succeed");
        assert!(pt.is_empty());
    }

    #[test]
    fn test_cbc_different_iv_different_ciphertext() {
        let key = [0x77u8; 16];
        let plaintext = b"Same plaintext different IVs";
        let cipher = AesCbc::new_128(&key);
        let ct1 = cipher
            .encrypt(plaintext, &[0x00; 16])
            .expect("encrypt should succeed");
        let ct2 = cipher
            .encrypt(plaintext, &[0x01; 16])
            .expect("encrypt should succeed");
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_cbc_invalid_iv_length() {
        let key = [0x00u8; 16];
        let cipher = AesCbc::new_128(&key);
        let result = cipher.encrypt(b"data", &[0u8; 8]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cbc_invalid_ciphertext_length_for_decrypt() {
        let key = [0x00u8; 16];
        let cipher = AesCbc::new_128(&key);
        let result = cipher.decrypt(&[0u8; 15], &[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cbc_from_key_128() {
        let key = vec![0xAAu8; 16];
        let cipher = AesCbc::from_key(&key).expect("from_key should succeed");
        assert_eq!(cipher.key_size_bits(), 128);
    }

    #[test]
    fn test_cbc_from_key_256() {
        let key = vec![0xBBu8; 32];
        let cipher = AesCbc::from_key(&key).expect("from_key should succeed");
        assert_eq!(cipher.key_size_bits(), 256);
    }

    #[test]
    fn test_cbc_from_key_invalid() {
        let key = vec![0xCCu8; 24];
        assert!(AesCbc::from_key(&key).is_err());
    }

    // ----- No-padding mode (for CENC cbcs) -----------------------------------

    #[test]
    fn test_cbc_no_padding_roundtrip() {
        let key = [0x55u8; 16];
        let iv = [0x33u8; 16];
        // Must be a multiple of 16
        let plaintext = [0xABu8; 48];
        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt_no_padding(&plaintext, &iv)
            .expect("encrypt should succeed");
        assert_eq!(ct.len(), 48);
        let pt = cipher
            .decrypt_no_padding(&ct, &iv)
            .expect("decrypt should succeed");
        assert_eq!(pt, plaintext.to_vec());
    }

    #[test]
    fn test_cbc_no_padding_rejects_non_multiple() {
        let key = [0x00u8; 16];
        let iv = [0x00u8; 16];
        let cipher = AesCbc::new_128(&key);
        let result = cipher.encrypt_no_padding(&[0u8; 17], &iv);
        assert!(result.is_err());
    }

    #[test]
    fn test_cbc128_nist_known_answer() {
        // NIST SP 800-38A F.2.1 CBC-AES128.Encrypt
        let key: [u8; 16] = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv: [u8; 16] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let plaintext: [u8; 64] = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a, 0xae, 0x2d, 0x8a, 0x57, 0x1e, 0x03, 0xac, 0x9c, 0x9e, 0xb7, 0x6f, 0xac,
            0x45, 0xaf, 0x8e, 0x51, 0x30, 0xc8, 0x1c, 0x46, 0xa3, 0x5c, 0xe4, 0x11, 0xe5, 0xfb,
            0xc1, 0x19, 0x1a, 0x0a, 0x52, 0xef, 0xf6, 0x9f, 0x24, 0x45, 0xdf, 0x4f, 0x9b, 0x17,
            0xad, 0x2b, 0x41, 0x7b, 0xe6, 0x6c, 0x37, 0x10,
        ];
        let expected_ct: [u8; 64] = [
            0x76, 0x49, 0xab, 0xac, 0x81, 0x19, 0xb2, 0x46, 0xce, 0xe9, 0x8e, 0x9b, 0x12, 0xe9,
            0x19, 0x7d, 0x50, 0x86, 0xcb, 0x9b, 0x50, 0x72, 0x19, 0xee, 0x95, 0xdb, 0x11, 0x3a,
            0x91, 0x76, 0x78, 0xb2, 0x73, 0xbe, 0xd6, 0xb8, 0xe3, 0xc1, 0x74, 0x3b, 0x71, 0x16,
            0xe6, 0x9e, 0x22, 0x22, 0x95, 0x16, 0x3f, 0xf1, 0xca, 0xa1, 0x68, 0x1f, 0xac, 0x09,
            0x12, 0x0e, 0xca, 0x30, 0x75, 0x86, 0xe1, 0xa7,
        ];

        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt_no_padding(&plaintext, &iv)
            .expect("encrypt should succeed");
        assert_eq!(ct, expected_ct.to_vec(), "NIST CBC-AES128 encrypt failed");

        let pt = cipher
            .decrypt_no_padding(&ct, &iv)
            .expect("decrypt should succeed");
        assert_eq!(pt, plaintext.to_vec(), "NIST CBC-AES128 decrypt failed");
    }

    #[test]
    fn test_cbc_large_data_roundtrip() {
        let key = [0x5au8; 16];
        let iv = [0x3cu8; 16];
        let plaintext: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt(&plaintext, &iv)
            .expect("encrypt should succeed");
        let pt = cipher.decrypt(&ct, &iv).expect("decrypt should succeed");
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn test_cbc_ciphertext_differs_from_plaintext() {
        let key = [0xFFu8; 16];
        let iv = [0xEEu8; 16];
        let plaintext = b"This should be encrypted not plain!";
        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt(plaintext, &iv)
            .expect("encrypt should succeed");
        assert_ne!(ct, plaintext.to_vec());
    }

    #[test]
    fn test_cbc_padding_one_byte_short() {
        // 15 bytes of plaintext -> 1 byte padding
        let key = [0x12u8; 16];
        let iv = [0x34u8; 16];
        let plaintext = [0xAA; 15];
        let cipher = AesCbc::new_128(&key);
        let ct = cipher
            .encrypt(&plaintext, &iv)
            .expect("encrypt should succeed");
        assert_eq!(ct.len(), 16);
        let pt = cipher.decrypt(&ct, &iv).expect("decrypt should succeed");
        assert_eq!(pt, plaintext.to_vec());
    }

    #[test]
    fn test_decrypt_block_inverse_of_encrypt_block() {
        // Verify with all-zero key and all-zero plaintext
        let key = [0u8; 16];
        let schedule = key_expand_128(&key);
        let pt = [0u8; 16];
        let ct = encrypt_block(&schedule, &pt);
        // ct should not equal pt (AES of zeros is not zeros)
        assert_ne!(ct, pt);
        let recovered = decrypt_block(&schedule, &ct);
        assert_eq!(recovered, pt);
    }
}
