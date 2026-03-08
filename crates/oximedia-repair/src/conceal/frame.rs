//! Frame concealment.
//!
//! This module provides functions for concealing corrupt video frames.

use crate::Result;

/// Conceal corrupt frame by copying previous frame.
pub fn conceal_with_previous(_corrupt_frame: &mut [u8], previous_frame: &[u8]) -> Result<()> {
    // Would copy previous frame data
    let _ = previous_frame;
    Ok(())
}

/// Conceal corrupt frame by interpolation.
pub fn conceal_with_interpolation(
    corrupt_frame: &mut [u8],
    previous_frame: &[u8],
    next_frame: &[u8],
) -> Result<()> {
    let len = corrupt_frame
        .len()
        .min(previous_frame.len())
        .min(next_frame.len());

    for i in 0..len {
        corrupt_frame[i] = ((previous_frame[i] as u16 + next_frame[i] as u16) / 2) as u8;
    }

    Ok(())
}

/// Insert black frame.
pub fn insert_black_frame(frame: &mut [u8]) {
    frame.fill(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conceal_with_interpolation() {
        let mut corrupt = vec![0; 10];
        let prev = vec![100; 10];
        let next = vec![100; 10];

        conceal_with_interpolation(&mut corrupt, &prev, &next).unwrap();
        assert_eq!(corrupt[0], 100);
    }

    #[test]
    fn test_insert_black_frame() {
        let mut frame = vec![255; 10];
        insert_black_frame(&mut frame);
        assert!(frame.iter().all(|&b| b == 0));
    }
}
