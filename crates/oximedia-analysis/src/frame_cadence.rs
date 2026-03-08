#![allow(dead_code)]
//! Frame cadence and pulldown detection analysis.
//!
//! Detects telecine patterns (3:2, 2:2, 2:3:3:2), duplicate/dropped frames,
//! and irregular frame cadence in video content. This is essential for
//! inverse telecine (IVTC), standards conversion QC, and identifying
//! encoding issues.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Known cadence patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CadencePattern {
    /// Progressive (no pulldown) — every frame is unique.
    Progressive,
    /// 3:2 pulldown (NTSC telecine from 24fps film).
    Pulldown32,
    /// 2:2 pulldown (PAL telecine, or interlaced from 25fps).
    Pulldown22,
    /// 2:3:3:2 advanced pulldown pattern.
    Pulldown2332,
    /// Irregular / mixed cadence.
    Irregular,
}

impl std::fmt::Display for CadencePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Progressive => write!(f, "Progressive"),
            Self::Pulldown32 => write!(f, "3:2 Pulldown"),
            Self::Pulldown22 => write!(f, "2:2 Pulldown"),
            Self::Pulldown2332 => write!(f, "2:3:3:2 Pulldown"),
            Self::Irregular => write!(f, "Irregular"),
        }
    }
}

/// Describes a single frame in the cadence analysis.
#[derive(Debug, Clone, Copy)]
pub struct FrameCadenceInfo {
    /// Frame index.
    pub index: usize,
    /// Similarity to the previous frame (0.0 = totally different, 1.0 = identical).
    pub similarity: f64,
    /// Whether this frame is considered a duplicate of the previous.
    pub is_duplicate: bool,
    /// Whether this frame appears to be a drop (abnormal gap).
    pub is_dropped: bool,
}

/// Result of a duplicate-frame run.
#[derive(Debug, Clone, Copy)]
pub struct DuplicateRun {
    /// Start frame index.
    pub start: usize,
    /// End frame index (inclusive).
    pub end: usize,
    /// Number of consecutive duplicate frames.
    pub length: usize,
}

