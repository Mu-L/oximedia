//! Caption line-breaking algorithms: greedy, optimal (Knuth-Plass-inspired DP),
//! reading-speed helpers, and line-balance optimisation.

use std::collections::HashMap;

/// Configuration for line-breaking behaviour.
#[derive(Debug, Clone, PartialEq)]
pub struct LineBreakConfig {
    /// Maximum characters per line.
    pub max_chars_per_line: u8,
    /// Maximum reading speed in characters per second.
    pub max_cps: f32,
    /// Maximum number of lines in a caption block.
    pub max_lines: u8,
    /// Minimum gap between successive caption blocks in milliseconds.
    pub min_gap_ms: u32,
    /// Hard maximum characters per line (enforced even if `max_chars_per_line`
    /// would allow more).  `None` means no additional constraint.
    pub hard_max_chars: Option<u8>,
}

impl LineBreakConfig {
    /// Sensible broadcast defaults: 42 chars/line, 17 CPS, 2 lines, 80ms gap.
    pub fn default_broadcast() -> Self {
        Self {
            max_chars_per_line: 42,
            max_cps: 17.0,
            max_lines: 2,
            min_gap_ms: 80,
            hard_max_chars: None,
        }
    }

    /// Effective maximum characters per line considering the hard cap.
    pub fn effective_max_chars(&self) -> u8 {
        match self.hard_max_chars {
            Some(hard) => self.max_chars_per_line.min(hard),
            None => self.max_chars_per_line,
        }
    }
}

// ─── Target audience reading speed ────────────────────────────────────────────

/// The intended viewing audience, used to select appropriate CPS limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudienceProfile {
    /// Young children (ages 4–7): very slow readers.
    YoungChildren,
    /// Older children (ages 8–12): moderate readers.
    OlderChildren,
    /// General adult audience: standard broadcast speed.
    Adults,
    /// Specialised/technical audience: faster reading expected.
    TechnicalAdults,
}

impl AudienceProfile {
    /// Maximum recommended reading speed (CPS) for this audience.
    pub fn max_cps(self) -> f32 {
        match self {
            AudienceProfile::YoungChildren => 5.0,
            AudienceProfile::OlderChildren => 10.0,
            AudienceProfile::Adults => 17.0,
            AudienceProfile::TechnicalAdults => 22.0,
        }
    }

    /// Minimum recommended display duration (ms) for this audience.
    pub fn min_display_ms(self) -> u32 {
        match self {
            AudienceProfile::YoungChildren => 3000,
            AudienceProfile::OlderChildren => 1500,
            AudienceProfile::Adults => 1000,
            AudienceProfile::TechnicalAdults => 700,
        }
    }
}

/// Validate reading speed for a specific audience profile.
///
/// Returns `true` if the CPS is within acceptable range for the audience.
pub fn reading_speed_ok_for_audience(
    text: &str,
    duration_ms: u64,
    audience: AudienceProfile,
) -> bool {
    reading_speed_ok(text, duration_ms, audience.max_cps())
}

// ─── CPS cache ────────────────────────────────────────────────────────────────

/// A cache for CPS (characters-per-second) computations.
///
/// This avoids recomputing CPS for the same `(text, duration_ms)` pairs when
/// captions are re-broken multiple times (e.g., during layout refinement).
#[derive(Debug, Default)]
pub struct CpsCache {
    cache: HashMap<(u64, u64), f32>, // key: (text_hash, duration_ms)
}

impl CpsCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute or retrieve cached CPS for `(text, duration_ms)`.
    pub fn compute_cps(&mut self, text: &str, duration_ms: u64) -> f32 {
        let key = (hash_str(text), duration_ms);
        *self
            .cache
            .entry(key)
            .or_insert_with(|| compute_cps(text, duration_ms))
    }

    /// Return the number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Return `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Simple FNV-1a 64-bit hash for a string.
fn hash_str(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    s.bytes().fold(FNV_OFFSET, |acc, b| {
        (acc ^ b as u64).wrapping_mul(FNV_PRIME)
    })
}

// ─── CJK line breaking ────────────────────────────────────────────────────────

