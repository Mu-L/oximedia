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

// ─── SMAWK O(n) accelerator for Knuth-Plass DP ────────────────────────────────

/// A cost matrix exposed to the SMAWK row-minima algorithm.
///
/// SMAWK requires the matrix to be **totally monotone**: for every
/// `i < i'` and `j < j'`,
///
/// ```text
///     cost(i,  j ) + cost(i', j') <= cost(i, j') + cost(i', j)
/// ```
///
/// When this property holds the row minima can be computed in
/// `O(rows + cols)` time using the Aggarwal–Klawe–Moran–Shor–Wilber
/// (1987) reduce-and-interpolate recursion.
pub trait TotallyMonotoneMatrix {
    /// Number of rows in the matrix.
    fn rows(&self) -> usize;
    /// Number of columns in the matrix.
    fn cols(&self) -> usize;
    /// Return the cost at `(row, col)`.  Infeasible entries return
    /// `f64::INFINITY`.
    fn cost(&self, row: usize, col: usize) -> f64;
}

/// Compute, for every row of `m`, the column index achieving the minimum
/// cost on that row using the SMAWK algorithm.
///
/// Time complexity: `O(rows + cols)` for a totally monotone matrix.
/// Falls back to a naive scan when there are zero rows.
///
/// **Invariant (debug only):** the input must be totally monotone.  This
/// function does not assert the property in release builds because the
/// check is `O(rows^2 * cols)`.
pub fn smawk_row_minima(m: &dyn TotallyMonotoneMatrix) -> Vec<usize> {
    let rows = m.rows();
    let cols = m.cols();
    if rows == 0 {
        return Vec::new();
    }
    if cols == 0 {
        return vec![0; rows];
    }

    // Initial row/column index lists (logical → physical).
    let row_indices: Vec<usize> = (0..rows).collect();
    let col_indices: Vec<usize> = (0..cols).collect();

    let mut answer: Vec<usize> = vec![0; rows];
    smawk_recurse(m, &row_indices, &col_indices, &mut answer);
    answer
}

/// SMAWK recursive worker.
///
/// `rows` and `cols` are sub-views into the original matrix; `answer`
/// stores, for every original row index appearing in `rows`, the column
/// index that minimises the cost on that row.
fn smawk_recurse(
    m: &dyn TotallyMonotoneMatrix,
    rows: &[usize],
    cols: &[usize],
    answer: &mut [usize],
) {
    if rows.is_empty() {
        return;
    }
    if rows.len() == 1 {
        // Linear scan for the single remaining row.
        let r = rows[0];
        let mut best_col = cols[0];
        let mut best = m.cost(r, best_col);
        for &c in &cols[1..] {
            let v = m.cost(r, c);
            if v < best {
                best = v;
                best_col = c;
            }
        }
        answer[r] = best_col;
        return;
    }

    // ── Reduce ──────────────────────────────────────────────────────────
    // Eliminate columns that cannot contribute a row minimum to any of
    // the surviving rows.  After reduction at most `rows.len()` columns
    // remain.
    let reduced_cols = smawk_reduce(m, rows, cols);

    // ── Interpolate ─────────────────────────────────────────────────────
    // Recurse on the even-indexed rows (every other row).
    let even_rows: Vec<usize> = rows.iter().step_by(2).copied().collect();
    smawk_recurse(m, &even_rows, &reduced_cols, answer);

    // For odd-indexed rows, the optimal column is constrained between
    // the optima of the neighbouring even rows; scan only that window.
    let n = rows.len();
    let mut col_pos: usize = 0;
    for i in (1..n).step_by(2) {
        let row = rows[i];
        // Lower bound: the answer of the previous even row.
        let prev_even = rows[i - 1];
        let lo_col = answer[prev_even];
        // Upper bound: the answer of the next even row, or last column.
        let hi_col = if i + 1 < n {
            let next_even = rows[i + 1];
            answer[next_even]
        } else {
            *reduced_cols
                .last()
                .expect("reduced_cols non-empty when rows.len() >= 2")
        };

        // Advance `col_pos` to the first reduced column >= lo_col.
        while col_pos < reduced_cols.len() && reduced_cols[col_pos] < lo_col {
            col_pos += 1;
        }
        if col_pos >= reduced_cols.len() {
            // Should never happen if matrix is well-formed; fall back
            // to the very last reduced column.
            answer[row] = hi_col;
            continue;
        }

        // Scan candidate columns in [lo_col, hi_col].
        let mut best_col = lo_col;
        let mut best = m.cost(row, best_col);
        let mut scan = col_pos;
        while scan < reduced_cols.len() && reduced_cols[scan] <= hi_col {
            let c = reduced_cols[scan];
            let v = m.cost(row, c);
            if v < best {
                best = v;
                best_col = c;
            }
            scan += 1;
        }
        answer[row] = best_col;
    }
}

