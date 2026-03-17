//! Pure-Rust AES-128-CTR and AES-256-CTR encryption.
//!
//! Implements the full AES block cipher from scratch (no external crypto crates).
//! Supports 128-bit and 256-bit key sizes in CTR mode.

// ---------------------------------------------------------------------------
// AES S-box and inverse S-box (NIST FIPS 197)
// ---------------------------------------------------------------------------

/// AES forward S-box (256-byte lookup table).
static SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// AES inverse S-box (256-byte lookup table).
#[cfg(test)]
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

// ---------------------------------------------------------------------------
// AES round constants (Rcon)
// ---------------------------------------------------------------------------

/// AES round constants for key expansion.
static RCON: [u32; 10] = [
    0x01000000, 0x02000000, 0x04000000, 0x08000000, 0x10000000, 0x20000000, 0x40000000, 0x80000000,
    0x1b000000, 0x36000000,
];

// ---------------------------------------------------------------------------
// GF(2^8) multiply helper
// ---------------------------------------------------------------------------

/// Multiply two elements in GF(2^8) with the AES irreducible polynomial.
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
            aa ^= 0x1b; // x^8 + x^4 + x^3 + x + 1
        }
        bb >>= 1;
    }
    result
}

// ---------------------------------------------------------------------------
// AES state manipulation functions
// ---------------------------------------------------------------------------

/// Extract byte i from a u32 word (big-endian).
#[inline]
fn word_byte(w: u32, i: usize) -> u8 {
    ((w >> (24 - 8 * i)) & 0xff) as u8
}

/// Build a u32 from 4 bytes (big-endian).
#[inline]
fn bytes_to_word(b0: u8, b1: u8, b2: u8, b3: u8) -> u32 {
    ((b0 as u32) << 24) | ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32)
}

/// Apply SubBytes to the 4x4 state (stored as 4 u32 words, each word = one column).
pub fn sub_bytes(state: &mut [u32; 4]) {
    for w in state.iter_mut() {
        let b0 = SBOX[word_byte(*w, 0) as usize];
        let b1 = SBOX[word_byte(*w, 1) as usize];
        let b2 = SBOX[word_byte(*w, 2) as usize];
        let b3 = SBOX[word_byte(*w, 3) as usize];
        *w = bytes_to_word(b0, b1, b2, b3);
    }
}

/// Apply ShiftRows to the 4x4 state.
///
/// The state is stored column-major: `state[col]` = (row0, row1, row2, row3).
/// ShiftRows shifts row i left by i bytes across columns.
pub fn shift_rows(state: &mut [u32; 4]) {
    // Extract the 4x4 byte grid (row-major)
    let mut grid = [[0u8; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            grid[row][col] = word_byte(state[col], row);
        }
    }

    // Shift row i by i positions to the left
    for row in 1..4 {
        grid[row].rotate_left(row);
    }

    // Write back column-major
    for col in 0..4 {
        state[col] = bytes_to_word(grid[0][col], grid[1][col], grid[2][col], grid[3][col]);
    }
}

/// Apply MixColumns to the 4x4 state (one column at a time).
pub fn mix_columns(state: &mut [u32; 4]) {
    for w in state.iter_mut() {
        let s0 = word_byte(*w, 0);
        let s1 = word_byte(*w, 1);
        let s2 = word_byte(*w, 2);
        let s3 = word_byte(*w, 3);

        let r0 = gf_mul(0x02, s0) ^ gf_mul(0x03, s1) ^ s2 ^ s3;
        let r1 = s0 ^ gf_mul(0x02, s1) ^ gf_mul(0x03, s2) ^ s3;
        let r2 = s0 ^ s1 ^ gf_mul(0x02, s2) ^ gf_mul(0x03, s3);
        let r3 = gf_mul(0x03, s0) ^ s1 ^ s2 ^ gf_mul(0x02, s3);

        *w = bytes_to_word(r0, r1, r2, r3);
    }
}

