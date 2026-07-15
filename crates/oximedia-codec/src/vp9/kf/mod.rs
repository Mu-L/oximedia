//! Real VP9 keyframe / intra-only frame decoding (8-bit, 4:2:0).
//!
//! This module is an exact port of the intra decode path of libvpx
//! (`vp9/decoder/*`, `vp9/common/*`, `vpx_dsp/*`, 8-bit build), verified
//! bit-exact against `ffmpeg`/libvpx reference decodes of real encoder
//! output (see the tests at the bottom).
//!
//! Layout:
//! - [`booldec`]: VP9 boolean (range) decoder (`vpx_dsp/bitreader`).
//! - [`hdr`]: intra compressed-header parse (tx mode, coef/skip prob
//!   updates) and the subexp probability-update machinery.
//! - [`tables`] / [`scan`]: constant tables mechanically extracted from
//!   libvpx sources (kf mode/partition probabilities, default coefficient
//!   probabilities, Pareto model tree, quantizer lookups, scan orders and
//!   neighbor tables, block-size lookups).
//! - [`itx`]: inverse DCT/ADST/WHT transforms.
//! - [`pred`]: intra predictors and border construction.
//! - [`recon`]: tile/partition/block decode driver.
//! - [`lf`]: loop filter (levels, masks, kernels).

mod booldec;
mod hdr;
mod itx;
mod lf;
mod pred;
mod recon;
mod scan;
mod tables;

pub(crate) use recon::{decode_intra_frame, DecodedIntraFrame};

use crate::error::{CodecError, CodecResult};
use crate::vp9::uncompressed::UncompressedHeader;

/// Decodes one VP9 intra frame (keyframe or intra-only) to planes.
///
/// Supported scope this pass: profile 0 (8-bit, 4:2:0). Other profiles and
/// bit depths return an honest [`CodecError::UnsupportedFeature`].
///
/// # Errors
///
/// Returns an error for unsupported profiles/formats or malformed data.
pub(crate) fn decode_keyframe(
    hdr: &UncompressedHeader,
    frame_data: &[u8],
) -> CodecResult<DecodedIntraFrame> {
    // TODO(0.2.x): profiles 1-3 — 4:2:2 / 4:4:4 subsampling and 10/12-bit
    // depths (highbd transform/predictor/loop-filter variants).
    if hdr.bit_depth != 8 || !hdr.subsampling_x || !hdr.subsampling_y {
        return Err(CodecError::UnsupportedFeature(format!(
            "VP9 keyframe decode supports 8-bit 4:2:0 (profile 0); \
             got {}-bit ss_x={} ss_y={} (profile {})",
            hdr.bit_depth, hdr.subsampling_x, hdr.subsampling_y, hdr.profile
        )));
    }
    if hdr.width == 0 || hdr.height == 0 {
        return Err(CodecError::InvalidBitstream(
            "VP9: zero frame dimensions".into(),
        ));
    }
    decode_intra_frame(hdr, frame_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Splits a raw planar YUV420 reference dump into (Y, U, V).
    fn split_yuv(data: &[u8], w: usize, h: usize) -> (&[u8], &[u8], &[u8]) {
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);
        let ysz = w * h;
        let csz = cw * ch;
        assert_eq!(data.len(), ysz + 2 * csz, "reference YUV size");
        (
            &data[..ysz],
            &data[ysz..ysz + csz],
            &data[ysz + csz..ysz + 2 * csz],
        )
    }

    /// Compares one decoded (aligned) plane against a cropped reference
    /// plane; returns the number of mismatching pixels and the sum of
    /// squared error. Prints the first mismatch coordinates to speed up
    /// failure triage.
    fn diff_plane(
        dec: &recon::PlaneBuf,
        r#ref: &[u8],
        w: usize,
        h: usize,
        name: &str,
    ) -> (usize, u64) {
        let mut bad = 0usize;
        let mut sse = 0u64;
        for y in 0..h {
            for x in 0..w {
                let d = dec.data[y * dec.stride + x];
                let e = r#ref[y * w + x];
                if d != e {
                    if bad < 20 {
                        println!(
                            "{name} mismatch at ({x},{y}) mi=({},{}) dec={d} ref={e}",
                            x / 8,
                            y / 8
                        );
                    }
                    bad += 1;
                    let diff = i64::from(d) - i64::from(e);
                    sse += (diff * diff) as u64;
                }
            }
        }
        (bad, sse)
    }

    fn assert_bit_exact(ivf_frame: &[u8], ref_yuv: &[u8], w: usize, h: usize, label: &str) {
        let hdr = UncompressedHeader::parse(ivf_frame).expect("header parse");
        assert!(hdr.is_keyframe(), "{label}: test vector must be a keyframe");
        assert_eq!(hdr.width as usize, w);
        assert_eq!(hdr.height as usize, h);

        let frame = decode_keyframe(&hdr, ivf_frame).expect("keyframe decode");
        let (ry, ru, rv) = split_yuv(ref_yuv, w, h);
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);

        let (bad_y, sse_y) = diff_plane(&frame.planes[0], ry, w, h, "Y");
        let (bad_u, sse_u) = diff_plane(&frame.planes[1], ru, cw, ch, "U");
        let (bad_v, sse_v) = diff_plane(&frame.planes[2], rv, cw, ch, "V");

        assert!(
            bad_y == 0 && bad_u == 0 && bad_v == 0,
            "{label}: decode differs from libvpx/ffmpeg reference: \
             Y {bad_y} px (sse {sse_y}), U {bad_u} px (sse {sse_u}), \
             V {bad_v} px (sse {sse_v})"
        );
    }

    /// 76x42 keyframe (non-8/64-aligned dimensions), libvpx-vp9 crf 24
    /// (non-zero loop filter level), reference-decoded with ffmpeg's native
    /// VP9 decoder (verified byte-identical to libvpx's own decoder).
    #[test]
    fn keyframe_76x42_crf24_bit_exact_vs_libvpx() {
        assert_bit_exact(
            include_bytes!("testdata/kf76x42.frame0.bin"),
            include_bytes!("testdata/ref76x42.yuv"),
            76,
            42,
            "kf76x42",
        );
    }

    /// 128x128 keyframe, libvpx-vp9 crf 12 (rich texture, multiple
    /// superblocks, all partition shapes).
    #[test]
    fn keyframe_128x128_crf12_bit_exact_vs_libvpx() {
        assert_bit_exact(
            include_bytes!("testdata/kf128.frame0.bin"),
            include_bytes!("testdata/ref128.yuv"),
            128,
            128,
            "kf128",
        );
    }

    /// 64x64 lossless keyframe (WHT transform path, loop filter off).
    #[test]
    fn keyframe_64x64_lossless_bit_exact_vs_libvpx() {
        assert_bit_exact(
            include_bytes!("testdata/kf64ll.frame0.bin"),
            include_bytes!("testdata/ref64ll.yuv"),
            64,
            64,
            "kf64ll",
        );
    }

    /// 512x64 keyframe encoded with two tile columns (`tile_cols_log2` = 1):
    /// exercises tile-size parsing, per-tile bool decoders, and the left
    /// context / intra-availability reset at the tile boundary.
    #[test]
    fn keyframe_512x64_two_tile_columns_bit_exact_vs_libvpx() {
        assert_bit_exact(
            include_bytes!("testdata/kf512tc.frame0.bin"),
            include_bytes!("testdata/ref512tc.yuv"),
            512,
            64,
            "kf512tc",
        );
    }
}