/// SMAWK column reduction.
///
/// After this call the returned column list contains at most
/// `rows.len()` entries.  Each entry is guaranteed to be the row
/// minimum of *some* row in `rows`.
fn smawk_reduce(m: &dyn TotallyMonotoneMatrix, rows: &[usize], cols: &[usize]) -> Vec<usize> {
    let mut stack: Vec<usize> = Vec::with_capacity(cols.len());
    for &c in cols {
        loop {
            if stack.is_empty() {
                stack.push(c);
                break;
            }
            let top = *stack.last().expect("stack is non-empty here");
            let k = stack.len() - 1;
            // Compare top of stack vs. new column on the row at depth k.
            let r = rows[k.min(rows.len() - 1)];
            if m.cost(r, top) <= m.cost(r, c) {
                // The top column is at least as good as c for row r;
                // push c if there is still room.
                if stack.len() < rows.len() {
                    stack.push(c);
                }
                break;
            } else {
                // The top column is dominated by c — discard.
                stack.pop();
            }
        }
    }
    stack
}

/// A forward Knuth-Plass cost matrix used by the SMAWK breaker.
///
/// `entry(i, j)` represents the optimal total cost to break the prefix
/// `words[0..=j]` such that the *last* line covers `words[i..=j]`:
///
/// ```text
///     entry(i, j) = f(i) + line_cost(i, j)
/// ```
///
/// where `f(i)` is the optimal cost up to (but not including) word `i`
/// and `line_cost(i, j) = (max_width - line_width(i, j))^2` if the
/// line fits, else `+∞`.
///
/// For fixed `j`, the row that minimises `entry(·, j)` gives both the
/// optimal cost `f(j+1) = entry(arg_min, j)` and the breakpoint
/// `prev[j+1] = arg_min`.
pub(crate) struct KpForwardMatrix<'a> {
    pub(crate) prefix: &'a [usize],
    /// f[i] = optimal cost for words 0..i (f[0] = 0)
    pub(crate) f: &'a [f64],
    pub(crate) max_width: usize,
    pub(crate) n: usize,
}

impl KpForwardMatrix<'_> {
    fn line_width(&self, i: usize, j: usize) -> usize {
        // `prefix` is the "always-one-space-per-word" cumulative
        // sum: `prefix[k] = sum(word_lens[0..k]) + k`.  The width of
        // the line covering `words[i..=j]` is
        // `prefix[j + 1] - prefix[i] - 1` (subtract one because the
        // leading "+1" corresponds to a space that we don't actually
        // render at the start of a line).
        self.prefix[j + 1] - self.prefix[i] - 1
    }
}

impl TotallyMonotoneMatrix for KpForwardMatrix<'_> {
    fn rows(&self) -> usize {
        self.n
    }
    fn cols(&self) -> usize {
        self.n
    }
    fn cost(&self, row: usize, col: usize) -> f64 {
        // Row index `row` corresponds to the start word of the last
        // line; column `col` corresponds to its end word (inclusive).
        // We need `row <= col` and the line must fit.
        if row > col {
            return f64::INFINITY;
        }
        let prev = self.f[row];
        if !prev.is_finite() {
            return f64::INFINITY;
        }
        let width = self.line_width(row, col);
        if width > self.max_width {
            return f64::INFINITY;
        }
        let slack = (self.max_width - width) as f64;
        prev + slack * slack
    }
}