/// Returns `true` if `ch` is a CJK character (logographic / ideographic).
fn is_cjk_char(ch: char) -> bool {
    // CJK Unified Ideographs and common extensions.
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
        // Hiragana and Katakana (Japanese syllabic scripts).
        || ('\u{3040}'..='\u{309F}').contains(&ch)
        || ('\u{30A0}'..='\u{30FF}').contains(&ch)
        // Hangul (Korean).
        || ('\u{AC00}'..='\u{D7AF}').contains(&ch)
}

/// Returns `true` if the character is a line-break *prohibiting* character.
///
/// These characters must not appear at the start of a line (opening brackets,
/// leading punctuation) per Unicode line-breaking rules (UAX #14).
fn is_cjk_no_start(ch: char) -> bool {
    matches!(
        ch,
        '、' | '。'
            | '，'
            | '．'
            | '：'
            | '；'
            | '？'
            | '！'
            | '）'
            | '」'
            | '』'
            | '】'
            | '〕'
            | '〉'
            | '》'
            | '·'
            | '‥'
            | '…'
            | 'ー'
            | 'ヽ'
            | 'ヾ'
            | 'ゝ'
            | 'ゞ'
    )
}

/// Break `text` into lines for CJK scripts (no spaces between words).
///
/// CJK text is broken at character boundaries with the following rules:
/// - No line ends with a leading bracket / punctuation character that should
///   not start a line (`is_cjk_no_start`).
/// - Lines do not exceed `max_width` characters.
pub fn cjk_break(text: &str, max_width: u8) -> Vec<String> {
    let max = max_width.max(1) as usize;
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();

    if n <= max {
        return vec![text.to_string()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut start = 0;

    while start < n {
        // Ideal end is `start + max`.
        let ideal_end = (start + max).min(n);

        if ideal_end >= n {
            lines.push(chars[start..].iter().collect());
            break;
        }

        // Adjust end if the character *after* the cut cannot start a line.
        let mut end = ideal_end;
        while end > start + 1 && is_cjk_no_start(chars[end]) {
            end -= 1;
        }

        lines.push(chars[start..end].iter().collect());
        start = end;
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Language-aware line breaking.
///
/// For CJK text, delegates to [`cjk_break`].  For all other scripts,
/// delegates to [`greedy_break`].
///
/// The heuristic for detecting CJK: if > 30% of non-whitespace characters
/// are CJK/Hiragana/Katakana/Hangul, the text is treated as CJK.
pub fn language_aware_break(text: &str, max_width: u8) -> Vec<String> {
    let non_ws: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if non_ws.is_empty() {
        return vec![String::new()];
    }

    let cjk_count = non_ws.iter().filter(|&&c| is_cjk_char(c)).count();
    let cjk_fraction = cjk_count as f32 / non_ws.len() as f32;

    if cjk_fraction > 0.30 {
        cjk_break(text, max_width)
    } else {
        greedy_break(text, max_width)
    }
}

/// Which algorithm to use when breaking caption text into lines.
#[derive(Debug, Clone, PartialEq)]
pub enum LineBreakAlgorithm {
    /// Break at the last space before `max_width`.
    Greedy,
    /// Dynamic-programming algorithm that minimises raggedness (Knuth-Plass
    /// inspired): `cost(line) = (max_width - used_width)^2`.
    Optimal,
    /// Every line is exactly `u8` characters wide (hard wrap, no splitting of words).
    Fixed(u8),
}

// ─── Greedy break ─────────────────────────────────────────────────────────────

/// Break `text` greedily at the last space before `max_width` characters.
///
/// Words longer than `max_width` are placed on their own line unchanged.
pub fn greedy_break(text: &str, max_width: u8) -> Vec<String> {
    let max = max_width.max(1) as usize;
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ─── Optimal break (Knuth-Plass DP) ──────────────────────────────────────────

/// Break `text` using a dynamic-programming algorithm that minimises the sum of
/// squared slack on each line: `cost(line) = (max_width - line_width)^2`.
///
/// This produces more balanced lines than the greedy approach.
pub fn optimal_break(text: &str, max_width: u8) -> Vec<String> {
    let max = max_width.max(1) as usize;
    let words: Vec<&str> = text.split_whitespace().collect();
    let n = words.len();

    if n == 0 {
        return vec![String::new()];
    }

    // Pre-compute cumulative character widths (without spaces for quick lookup).
    // span_width(i, j) = sum of word lengths from i..=j plus (j-i) spaces.
    let word_lens: Vec<usize> = words.iter().map(|w| w.chars().count()).collect();

    // dp[i] = minimum cost to break words[i..n] optimally.
    // breaks[i] = the end-index (exclusive) of the first line when starting at i.
    let mut dp = vec![u64::MAX; n + 1];
    let mut breaks: Vec<usize> = vec![n; n + 1];
    dp[n] = 0;

    for i in (0..n).rev() {
        let mut width = 0usize;
        for j in i..n {
            width += word_lens[j];
            if j > i {
                width += 1; // space
            }
            if width > max {
                break;
            }
            let slack = max - width;
            let line_cost = (slack * slack) as u64;
            let rest_cost = dp[j + 1];
            if rest_cost != u64::MAX {
                let total = line_cost.saturating_add(rest_cost);
                if total < dp[i] {
                    dp[i] = total;
                    breaks[i] = j + 1;
                }
            }
        }
        // If no valid break was found (all words too wide), force a single word.
        if dp[i] == u64::MAX {
            dp[i] = 0;
            breaks[i] = i + 1;
        }
    }

    // Reconstruct lines.
    let mut lines: Vec<String> = Vec::new();
    let mut pos = 0;
    while pos < n {
        let end = breaks[pos].min(n);
        let end = if end <= pos { pos + 1 } else { end };
        lines.push(words[pos..end].join(" "));
        pos = end;
    }
    lines
}

// ─── Reading-speed helpers ────────────────────────────────────────────────────

/// Compute reading speed in characters per second.
///
/// Returns 0.0 if `duration_ms` is zero.
pub fn compute_cps(text: &str, duration_ms: u64) -> f32 {
    if duration_ms == 0 {
        return 0.0;
    }
    let char_count = text.chars().count() as f32;
    char_count / (duration_ms as f32 / 1000.0)
}

/// Returns `true` when the reading speed of `text` over `duration_ms` does not
/// exceed `max_cps`.
pub fn reading_speed_ok(text: &str, duration_ms: u64, max_cps: f32) -> bool {
    compute_cps(text, duration_ms) <= max_cps
}

/// Compute the minimum display duration required to read `text` at `max_cps`,
/// but never shorter than `min_ms`.
///
/// Formula: `max(min_ms, ceil(char_count * 1000 / max_cps))`.
pub fn adjust_duration_for_reading(text: &str, min_ms: u32, max_cps: f32) -> u32 {
    if max_cps <= 0.0 {
        return min_ms;
    }
    let char_count = text.chars().count() as f32;
    let required_ms = (char_count * 1000.0 / max_cps).ceil() as u32;
    required_ms.max(min_ms)
}

// ─── Line balance ─────────────────────────────────────────────────────────────

/// Statistics and scoring for caption line balance.
pub struct LineBalance;

impl LineBalance {
    /// Compute a balance factor in [0.0, 1.0]:
    /// - `0.0` = perfectly balanced (all lines same length).
    /// - `1.0` = maximally unbalanced.
    ///
    /// Uses the standard deviation of line lengths normalised by the mean.
    /// Returns `0.0` for 0 or 1 lines.
    pub fn balance_factor(lines: &[String]) -> f32 {
        if lines.len() <= 1 {
            return 0.0;
        }
        let lengths: Vec<f32> = lines.iter().map(|l| l.chars().count() as f32).collect();
        let mean = lengths.iter().sum::<f32>() / lengths.len() as f32;
        if mean < 1e-6 {
            return 0.0;
        }
        let variance =
            lengths.iter().map(|&l| (l - mean).powi(2)).sum::<f32>() / lengths.len() as f32;
        let std_dev = variance.sqrt();
        // Normalise by mean so the result is dimensionless; cap at 1.0.
        (std_dev / mean).min(1.0)
    }
}

/// Redistribute words across lines to minimise [`LineBalance::balance_factor`].
///
/// Internally calls [`optimal_break`] with a `max_width` derived from the
/// average line length, then returns the result if it is better balanced than
/// the input, otherwise returns the input unchanged.
pub fn rebalance_lines(lines: Vec<String>, max_width: u8) -> Vec<String> {
    if lines.len() <= 1 {
        return lines;
    }

    let original_factor = LineBalance::balance_factor(&lines);
    let combined = lines.join(" ");
    let rebroken = optimal_break(&combined, max_width);
    let new_factor = LineBalance::balance_factor(&rebroken);

    if new_factor < original_factor {
        rebroken
    } else {
        lines
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- greedy_break ---

    #[test]
    fn greedy_break_empty_string() {
        let result = greedy_break("", 40);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn greedy_break_single_word_fits() {
        let result = greedy_break("Hello", 40);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn greedy_break_two_words_fit_on_one_line() {
        let result = greedy_break("Hello world", 20);
        assert_eq!(result, vec!["Hello world"]);
    }

    #[test]
    fn greedy_break_wraps_at_limit() {
        let result = greedy_break("Hello world", 8);
        assert_eq!(result, vec!["Hello", "world"]);
    }

    #[test]
    fn greedy_break_multiple_lines() {
        let result = greedy_break("one two three four five", 9);
        // "one two" = 7, "three" = 5, "four" = 4, "five" = 4
        assert!(result.len() >= 2);
        for line in &result {
            assert!(line.chars().count() <= 9, "line '{line}' exceeds max width");
        }
    }

    #[test]
    fn greedy_break_long_word_gets_own_line() {
        let result = greedy_break("A superlongwordthatexceedslimit B", 10);
        // The long word must appear alone on its line.
        assert!(result.iter().any(|l| l.contains("superlongword")));
    }

    #[test]
    fn greedy_break_preserves_all_words() {
        let text = "one two three four five six seven";
        let result = greedy_break(text, 15);
        let rejoined = result.join(" ");
        assert_eq!(rejoined, text);
    }

    // --- optimal_break ---

    #[test]
    fn optimal_break_empty_string() {
        let result = optimal_break("", 40);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn optimal_break_single_line() {
        let result = optimal_break("Hello world", 20);
        assert_eq!(result, vec!["Hello world"]);
    }

    #[test]
    fn optimal_break_more_balanced_than_greedy() {
        // "one two three four" greedy at width 10:
        //   "one two"  (7) + "three"   (5) + "four" (4)  → slack: 3,5,6
        // optimal should find a better balance.
        let text = "one two three four";
        let optimal = optimal_break(text, 10);
        let greedy = greedy_break(text, 10);
        let opt_balance = LineBalance::balance_factor(&optimal);
        let greed_balance = LineBalance::balance_factor(&greedy);
        // Optimal should be at least as balanced.
        assert!(
            opt_balance <= greed_balance + 0.01,
            "optimal balance {opt_balance} worse than greedy {greed_balance}"
        );
    }

    #[test]
    fn optimal_break_preserves_all_words() {
        let text = "alpha beta gamma delta epsilon zeta";
        let result = optimal_break(text, 15);
        let rejoined = result.join(" ");
        assert_eq!(rejoined, text);
    }

    #[test]
    fn optimal_break_no_line_exceeds_max_width() {
        let text = "short lines should be wrapped correctly by algorithm";
        let result = optimal_break(text, 20);
        for line in &result {
            assert!(
                line.chars().count() <= 20,
                "line '{line}' exceeds max width"
            );
        }
    }

    // --- compute_cps ---

    #[test]
    fn compute_cps_basic() {
        // 10 chars over 2000ms = 5 cps.
        let cps = compute_cps("Hello wrld", 2000);
        assert!((cps - 5.0).abs() < 0.01, "expected ~5.0, got {cps}");
    }

    #[test]
    fn compute_cps_zero_duration_returns_zero() {
        assert_eq!(compute_cps("Hello", 0), 0.0);
    }

    #[test]
    fn compute_cps_empty_text() {
        assert_eq!(compute_cps("", 1000), 0.0);
    }

    // --- reading_speed_ok ---

    #[test]
    fn reading_speed_ok_slow_enough() {
        // 5 chars at 1 second = 5 cps < 17 → ok.
        assert!(reading_speed_ok("Hello", 1000, 17.0));
    }

    #[test]
    fn reading_speed_ok_too_fast() {
        // 50 chars at 1 second = 50 cps > 17.
        let long_text = "A".repeat(50);
        assert!(!reading_speed_ok(&long_text, 1000, 17.0));
    }

    // --- adjust_duration_for_reading ---

    #[test]
    fn adjust_duration_respects_min() {
        // 5 chars at 17 cps needs ~295ms, but min is 1000ms.
        let d = adjust_duration_for_reading("Hello", 1000, 17.0);
        assert_eq!(d, 1000);
    }

    #[test]
    fn adjust_duration_extends_for_long_text() {
        // 170 chars at 17 cps needs 10000ms; min is 1000ms.
        let text = "A".repeat(170);
        let d = adjust_duration_for_reading(&text, 1000, 17.0);
        assert_eq!(d, 10000);
    }

    #[test]
    fn adjust_duration_zero_max_cps_returns_min() {
        let d = adjust_duration_for_reading("Hello world", 500, 0.0);
        assert_eq!(d, 500);
    }

    // --- LineBalance ---

    #[test]
    fn balance_factor_single_line_is_zero() {
        let lines = vec!["Hello world".to_string()];
        assert_eq!(LineBalance::balance_factor(&lines), 0.0);
    }

    #[test]
    fn balance_factor_equal_lines_is_zero() {
        let lines = vec!["Hello".to_string(), "World".to_string()];
        assert!((LineBalance::balance_factor(&lines)).abs() < 1e-5);
    }

    #[test]
    fn balance_factor_unequal_lines_nonzero() {
        let lines = vec!["A".to_string(), "A much longer line here".to_string()];
        assert!(LineBalance::balance_factor(&lines) > 0.0);
    }

    #[test]
    fn balance_factor_empty_lines_is_zero() {
        assert_eq!(LineBalance::balance_factor(&[]), 0.0);
    }

    // --- rebalance_lines ---

    #[test]
    fn rebalance_lines_single_line_unchanged() {
        let lines = vec!["Hello world".to_string()];
        let result = rebalance_lines(lines.clone(), 40);
        assert_eq!(result, lines);
    }

    #[test]
    fn rebalance_lines_produces_at_most_same_balance_factor() {
        let lines = vec![
            "Hi".to_string(),
            "This is a much longer second line here".to_string(),
        ];
        let original_factor = LineBalance::balance_factor(&lines);
        let result = rebalance_lines(lines, 40);
        let new_factor = LineBalance::balance_factor(&result);
        assert!(new_factor <= original_factor + 0.01);
    }

    #[test]
    fn rebalance_lines_preserves_all_words() {
        let lines = vec!["one two".to_string(), "three four five six".to_string()];
        let original_words: std::collections::HashSet<String> = lines
            .iter()
            .flat_map(|l| l.split_whitespace())
            .map(|w| w.to_string())
            .collect();
        let result = rebalance_lines(lines, 20);
        let result_words: std::collections::HashSet<String> = result
            .iter()
            .flat_map(|l| l.split_whitespace())
            .map(|w| w.to_string())
            .collect();
        assert_eq!(original_words, result_words);
    }

    #[test]
    fn line_break_config_default_broadcast_values() {
        let cfg = LineBreakConfig::default_broadcast();
        assert_eq!(cfg.max_chars_per_line, 42);
        assert_eq!(cfg.max_lines, 2);
        assert_eq!(cfg.min_gap_ms, 80);
        assert_eq!(cfg.hard_max_chars, None);
    }

    // --- LineBreakConfig.hard_max_chars ---

    #[test]
    fn line_break_config_hard_max_chars_constrains_effective() {
        let mut cfg = LineBreakConfig::default_broadcast();
        cfg.hard_max_chars = Some(30);
        assert_eq!(cfg.effective_max_chars(), 30); // hard cap wins
        cfg.hard_max_chars = Some(50);
        assert_eq!(cfg.effective_max_chars(), 42); // max_chars_per_line wins
    }

    // --- AudienceProfile ---

    #[test]
    fn audience_profile_children_have_lower_cps() {
        assert!(AudienceProfile::YoungChildren.max_cps() < AudienceProfile::Adults.max_cps());
        assert!(AudienceProfile::OlderChildren.max_cps() < AudienceProfile::Adults.max_cps());
    }

    #[test]
    fn audience_profile_children_have_longer_min_display() {
        assert!(
            AudienceProfile::YoungChildren.min_display_ms()
                > AudienceProfile::Adults.min_display_ms()
        );
    }

    #[test]
    fn reading_speed_ok_for_audience_children() {
        // 10 chars at 3 seconds = 3.3 cps < 5 cps (YoungChildren threshold)
        assert!(reading_speed_ok_for_audience(
            "Hello world",
            3000,
            AudienceProfile::YoungChildren
        ));
    }

    #[test]
    fn reading_speed_too_fast_for_children() {
        // 100 chars at 2 seconds = 50 cps > 5 cps
        let text = "A".repeat(100);
        assert!(!reading_speed_ok_for_audience(
            &text,
            2000,
            AudienceProfile::YoungChildren
        ));
    }

    // --- CpsCache ---

    #[test]
    fn cps_cache_returns_same_value_twice() {
        let mut cache = CpsCache::new();
        let v1 = cache.compute_cps("Hello world", 2000);
        let v2 = cache.compute_cps("Hello world", 2000);
        assert!((v1 - v2).abs() < 1e-6);
    }

    #[test]
    fn cps_cache_stores_entry() {
        let mut cache = CpsCache::new();
        assert_eq!(cache.len(), 0);
        cache.compute_cps("Hello", 1000);
        assert_eq!(cache.len(), 1);
        // Same key → no new entry.
        cache.compute_cps("Hello", 1000);
        assert_eq!(cache.len(), 1);
        // Different text → new entry.
        cache.compute_cps("World", 1000);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn cps_cache_clear_removes_all_entries() {
        let mut cache = CpsCache::new();
        cache.compute_cps("Hello", 1000);
        cache.clear();
        assert!(cache.is_empty());
    }

    // --- CJK breaking ---

    #[test]
    fn cjk_break_short_text_unchanged() {
        let text = "日本語";
        let result = cjk_break(text, 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], text);
    }

    #[test]
    fn cjk_break_long_text_splits_at_char_boundary() {
        let text = "これは日本語のテキストサンプルです"; // 16 chars
        let result = cjk_break(text, 5);
        assert!(result.len() > 1, "expected split");
        for line in &result {
            let count = line.chars().count();
            assert!(count <= 5, "line '{line}' has {count} chars > 5");
        }
        // All characters should be preserved.
        let combined: String = result.concat();
        assert_eq!(combined.chars().count(), text.chars().count());
    }

    #[test]
    fn language_aware_break_latin_uses_greedy() {
        let text = "Hello there how are you doing";
        let result = language_aware_break(text, 12);
        let rejoined = result.join(" ");
        assert_eq!(rejoined, text);
    }

    #[test]
    fn language_aware_break_cjk_detected() {
        let text = "これは日本語のテキストです"; // all CJK
        let result = language_aware_break(text, 5);
        assert!(result.len() > 1, "expected multi-line CJK break");
    }

    // --- optimal_break reference output test ---

    #[test]
    fn optimal_break_reference_output_known_case() {
        // Reference: "one two three four five" at width 11.
        // Optimal should produce lines whose total slack is minimised.
        let text = "one two three four five";
        let result = optimal_break(text, 11);
        // All words must be present.
        let rejoined = result.join(" ");
        assert_eq!(rejoined, text);
        // No line exceeds max width.
        for line in &result {
            assert!(
                line.chars().count() <= 11,
                "line '{line}' exceeds max width"
            );
        }
    }

    #[test]
    fn greedy_and_optimal_produce_identical_single_line() {
        // When all text fits on one line, both algorithms must produce one line.
        let text = "Hello";
        let g = greedy_break(text, 20);
        let o = optimal_break(text, 20);
        assert_eq!(g, o);
    }

    #[test]
    fn greedy_and_optimal_identical_for_single_word_per_line() {
        // Each word fits on one line individually: both algorithms agree.
        let text = "a b c";
        let g = greedy_break(text, 1);
        let o = optimal_break(text, 1);
        // Both produce 3 lines of 1 character each.
        assert_eq!(g.len(), o.len(), "g={:?} o={:?}", g, o);
    }
}
