#![allow(dead_code)]
//! Deinterlacing configuration and processing helpers.

/// The field order of an interlaced video signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldOrder {
    /// Top (odd) field is displayed first.
    TopFieldFirst,
    /// Bottom (even) field is displayed first.
    BottomFieldFirst,
    /// The material is already progressive — no deinterlacing needed.
    Progressive,
}

/// Algorithm used to convert interlaced fields to progressive frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeinterlaceMethod {
    /// Drop one field and scale the remaining field up — fastest, lowest quality.
    FieldDrop,
    /// Blend the two fields together — simple, introduces motion blur.
    Blend,
    /// Bob deinterlacing: each field becomes a full progressive frame.
    Bob,
    /// Weave: combine two consecutive fields into one frame (motion artifacts).
    Weave,
    /// Motion-adaptive deinterlacing — high quality, more compute.
    MotionAdaptive,
    /// Yadif algorithm: temporal + spatial interpolation.
    Yadif,
}

impl DeinterlaceMethod {
    /// Frame-rate multiplier relative to the input field rate.
    ///
    /// `Bob` produces one output frame per field (×2), `Yadif` can be
    /// configured to do the same.  All other methods produce one output frame
    /// per two fields (×1).
    pub fn output_frame_rate_multiplier(&self) -> u32 {
        match self {
            DeinterlaceMethod::Bob | DeinterlaceMethod::Yadif => 2,
            _ => 1,
        }
    }
}

/// Configuration for a deinterlacing operation.
#[derive(Debug, Clone)]
pub struct DeinterlaceConfig {
    /// The field order of the input material.
    pub field_order: FieldOrder,
    /// The deinterlacing algorithm to apply.
    pub method: DeinterlaceMethod,
    /// Number of threads to use for processing (0 = auto).
    pub threads: u32,
}

impl DeinterlaceConfig {
    /// Create a new [`DeinterlaceConfig`] with default thread count (0 = auto).
    pub fn new(field_order: FieldOrder, method: DeinterlaceMethod) -> Self {
        Self {
            field_order,
            method,
            threads: 0,
        }
    }

    /// Whether the chosen method uses temporal information (i.e. references
    /// more than one field).
    pub fn is_temporal(&self) -> bool {
        matches!(
            self.method,
            DeinterlaceMethod::MotionAdaptive | DeinterlaceMethod::Yadif
        )
    }

    /// Whether the input is already progressive (no processing needed).
    pub fn is_progressive_passthrough(&self) -> bool {
        self.field_order == FieldOrder::Progressive
    }
}

/// A single video field extracted from an interlaced frame.
#[derive(Debug, Clone)]
pub struct VideoField {
    /// Which field this is (0 = top / odd, 1 = bottom / even).
    pub field_index: u8,
    /// Width of the field in pixels.
    pub width: u32,
    /// Height of the field in pixels (half the frame height).
    pub height: u32,
    /// Raw luma byte data (Y plane only for simplicity).
    pub luma: Vec<u8>,
}

impl VideoField {
    /// Create a new [`VideoField`] with blank luma.
    pub fn blank(field_index: u8, width: u32, height: u32) -> Self {
        Self {
            field_index,
            width,
            height,
            luma: vec![0u8; (width * height) as usize],
        }
    }
}

/// Processes video fields into progressive frames.
#[derive(Debug)]
pub struct DeinterlaceProcessor {
    config: DeinterlaceConfig,
}

impl DeinterlaceProcessor {
    /// Create a new [`DeinterlaceProcessor`].
    pub fn new(config: DeinterlaceConfig) -> Self {
        Self { config }
    }

    /// Access the current configuration.
    pub fn config(&self) -> &DeinterlaceConfig {
        &self.config
    }

    /// Process a single [`VideoField`] and return a progressive frame as raw
    /// luma bytes.
    ///
    /// For `FieldDrop` and `Bob` the field luma is simply returned as-is
    /// (a real implementation would scale the field to full height).
    /// For `Blend`/`Weave`/`MotionAdaptive`/`Yadif` a previous field is needed;
    /// this simplified version returns the input unchanged.
    pub fn process_field(&self, field: &VideoField) -> Vec<u8> {
        if self.config.is_progressive_passthrough() {
            return field.luma.clone();
        }

        match self.config.method {
            DeinterlaceMethod::FieldDrop | DeinterlaceMethod::Bob => {
                // Return field data unchanged (caller would up-scale in practice).
                field.luma.clone()
            }
            DeinterlaceMethod::Blend => {
                // Single-field blend: halve each luma sample (placeholder).
                field.luma.iter().map(|&v| v / 2).collect()
            }
            _ => field.luma.clone(),
        }
    }