/// Complete cadence analysis result.
#[derive(Debug, Clone)]
pub struct CadenceAnalysisResult {
    /// Total frames analysed.
    pub total_frames: usize,
    /// Detected dominant cadence pattern.
    pub dominant_pattern: CadencePattern,
    /// Confidence in the detected pattern (0.0..1.0).
    pub confidence: f64,
    /// Per-frame information.
    pub frame_info: Vec<FrameCadenceInfo>,
    /// Detected runs of duplicate frames.
    pub duplicate_runs: Vec<DuplicateRun>,
    /// Total duplicate frames.
    pub total_duplicates: usize,
    /// Total detected dropped frames.
    pub total_drops: usize,
    /// Ratio of unique frames to total frames.
    pub unique_ratio: f64,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for cadence analysis.
#[derive(Debug, Clone)]
pub struct CadenceConfig {
    /// Similarity threshold above which two frames are considered duplicates.
    pub duplicate_threshold: f64,
    /// Minimum run length to be reported as a duplicate run.
    pub min_run_length: usize,
    /// Minimum number of frames to attempt pattern detection.
    pub min_pattern_frames: usize,
}

impl Default for CadenceConfig {
    fn default() -> Self {
        Self {
            duplicate_threshold: 0.98,
            min_run_length: 1,
            min_pattern_frames: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Analyzer
// ---------------------------------------------------------------------------

/// Stateful cadence analyzer.
#[derive(Debug)]
pub struct CadenceAnalyzer {
    /// Configuration.
    config: CadenceConfig,
    /// Per-frame records.
    frames: Vec<FrameCadenceInfo>,
    /// Previous frame Y-plane summary (mean luminance per block).
    prev_block_means: Option<Vec<f64>>,
    /// Grid dimensions for block-based comparison.
    grid_cols: usize,
    /// Grid rows.
    grid_rows: usize,
}

impl CadenceAnalyzer {
    /// Create a new cadence analyzer.
    pub fn new(config: CadenceConfig) -> Self {
        Self {
            config,
            frames: Vec::new(),
            prev_block_means: None,
            grid_cols: 8,
            grid_rows: 6,
        }
    }

    /// Feed a Y-plane frame.
    pub fn push_frame(&mut self, y_plane: &[u8], width: usize, height: usize) {
        let block_means =
            compute_block_means(y_plane, width, height, self.grid_cols, self.grid_rows);
        let (similarity, is_dup) = if let Some(ref prev) = self.prev_block_means {
            let sim = block_similarity(prev, &block_means);
            (sim, sim >= self.config.duplicate_threshold)
        } else {
            (0.0, false)
        };

        let index = self.frames.len();
        self.frames.push(FrameCadenceInfo {
            index,
            similarity,
            is_duplicate: is_dup,
            is_dropped: false, // resolved in finalize
        });
        self.prev_block_means = Some(block_means);
    }

    /// Push a pre-computed similarity value (useful when frame data is unavailable).
    pub fn push_similarity(&mut self, similarity: f64) {
        let is_dup = similarity >= self.config.duplicate_threshold;
        let index = self.frames.len();
        self.frames.push(FrameCadenceInfo {
            index,
            similarity,
            is_duplicate: is_dup,
            is_dropped: false,
        });
    }

    /// Finalize and return the analysis result.
    pub fn finalize(mut self) -> CadenceAnalysisResult {
        let total = self.frames.len();
        if total == 0 {
            return CadenceAnalysisResult {
                total_frames: 0,
                dominant_pattern: CadencePattern::Progressive,
                confidence: 0.0,
                frame_info: Vec::new(),
                duplicate_runs: Vec::new(),
                total_duplicates: 0,
                total_drops: 0,
                unique_ratio: 1.0,
            };
        }

        // Mark dropped frames: abnormally low similarity after high-similarity frames
        detect_drops(&mut self.frames);

        let dup_runs = find_duplicate_runs(&self.frames, self.config.min_run_length);
        let total_dups = self.frames.iter().filter(|f| f.is_duplicate).count();
        let total_drops = self.frames.iter().filter(|f| f.is_dropped).count();

        #[allow(clippy::cast_precision_loss)]
        let unique_ratio = if total > 0 {
            (total - total_dups) as f64 / total as f64
        } else {
            1.0
        };

        let (pattern, confidence) = detect_pattern(&self.frames, self.config.min_pattern_frames);

        CadenceAnalysisResult {
            total_frames: total,
            dominant_pattern: pattern,
            confidence,
            frame_info: self.frames,
            duplicate_runs: dup_runs,
            total_duplicates: total_dups,
            total_drops,
            unique_ratio,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compute mean luminance for each block in a grid.
fn compute_block_means(
    y_plane: &[u8],
    width: usize,
    height: usize,
    cols: usize,
    rows: usize,
) -> Vec<f64> {
    if width == 0 || height == 0 || cols == 0 || rows == 0 {
        return vec![0.0; cols * rows];
    }
    let bw = width / cols;
    let bh = height / rows;
    let mut means = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let mut sum = 0u64;
            let mut count = 0u64;
            let y0 = r * bh;
            let y1 = if r == rows - 1 { height } else { y0 + bh };
            let x0 = c * bw;
            let x1 = if c == cols - 1 { width } else { x0 + bw };
            for y in y0..y1 {
                for x in x0..x1 {
                    let idx = y * width + x;
                    if idx < y_plane.len() {
                        sum += u64::from(y_plane[idx]);
                        count += 1;
                    }
                }
            }
            #[allow(clippy::cast_precision_loss)]
            let mean = if count > 0 {
                sum as f64 / count as f64
            } else {
                0.0
            };
            means.push(mean);
        }
    }
    means
}

/// Cosine-similarity-like metric between two block-mean vectors.
fn block_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for (&va, &vb) in a.iter().zip(b.iter()) {
        dot += va * vb;
        norm_a += va * va;
        norm_b += vb * vb;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-12 {
        return if norm_a < 1e-12 && norm_b < 1e-12 {
            1.0
        } else {
            0.0
        };
    }
    (dot / denom).clamp(0.0, 1.0)
}

/// Mark frames that appear to be drops (sudden low similarity surrounded by high).
fn detect_drops(frames: &mut [FrameCadenceInfo]) {
    if frames.len() < 3 {
        return;
    }
    for i in 1..frames.len() - 1 {
        let prev_sim = frames[i - 1].similarity;
        let next_sim = frames[i + 1].similarity;
        let cur_sim = frames[i].similarity;
        // A "drop" manifests as a sudden dip in similarity
        if cur_sim < 0.5 && prev_sim > 0.8 && next_sim > 0.8 {
            frames[i].is_dropped = true;
        }
    }
}

/// Find runs of consecutive duplicate frames.
fn find_duplicate_runs(frames: &[FrameCadenceInfo], min_run: usize) -> Vec<DuplicateRun> {
    let mut runs = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, f) in frames.iter().enumerate() {
        if f.is_duplicate {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start {
            let len = i - start;
            if len >= min_run {
                runs.push(DuplicateRun {
                    start,
                    end: i - 1,
                    length: len,
                });
            }
            run_start = None;
        }
    }
    if let Some(start) = run_start {
        let len = frames.len() - start;
        if len >= min_run {
            runs.push(DuplicateRun {
                start,
                end: frames.len() - 1,
                length: len,
            });
        }
    }
    runs
}

/// Attempt to match the duplicate/unique pattern to a known cadence.
fn detect_pattern(frames: &[FrameCadenceInfo], min_frames: usize) -> (CadencePattern, f64) {
    if frames.len() < min_frames {
        return (CadencePattern::Irregular, 0.0);
    }

    // Build a binary string: D = duplicate, U = unique
    let pattern_str: Vec<bool> = frames.iter().map(|f| f.is_duplicate).collect();
    let total_dups = pattern_str.iter().filter(|&&d| d).count();

    if total_dups == 0 {
        return (CadencePattern::Progressive, 1.0);
    }

    // Test 3:2 pattern (period 5): DDUUD or similar rotations
    let score_32 = test_periodic_pattern(&pattern_str, &[true, false, false, true, false]);
    // Test 2:2 pattern (period 4): DUDU or similar
    let score_22 = test_periodic_pattern(&pattern_str, &[true, false, true, false]);
    // Test 2:3:3:2 pattern (period 10): DDUUUDDUUU variants
    let score_2332 = test_periodic_pattern(
        &pattern_str,
        &[
            true, false, false, false, true, true, false, false, false, true,
        ],
    );

    let mut best = (CadencePattern::Irregular, 0.0f64);
    if score_32 > best.1 {
        best = (CadencePattern::Pulldown32, score_32);
    }
    if score_22 > best.1 {
        best = (CadencePattern::Pulldown22, score_22);
    }
    if score_2332 > best.1 {
        best = (CadencePattern::Pulldown2332, score_2332);
    }

    if best.1 < 0.5 {
        best = (CadencePattern::Irregular, best.1);
    }

    best
}

/// Score how well the observed pattern matches a reference periodic template.
/// Returns a match ratio 0.0..1.0 (best rotation is used).
fn test_periodic_pattern(observed: &[bool], template: &[bool]) -> f64 {
    let period = template.len();
    if period == 0 || observed.len() < period {
        return 0.0;
    }
    let mut best_score = 0.0f64;
    for rotation in 0..period {
        let mut matches = 0usize;
        for (i, &obs) in observed.iter().enumerate() {
            let tpl = template[(i + rotation) % period];
            if obs == tpl {
                matches += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let score = matches as f64 / observed.len() as f64;
        if score > best_score {
            best_score = score;
        }
    }
    best_score
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(width: usize, height: usize, value: u8) -> Vec<u8> {
        vec![value; width * height]
    }

    #[test]
    fn test_cadence_config_defaults() {
        let cfg = CadenceConfig::default();
        assert!((cfg.duplicate_threshold - 0.98).abs() < 0.001);
        assert_eq!(cfg.min_run_length, 1);
    }

    #[test]
    fn test_empty_analyzer() {
        let a = CadenceAnalyzer::new(CadenceConfig::default());
        let result = a.finalize();
        assert_eq!(result.total_frames, 0);
        assert_eq!(result.dominant_pattern, CadencePattern::Progressive);
    }

    #[test]
    fn test_progressive_content() {
        let mut a = CadenceAnalyzer::new(CadenceConfig::default());
        // Each frame has a distinct spatial pattern so cosine similarity between
        // consecutive frames is well below the duplicate threshold.
        for i in 0..20u32 {
            let frame: Vec<u8> = (0..16 * 16)
                .map(|p| {
                    // Create a gradient that shifts with each frame index
                    let col = (p % 16) as u32;
                    let row = (p / 16) as u32;
                    ((col
                        .wrapping_mul(17)
                        .wrapping_add(row.wrapping_mul(13))
                        .wrapping_add(i.wrapping_mul(73)))
                        % 256) as u8
                })
                .collect();
            a.push_frame(&frame, 16, 16);
        }
        let result = a.finalize();
        assert_eq!(result.dominant_pattern, CadencePattern::Progressive);
        assert_eq!(result.total_duplicates, 0);
    }

    #[test]
    fn test_all_duplicates() {
        let mut a = CadenceAnalyzer::new(CadenceConfig::default());
        for _ in 0..10 {
            a.push_frame(&make_frame(16, 16, 128), 16, 16);
        }
        let result = a.finalize();
        // First frame has no "previous" so 9 duplicates
        assert!(result.total_duplicates >= 9);
        assert!(result.unique_ratio < 0.2);
    }

    #[test]
    fn test_push_similarity_api() {
        let mut a = CadenceAnalyzer::new(CadenceConfig::default());
        a.push_similarity(0.0);
        a.push_similarity(0.99);
        a.push_similarity(0.50);
        let result = a.finalize();
        assert_eq!(result.total_frames, 3);
        assert_eq!(result.total_duplicates, 1); // only the 0.99 one
    }

    #[test]
    fn test_duplicate_run_detection() {
        let cfg = CadenceConfig {
            min_run_length: 2,
            ..Default::default()
        };
        let mut a = CadenceAnalyzer::new(cfg);
        // Simulate: unique, dup, dup, dup, unique
        a.push_similarity(0.0);
        a.push_similarity(0.99);
        a.push_similarity(0.99);
        a.push_similarity(0.99);
        a.push_similarity(0.5);
        let result = a.finalize();
        assert!(!result.duplicate_runs.is_empty());
        assert_eq!(result.duplicate_runs[0].length, 3);
    }

    #[test]
    fn test_cadence_pattern_display() {
        assert_eq!(CadencePattern::Progressive.to_string(), "Progressive");
        assert_eq!(CadencePattern::Pulldown32.to_string(), "3:2 Pulldown");
        assert_eq!(CadencePattern::Pulldown22.to_string(), "2:2 Pulldown");
        assert_eq!(CadencePattern::Pulldown2332.to_string(), "2:3:3:2 Pulldown");
        assert_eq!(CadencePattern::Irregular.to_string(), "Irregular");
    }

    #[test]
    fn test_block_similarity_identical() {
        let a = vec![100.0, 200.0, 150.0];
        let sim = block_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_block_similarity_zero_vectors() {
        let a = vec![0.0, 0.0, 0.0];
        let sim = block_similarity(&a, &a);
        // Both zero => considered identical
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_periodic_pattern_exact_match() {
        // Build exact 2:2 pattern
        let observed: Vec<bool> = (0..20).map(|i| i % 2 == 0).collect();
        let score = test_periodic_pattern(&observed, &[true, false, true, false]);
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_frame_cadence_info_fields() {
        let mut a = CadenceAnalyzer::new(CadenceConfig::default());
        a.push_frame(&make_frame(8, 8, 100), 8, 8);
        a.push_frame(&make_frame(8, 8, 100), 8, 8);
        let result = a.finalize();
        assert_eq!(result.frame_info.len(), 2);
        assert_eq!(result.frame_info[0].index, 0);
        assert!(result.frame_info[1].is_duplicate);
    }

    #[test]
    fn test_drop_detection() {
        let mut a = CadenceAnalyzer::new(CadenceConfig::default());
        // High sim, high sim, sudden dip, high sim, high sim
        a.push_similarity(0.0);
        a.push_similarity(0.95);
        a.push_similarity(0.95);
        a.push_similarity(0.2); // should be detected as drop
        a.push_similarity(0.95);
        a.push_similarity(0.95);
        let result = a.finalize();
        assert!(result.total_drops >= 1);
    }

    #[test]
    fn test_unique_ratio_calculation() {
        let mut a = CadenceAnalyzer::new(CadenceConfig::default());
        // 5 unique + 5 duplicate
        for i in 0..10 {
            if i % 2 == 0 {
                a.push_similarity(0.5); // unique
            } else {
                a.push_similarity(0.99); // duplicate
            }
        }
        let result = a.finalize();
        assert!((result.unique_ratio - 0.5).abs() < 0.01);
    }
}