/// XOR the round key into the state.
pub fn add_round_key(state: &mut [u32; 4], round_key: &[u32; 4]) {
    for (s, k) in state.iter_mut().zip(round_key.iter()) {
        *s ^= k;
    }
}

// ---------------------------------------------------------------------------
// Key expansion
// ---------------------------------------------------------------------------

/// SubWord: apply S-box to each byte of a 32-bit word.
#[inline]
fn sub_word(w: u32) -> u32 {
    bytes_to_word(
        SBOX[word_byte(w, 0) as usize],
        SBOX[word_byte(w, 1) as usize],
        SBOX[word_byte(w, 2) as usize],
        SBOX[word_byte(w, 3) as usize],
    )
}

/// RotWord: rotate a 32-bit word left by 8 bits.
#[inline]
fn rot_word(w: u32) -> u32 {
    w.rotate_left(8)
}

/// Expand a 128-bit key into 11 round keys (each is [u32; 4]).
pub fn key_expand_128(key: &[u8; 16]) -> Vec<[u32; 4]> {
    let nk = 4usize; // words in key
    let nr = 10usize; // rounds
    let total_words = 4 * (nr + 1);

    let mut w = Vec::with_capacity(total_words);

    // Load key words
    for i in 0..nk {
        w.push(bytes_to_word(
            key[4 * i],
            key[4 * i + 1],
            key[4 * i + 2],
            key[4 * i + 3],
        ));
    }

    for i in nk..total_words {
        let mut temp = w[i - 1];
        if i % nk == 0 {
            temp = sub_word(rot_word(temp)) ^ RCON[i / nk - 1];
        }
        w.push(w[i - nk] ^ temp);
    }

    // Group into 4-word round keys
    let mut round_keys = Vec::with_capacity(nr + 1);
    for r in 0..=(nr) {
        round_keys.push([w[4 * r], w[4 * r + 1], w[4 * r + 2], w[4 * r + 3]]);
    }
    round_keys
}

/// Expand a 256-bit key into 15 round keys (each is [u32; 4]).
pub fn key_expand_256(key: &[u8; 32]) -> Vec<[u32; 4]> {
    let nk = 8usize; // words in key
    let nr = 14usize; // rounds
    let total_words = 4 * (nr + 1);

    let mut w = Vec::with_capacity(total_words);

    // Load key words
    for i in 0..nk {
        w.push(bytes_to_word(
            key[4 * i],
            key[4 * i + 1],
            key[4 * i + 2],
            key[4 * i + 3],
        ));
    }

    for i in nk..total_words {
        let mut temp = w[i - 1];
        if i % nk == 0 {
            temp = sub_word(rot_word(temp)) ^ RCON[i / nk - 1];
        } else if i % nk == 4 {
            temp = sub_word(temp);
        }
        w.push(w[i - nk] ^ temp);
    }

    // Group into 4-word round keys
    let mut round_keys = Vec::with_capacity(nr + 1);
    for r in 0..=(nr) {
        round_keys.push([w[4 * r], w[4 * r + 1], w[4 * r + 2], w[4 * r + 3]]);
    }
    round_keys
}

// ---------------------------------------------------------------------------
// AES block encryption
// ---------------------------------------------------------------------------