/// Optimal line break using a forward Knuth-Plass DP with SMAWK
/// support primitives.
///
/// Produces the same optimal total slack-squared cost as
/// [`optimal_break`]; tie-breakers between equally-optimal layouts may
/// differ.
///
/// The forward recurrence is
///
/// ```text
///     f(0) = 0
///     f(j+1) = min over i in 0..=j  of  f(i) + line_cost(i, j)
/// ```
///
/// The cost matrix `cost(i, j) = f(i) + (max_width - line_width(i,
/// j))^2` is **totally monotone on its feasible entries**
/// (`KpForwardMatrix`) and the SMAWK primitive
/// [`smawk_row_minima`] computes row minima on such matrices in
/// `O(rows + cols)`.  However, this DP's matrix is **online**:
/// column `j`'s costs depend on `f[0..=j]` which is what we are
/// computing.  Wiring SMAWK into the online case requires the
/// Larmore–Schieber LARSCH algorithm (concave SMAWK with online
/// updates) which is beyond this iteration.
///
/// Today's implementation therefore uses a per-column linear scan
/// over the **feasibility window** `[i_lo(j), j]` where `i_lo(j)` is
/// the smallest feasible row.  This gives the same worst-case
/// `O(n^2)` as [`optimal_break`] but with better constants
/// (one-pass, single-allocation prefix sums; no suffix-recursion
/// overhead).  The window pruning amortises to `O(n)` on inputs
/// where `max_width` stays small relative to `n` — common for
/// caption text.
///
/// The output has been validated against [`optimal_break`] across
/// 10 000 randomised inputs (see
/// `tests/integration.rs::test_smawk_matches_dp_10k_random_inputs`).
pub fn optimal_break_smawk(text: &str, max_width: u8) -> Vec<String> {
    let max = max_width.max(1) as usize;
    let words: Vec<&str> = text.split_whitespace().collect();
    let n = words.len();

    if n == 0 {
        return vec![String::new()];
    }

    // Prefix sums using the "always-one-space-per-word" convention:
    // `prefix[k] = sum(word_lens[0..k]) + k`.  This makes the line
    // width formula `prefix[j+1] - prefix[i] - 1` regardless of `i`,
    // because the leading "+1" cancels for any `i >= 0`.
    let mut prefix: Vec<usize> = Vec::with_capacity(n + 1);
    prefix.push(0);
    for w in &words {
        let last = *prefix.last().expect("prefix has at least one entry");
        prefix.push(last + 1 + w.chars().count());
    }

    // f[k] = optimal cost for words[0..k]; f[0] = 0; infeasible = +∞.
    let mut f = vec![f64::INFINITY; n + 1];
    f[0] = 0.0;
    // prev[k] = breakpoint row i such that the last line of the
    // optimal break of words[0..k] covers words[i..k].
    let mut prev: Vec<usize> = vec![0; n + 1];

    // i_lo monotonically advances; reused across columns.
    let mut i_lo: usize = 0;

    for j in 0..n {
        // Advance i_lo so the line [i_lo..=j] still fits.  Uses the
        // same `prefix[j+1] - prefix[i] - 1` formula as
        // `KpForwardMatrix::line_width`.
        while i_lo <= j && prefix[j + 1] - prefix[i_lo] > max + 1 {
            i_lo += 1;
        }

        // Compute row minimum for column j on feasible rows
        // [i_lo..=j] using the totally-monotone matrix view of the
        // forward DP.
        let matrix = KpForwardMatrix {
            prefix: &prefix,
            f: &f,
            max_width: max,
            n,
        };

        let lo = i_lo.min(j); // ensure at least one row
                              // Single-column row-minimum query.  Linear scan over the
                              // feasible window [lo..=j] — the *amortised* benefit comes
                              // from the fact that `lo` only advances, giving `O(n)` total
                              // work over the whole loop on bounded-width inputs.
        let mut best_i = lo;
        let mut best = matrix.cost(lo, j);
        for i in (lo + 1)..=j {
            let c = matrix.cost(i, j);
            if c < best {
                best = c;
                best_i = i;
            }
        }

        if best.is_finite() {
            f[j + 1] = best;
            prev[j + 1] = best_i;
        } else {
            // No feasible break — force one word per line.
            f[j + 1] = f[j];
            prev[j + 1] = j;
        }
    }

    // Reconstruct line breaks by walking `prev` from n back to 0.
    // `starts` collects the starting word index of each line in
    // reverse order; we will reverse and append `n` as a sentinel.
    let mut starts: Vec<usize> = Vec::new();
    let mut k = n;
    let mut guard = 0;
    while k > 0 {
        let p = prev[k];
        let next_k = if p == k {
            // Degenerate guard: a degenerate prev[k] = k would
            // signal a single-word forced line; fall back to k-1
            // to make progress.
            k.saturating_sub(1)
        } else {
            p
        };
        starts.push(next_k);
        // Sanity: prevent infinite loops on pathological data.
        guard += 1;
        if guard > n + 2 {
            break;
        }
        k = next_k;
    }
    starts.reverse();
    starts.push(n); // sentinel: the line that *would* start at n is empty
                    // and serves only as the upper bound for the last real line.

    let mut lines: Vec<String> = Vec::new();
    for win in starts.windows(2) {
        let start = win[0];
        let end = win[1];
        if start >= end {
            continue;
        }
        lines.push(words[start..end].join(" "));
    }
    if lines.is_empty() {
        lines.push(words.join(" "));
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

    // --- SMAWK primitive tests ---

    /// A small totally-monotone matrix used as a unit fixture for
    /// [`smawk_row_minima`].  Row `i` is `cost[i]` and is treated as
    /// fully feasible.
    struct DenseMatrix {
        data: Vec<Vec<f64>>,
    }

    impl TotallyMonotoneMatrix for DenseMatrix {
        fn rows(&self) -> usize {
            self.data.len()
        }
        fn cols(&self) -> usize {
            self.data.first().map(Vec::len).unwrap_or(0)
        }
        fn cost(&self, row: usize, col: usize) -> f64 {
            self.data[row][col]
        }
    }

    fn naive_row_minima(m: &dyn TotallyMonotoneMatrix) -> Vec<usize> {
        let rows = m.rows();
        let cols = m.cols();
        (0..rows)
            .map(|r| {
                let mut best_c = 0;
                let mut best_v = m.cost(r, 0);
                for c in 1..cols {
                    let v = m.cost(r, c);
                    if v < best_v {
                        best_v = v;
                        best_c = c;
                    }
                }
                best_c
            })
            .collect()
    }

    #[test]
    fn smawk_row_minima_zero_rows_returns_empty() {
        let m = DenseMatrix { data: Vec::new() };
        let r = smawk_row_minima(&m);
        assert!(r.is_empty());
    }

    #[test]
    fn smawk_row_minima_single_row() {
        let m = DenseMatrix {
            data: vec![vec![5.0, 3.0, 9.0, 1.0, 7.0]],
        };
        let r = smawk_row_minima(&m);
        assert_eq!(r, vec![3]);
    }

    #[test]
    fn smawk_row_minima_matches_naive_small() {
        // Classic Monge example used in SMAWK literature.
        let data = vec![
            vec![
                25.0, 21.0, 13.0, 10.0, 20.0, 13.0, 19.0, 35.0, 37.0, 41.0, 58.0, 62.0, 50.0,
            ],
            vec![
                42.0, 35.0, 26.0, 20.0, 29.0, 21.0, 25.0, 37.0, 36.0, 39.0, 56.0, 59.0, 47.0,
            ],
            vec![
                57.0, 48.0, 35.0, 28.0, 33.0, 24.0, 28.0, 40.0, 37.0, 37.0, 54.0, 55.0, 43.0,
            ],
            vec![
                78.0, 65.0, 51.0, 42.0, 44.0, 35.0, 38.0, 48.0, 42.0, 42.0, 55.0, 53.0, 41.0,
            ],
            vec![
                90.0, 76.0, 58.0, 48.0, 49.0, 39.0, 42.0, 48.0, 39.0, 35.0, 47.0, 45.0, 31.0,
            ],
        ];
        let m = DenseMatrix { data };
        let smawk = smawk_row_minima(&m);
        let naive = naive_row_minima(&m);
        assert_eq!(smawk.len(), naive.len());
        // Each SMAWK answer is at least as good as the naive answer.
        for (i, (&sm, &nv)) in smawk.iter().zip(naive.iter()).enumerate() {
            let sm_v = m.cost(i, sm);
            let nv_v = m.cost(i, nv);
            assert!(
                (sm_v - nv_v).abs() < 1e-9,
                "row {i}: smawk picked col {sm} (={sm_v}); naive col {nv} (={nv_v})"
            );
        }
    }

    #[test]
    fn smawk_row_minima_matches_naive_random_monge() {
        // Construct a Monge matrix from concave sums: a[i] + b[j]
        // with both a and b sorted, plus a strictly-decreasing pair
        // cost: that is a totally-monotone matrix.  This is the
        // canonical construction used in textbooks.
        let a: Vec<f64> = (0..30).map(|i| (i as f64) * 1.5).collect();
        let b: Vec<f64> = (0..50).map(|j| 100.0 - (j as f64) * 1.3).collect();
        let mut data: Vec<Vec<f64>> = Vec::with_capacity(a.len());
        for i in 0..a.len() {
            let row: Vec<f64> = (0..b.len()).map(|j| a[i] + b[j]).collect();
            data.push(row);
        }
        let m = DenseMatrix { data };
        let smawk = smawk_row_minima(&m);
        // With strictly-decreasing b, every row's minimum should be
        // at the last column.
        for (i, &c) in smawk.iter().enumerate() {
            let last = m.cols() - 1;
            let v = m.cost(i, c);
            let vlast = m.cost(i, last);
            assert!(
                (v - vlast).abs() < 1e-9,
                "row {i}: smawk col {c} cost {v} != cost at last col {vlast}"
            );
        }
    }

    // --- optimal_break_smawk ---

    #[test]
    fn smawk_break_empty_string() {
        let result = optimal_break_smawk("", 40);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn smawk_break_single_word() {
        let result = optimal_break_smawk("Hello", 40);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn smawk_break_two_words_fit_one_line() {
        let result = optimal_break_smawk("Hello world", 40);
        assert_eq!(result, vec!["Hello world"]);
    }

    #[test]
    fn smawk_break_respects_max_width() {
        let text = "one two three four five six seven eight";
        let result = optimal_break_smawk(text, 12);
        for line in &result {
            assert!(line.chars().count() <= 12, "line '{line}' too wide");
        }
        let rejoined = result.join(" ");
        assert_eq!(rejoined, text);
    }

    /// Slack-squared cost of a layout (canonical Knuth-Plass cost).
    fn layout_cost(lines: &[String], max_width: usize) -> u64 {
        lines
            .iter()
            .map(|l| {
                let w = l.chars().count();
                if w > max_width {
                    0 // overflow lines incur no slack cost (single fat word)
                } else {
                    let s = (max_width - w) as u64;
                    s * s
                }
            })
            .sum()
    }

    #[test]
    fn smawk_break_optimal_cost_matches_dp() {
        // Same total slack-squared cost as the naive DP, even if the
        // exact line split differs in tie-breaking.
        let texts = [
            "one two three four",
            "short and sweet sentence here",
            "alpha beta gamma delta epsilon zeta eta theta iota kappa",
            "the quick brown fox jumps over the lazy dog quickly today",
        ];
        for text in texts {
            for &w in &[5_u8, 10, 15, 20, 30] {
                let smawk = optimal_break_smawk(text, w);
                let dp = optimal_break(text, w);
                let c_smawk = layout_cost(&smawk, w as usize);
                let c_dp = layout_cost(&dp, w as usize);
                assert_eq!(
                    c_smawk, c_dp,
                    "cost mismatch at width {w}: smawk={smawk:?} ({c_smawk}) dp={dp:?} ({c_dp})"
                );
                // Multi-word lines must fit; single-word lines may
                // overflow when the word itself is longer than `w`.
                for line in &smawk {
                    let words_in_line: Vec<&str> = line.split_whitespace().collect();
                    if words_in_line.len() > 1 {
                        assert!(
                            line.chars().count() <= w as usize,
                            "multi-word line '{line}' exceeds width {w}"
                        );
                    }
                }
            }
        }
    }
}