    /// Output frame rate given the input frame rate (in fps numerator/denominator).
    #[allow(clippy::cast_precision_loss)]
    pub fn output_fps(&self, input_fps_num: u32, input_fps_den: u32) -> f64 {
        let multiplier = self.config.method.output_frame_rate_multiplier();
        (input_fps_num as f64 / input_fps_den as f64) * multiplier as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_order_variants() {
        let tff = FieldOrder::TopFieldFirst;
        let bff = FieldOrder::BottomFieldFirst;
        let prog = FieldOrder::Progressive;
        assert_ne!(tff, bff);
        assert_ne!(tff, prog);
    }

    #[test]
    fn test_frame_rate_multiplier_bob() {
        assert_eq!(DeinterlaceMethod::Bob.output_frame_rate_multiplier(), 2);
    }

    #[test]
    fn test_frame_rate_multiplier_yadif() {
        assert_eq!(DeinterlaceMethod::Yadif.output_frame_rate_multiplier(), 2);
    }

    #[test]
    fn test_frame_rate_multiplier_blend() {
        assert_eq!(DeinterlaceMethod::Blend.output_frame_rate_multiplier(), 1);
    }

    #[test]
    fn test_frame_rate_multiplier_field_drop() {
        assert_eq!(
            DeinterlaceMethod::FieldDrop.output_frame_rate_multiplier(),
            1
        );
    }

    #[test]
    fn test_config_is_temporal_true() {
        let cfg =
            DeinterlaceConfig::new(FieldOrder::TopFieldFirst, DeinterlaceMethod::MotionAdaptive);
        assert!(cfg.is_temporal());
    }

    #[test]
    fn test_config_is_temporal_false() {
        let cfg = DeinterlaceConfig::new(FieldOrder::TopFieldFirst, DeinterlaceMethod::Blend);
        assert!(!cfg.is_temporal());
    }

    #[test]
    fn test_progressive_passthrough() {
        let cfg = DeinterlaceConfig::new(FieldOrder::Progressive, DeinterlaceMethod::FieldDrop);
        assert!(cfg.is_progressive_passthrough());
    }

    #[test]
    fn test_not_progressive_passthrough() {
        let cfg = DeinterlaceConfig::new(FieldOrder::TopFieldFirst, DeinterlaceMethod::FieldDrop);
        assert!(!cfg.is_progressive_passthrough());
    }

    #[test]
    fn test_process_field_progressive_passthrough() {
        let cfg = DeinterlaceConfig::new(FieldOrder::Progressive, DeinterlaceMethod::Bob);
        let proc = DeinterlaceProcessor::new(cfg);
        let field = VideoField::blank(0, 4, 4);
        let out = proc.process_field(&field);
        assert_eq!(out, vec![0u8; 16]);
    }

    #[test]
    fn test_process_field_field_drop() {
        let cfg = DeinterlaceConfig::new(FieldOrder::TopFieldFirst, DeinterlaceMethod::FieldDrop);
        let proc = DeinterlaceProcessor::new(cfg);
        let mut field = VideoField::blank(0, 2, 2);
        field.luma = vec![10, 20, 30, 40];
        let out = proc.process_field(&field);
        assert_eq!(out, vec![10, 20, 30, 40]);
    }

    #[test]
    fn test_process_field_blend() {
        let cfg = DeinterlaceConfig::new(FieldOrder::BottomFieldFirst, DeinterlaceMethod::Blend);
        let proc = DeinterlaceProcessor::new(cfg);
        let mut field = VideoField::blank(1, 2, 2);
        field.luma = vec![100, 200, 50, 150];
        let out = proc.process_field(&field);
        assert_eq!(out, vec![50, 100, 25, 75]);
    }

    #[test]
    fn test_output_fps_bob() {
        let cfg = DeinterlaceConfig::new(FieldOrder::TopFieldFirst, DeinterlaceMethod::Bob);
        let proc = DeinterlaceProcessor::new(cfg);
        // 25 fps interlaced → 50 fps progressive with Bob
        let fps = proc.output_fps(25, 1);
        assert!((fps - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_output_fps_blend() {
        let cfg = DeinterlaceConfig::new(FieldOrder::TopFieldFirst, DeinterlaceMethod::Blend);
        let proc = DeinterlaceProcessor::new(cfg);
        let fps = proc.output_fps(25, 1);
        assert!((fps - 25.0).abs() < 1e-9);
    }
}