/// Encrypt a single 16-byte AES block using the provided key schedule.
///
/// Supports 128-bit (11 round keys) and 256-bit (15 round keys) key sizes.
pub fn encrypt_block(key_schedule: &[[u32; 4]], plaintext: &[u8; 16]) -> [u8; 16] {
    let nr = key_schedule.len() - 1; // 10 for AES-128, 14 for AES-256

    // Load plaintext into state (column-major)
    let mut state = [0u32; 4];
    for col in 0..4 {
        state[col] = bytes_to_word(
            plaintext[col * 4],
            plaintext[col * 4 + 1],
            plaintext[col * 4 + 2],
            plaintext[col * 4 + 3],
        );
    }

    // Initial round key addition
    add_round_key(&mut state, &key_schedule[0]);

    // Main rounds
    for round in 1..nr {
        sub_bytes(&mut state);
        shift_rows(&mut state);
        mix_columns(&mut state);
        add_round_key(&mut state, &key_schedule[round]);
    }

    // Final round (no MixColumns)
    sub_bytes(&mut state);
    shift_rows(&mut state);
    add_round_key(&mut state, &key_schedule[nr]);

    // Convert state back to bytes
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
// CTR mode state
// ---------------------------------------------------------------------------

/// Counter state for AES-CTR mode: 64-bit nonce + 64-bit counter = 128-bit counter block.
#[derive(Debug, Clone)]
pub struct CtrState {
    /// 64-bit nonce (high 8 bytes of the 128-bit counter block).
    pub nonce: [u8; 8],
    /// 64-bit counter (low 8 bytes of the 128-bit counter block).
    pub counter: u64,
}

impl CtrState {
    /// Create a new CTR state with the given nonce and counter starting at 0.
    pub fn new(nonce: [u8; 8]) -> Self {
        Self { nonce, counter: 0 }
    }

    /// Produce the next 16-byte counter block: nonce (8 bytes) || counter (8 bytes, big-endian).
    /// Increments the internal counter after returning the current block.
    pub fn next_block(&mut self) -> [u8; 16] {
        let mut block = [0u8; 16];
        block[0..8].copy_from_slice(&self.nonce);
        block[8..16].copy_from_slice(&self.counter.to_be_bytes());
        self.counter = self.counter.wrapping_add(1);
        block
    }
}

// ---------------------------------------------------------------------------
// AesCtr structure
// ---------------------------------------------------------------------------

/// AES-CTR cipher for 128-bit or 256-bit keys.
#[derive(Debug, Clone)]
pub struct AesCtr {
    /// Key size in bits: 128 or 256.
    pub key_size_bits: usize,
    /// Expanded key schedule: 11 round keys for AES-128, 15 for AES-256.
    round_keys: Vec<[u32; 4]>,
}

impl AesCtr {
    /// Create a new AES-128-CTR cipher from a 16-byte key.
    pub fn new_128(key: &[u8; 16]) -> Self {
        Self {
            key_size_bits: 128,
            round_keys: key_expand_128(key),
        }
    }

    /// Create a new AES-256-CTR cipher from a 32-byte key.
    pub fn new_256(key: &[u8; 32]) -> Self {
        Self {
            key_size_bits: 256,
            round_keys: key_expand_256(key),
        }
    }

    /// Encrypt `data` in CTR mode.
    ///
    /// The keystream is generated by encrypting `nonce || counter` blocks,
    /// where `counter` starts at `counter_start` and increments by 1 per 16-byte block.
    pub fn encrypt(&self, data: &[u8], nonce: &[u8; 8], counter_start: u64) -> Vec<u8> {
        self.apply_keystream(data, nonce, counter_start)
    }

    /// Decrypt `data` in CTR mode (identical to encryption — CTR is symmetric).
    pub fn decrypt(&self, data: &[u8], nonce: &[u8; 8], counter_start: u64) -> Vec<u8> {
        self.apply_keystream(data, nonce, counter_start)
    }

    /// Core CTR keystream XOR operation.
    fn apply_keystream(&self, data: &[u8], nonce: &[u8; 8], counter_start: u64) -> Vec<u8> {
        let mut output = Vec::with_capacity(data.len());
        let mut counter = counter_start;

        let mut offset = 0;
        while offset < data.len() {
            // Build counter block: nonce (8 bytes) || counter (8 bytes big-endian)
            let mut block = [0u8; 16];
            block[0..8].copy_from_slice(nonce);
            block[8..16].copy_from_slice(&counter.to_be_bytes());

            // Encrypt counter block to produce keystream block
            let keystream = encrypt_block(&self.round_keys, &block);

            // XOR keystream with plaintext/ciphertext
            let chunk_len = (data.len() - offset).min(16);
            for i in 0..chunk_len {
                output.push(data[offset + i] ^ keystream[i]);
            }

            offset += chunk_len;
            counter = counter.wrapping_add(1);
        }

        output
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // NIST FIPS 197 Appendix B test vector
    // Key:        2b 7e 15 16 28 ae d2 a6 ab f7 15 88 09 cf 4f 3c
    // Plaintext:  32 43 f6 a8 88 5a 30 8d 31 31 98 a2 e0 37 07 34
    // Ciphertext: 39 25 84 1d 02 dc 09 fb dc 11 85 97 19 6a 0b 32
    // -----------------------------------------------------------------------

    #[test]
    fn test_aes128_nist_fips197_appendix_b() {
        let key: [u8; 16] = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let plaintext: [u8; 16] = [
            0x32, 0x43, 0xf6, 0xa8, 0x88, 0x5a, 0x30, 0x8d, 0x31, 0x31, 0x98, 0xa2, 0xe0, 0x37,
            0x07, 0x34,
        ];
        let expected: [u8; 16] = [
            0x39, 0x25, 0x84, 0x1d, 0x02, 0xdc, 0x09, 0xfb, 0xdc, 0x11, 0x85, 0x97, 0x19, 0x6a,
            0x0b, 0x32,
        ];
        let schedule = key_expand_128(&key);
        let ciphertext = encrypt_block(&schedule, &plaintext);
        assert_eq!(
            ciphertext, expected,
            "NIST FIPS 197 Appendix B test vector failed"
        );
    }

    // NIST AES-256 test vector (ECB mode, from FIPS 197 Appendix B / NIST AESAVS)
    // Key:        00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f
    //             10 11 12 13 14 15 16 17 18 19 1a 1b 1c 1d 1e 1f
    // Plaintext:  00 11 22 33 44 55 66 77 88 99 aa bb cc dd ee ff
    // Ciphertext: 8e a2 b7 ca 51 67 45 bf ea fc 49 90 4b 49 60 89
    #[test]
    fn test_aes256_nist_vector() {
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let plaintext: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let expected: [u8; 16] = [
            0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
            0x60, 0x89,
        ];
        let schedule = key_expand_256(&key);
        let ciphertext = encrypt_block(&schedule, &plaintext);
        assert_eq!(ciphertext, expected, "AES-256 NIST vector failed");
    }

    #[test]
    fn test_ctr128_roundtrip() {
        let key = [0x42u8; 16];
        let nonce = [0x01u8; 8];
        let plaintext = b"Hello, AES-128-CTR mode!";

        let cipher = AesCtr::new_128(&key);
        let ciphertext = cipher.encrypt(plaintext, &nonce, 0);
        let decrypted = cipher.decrypt(&ciphertext, &nonce, 0);

        assert_eq!(decrypted, plaintext.to_vec());
    }

    #[test]
    fn test_ctr256_roundtrip() {
        let key = [0xabu8; 32];
        let nonce = [0x00u8; 8];
        let plaintext = b"AES-256-CTR encryption and decryption round-trip test";

        let cipher = AesCtr::new_256(&key);
        let ciphertext = cipher.encrypt(plaintext, &nonce, 0);
        let decrypted = cipher.decrypt(&ciphertext, &nonce, 0);

        assert_eq!(decrypted, plaintext.to_vec());
    }

    #[test]
    fn test_ctr_empty_input() {
        let key = [0x00u8; 16];
        let nonce = [0x00u8; 8];
        let cipher = AesCtr::new_128(&key);
        let result = cipher.encrypt(&[], &nonce, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_ctr_partial_block() {
        // Less than 16 bytes — must still work correctly
        let key = [0x11u8; 16];
        let nonce = [0x22u8; 8];
        let plaintext = b"short";

        let cipher = AesCtr::new_128(&key);
        let ciphertext = cipher.encrypt(plaintext, &nonce, 0);
        let decrypted = cipher.decrypt(&ciphertext, &nonce, 0);

        assert_eq!(ciphertext.len(), plaintext.len());
        assert_eq!(decrypted, plaintext.to_vec());
    }

    #[test]
    fn test_ctr_large_data() {
        let key = [0x5au8; 16];
        let nonce = [0x3cu8; 8];
        let plaintext: Vec<u8> = (0u8..=255u8).cycle().take(65536).collect();

        let cipher = AesCtr::new_128(&key);
        let ciphertext = cipher.encrypt(&plaintext, &nonce, 0);
        let decrypted = cipher.decrypt(&ciphertext, &nonce, 0);

        assert_eq!(ciphertext.len(), plaintext.len());
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_ctr_different_nonces_produce_different_output() {
        let key = [0x77u8; 16];
        let nonce1 = [0x00u8; 8];
        let nonce2 = [0x01u8; 8];
        let plaintext = b"same plaintext for nonce comparison";

        let cipher = AesCtr::new_128(&key);
        let ct1 = cipher.encrypt(plaintext, &nonce1, 0);
        let ct2 = cipher.encrypt(plaintext, &nonce2, 0);

        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_ctr_different_counter_start_produces_different_output() {
        let key = [0x88u8; 16];
        let nonce = [0x00u8; 8];
        let plaintext = b"counter offset test";

        let cipher = AesCtr::new_128(&key);
        let ct1 = cipher.encrypt(plaintext, &nonce, 0);
        let ct2 = cipher.encrypt(plaintext, &nonce, 1);

        assert_ne!(ct1, ct2);
    }

    #[test]
    fn test_ctr_state_new() {
        let nonce = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let state = CtrState::new(nonce);
        assert_eq!(state.nonce, nonce);
        assert_eq!(state.counter, 0);
    }

    #[test]
    fn test_ctr_state_next_block_increments_counter() {
        let nonce = [0xffu8; 8];
        let mut state = CtrState::new(nonce);

        let block0 = state.next_block();
        assert_eq!(&block0[0..8], &nonce);
        assert_eq!(&block0[8..16], &0u64.to_be_bytes());
        assert_eq!(state.counter, 1);

        let block1 = state.next_block();
        assert_eq!(&block1[8..16], &1u64.to_be_bytes());
        assert_eq!(state.counter, 2);
    }

    #[test]
    fn test_ctr_counter_wraps_around() {
        let key = [0xaau8; 16];
        let nonce = [0x00u8; 8];
        let plaintext = b"wrap test";

        let cipher = AesCtr::new_128(&key);
        // Counter near overflow
        let ct = cipher.encrypt(plaintext, &nonce, u64::MAX);
        let pt = cipher.decrypt(&ct, &nonce, u64::MAX);
        assert_eq!(pt, plaintext.to_vec());
    }

    #[test]
    fn test_aes128_key_schedule_length() {
        let key = [0u8; 16];
        let schedule = key_expand_128(&key);
        assert_eq!(schedule.len(), 11, "AES-128 should have 11 round keys");
    }

    #[test]
    fn test_aes256_key_schedule_length() {
        let key = [0u8; 32];
        let schedule = key_expand_256(&key);
        assert_eq!(schedule.len(), 15, "AES-256 should have 15 round keys");
    }

    #[test]
    fn test_encrypt_decrypt_symmetry_256() {
        let key = [0xdeu8; 32];
        let nonce = [0xadu8; 8];
        let original = b"test symmetry for AES-256-CTR mode encryption";

        let cipher = AesCtr::new_256(&key);
        let encrypted = cipher.encrypt(original, &nonce, 42);
        let decrypted = cipher.decrypt(&encrypted, &nonce, 42);

        assert_eq!(decrypted, original.to_vec());
        // Encrypted should differ from plaintext
        assert_ne!(encrypted, original.to_vec());
    }

    #[test]
    fn test_inv_sbox_is_inverse_of_sbox() {
        // Verify that INV_SBOX[SBOX[i]] == i for all i
        for i in 0..=255usize {
            assert_eq!(
                INV_SBOX[SBOX[i] as usize], i as u8,
                "INV_SBOX should be the inverse of SBOX at index {}",
                i
            );
        }
    }
}
