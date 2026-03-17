//! Timeline rendering.
//!
//! Renders the timeline to a stream of video and audio frames.

use oximedia_audio::{AudioFrame, ChannelLayout};
use oximedia_codec::VideoFrame;
use oximedia_core::{PixelFormat, Rational, SampleFormat, Timestamp};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;

use crate::clip::Clip;
use crate::error::EditResult;
use crate::timeline::{Timeline, TimelineConfig};
use crate::transition::Transition;

/// Timeline renderer.
pub struct TimelineRenderer {
    /// Timeline to render.
    timeline: Arc<Timeline>,
    /// Render configuration.
    config: RenderConfig,
    /// Frame cache.
    cache: FrameCache,
}

impl TimelineRenderer {
    /// Create a new timeline renderer.
    #[must_use]
    pub fn new(timeline: Arc<Timeline>, config: RenderConfig) -> Self {
        let cache_size = config.cache_size;
        Self {
            timeline,
            config,
            cache: FrameCache::new(cache_size),
        }
    }

    /// Render a frame at a specific timeline position.
    pub async fn render_frame_at(&mut self, position: i64) -> EditResult<RenderFrame> {
        // Check cache first
        if let Some(frame) = self.cache.get(position) {
            return Ok(frame);
        }

        // Find all active clips at this position
        let clips_at_pos = self.timeline.get_clips_at(position);

        // Render video layers
        let video_frame = if self.config.render_video {
            self.render_video_at(position, &clips_at_pos).await?
        } else {
            None
        };

        // Render audio
        let audio_frame = if self.config.render_audio {
            self.render_audio_at(position, &clips_at_pos).await?
        } else {
            None
        };

        let frame = RenderFrame {
            position,
            timestamp: Timestamp::new(position, self.timeline.timebase),
            video: video_frame,
            audio: audio_frame,
        };

        // Cache the frame
        self.cache.put(position, frame.clone());

        Ok(frame)
    }

    /// Render video frame at position.
    async fn render_video_at(
        &self,
        position: i64,
        clips: &[(usize, &Clip)],
    ) -> EditResult<Option<VideoFrame>> {
        let video_clips: Vec<&Clip> = clips
            .iter()
            .filter(|(_, clip)| clip.is_video())
            .map(|(_, clip)| *clip)
            .collect();

        if video_clips.is_empty() {
            return Ok(None);
        }

        // Create output frame
        let mut output = VideoFrame::new(
            self.config.pixel_format,
            self.config.width,
            self.config.height,
        );
        output.allocate();
        output.timestamp = Timestamp::new(position, self.timeline.timebase);

        // Composite video layers (top to bottom)
        for clip in video_clips.iter().rev() {
            if clip.muted {
                continue;
            }

            // Get source frame for this clip at this position
            let source_pos = clip.timeline_to_source(position);
            let _source_frame = self.get_source_frame(clip, source_pos)?;

            // Apply effects
            // Apply transitions
            // Composite onto output

            // This is a placeholder - actual implementation would:
            // 1. Load source frame from clip.source
            // 2. Apply clip.effects
            // 3. Apply clip.opacity
            // 4. Composite onto output frame
        }

        Ok(Some(output))
    }

    /// Render audio frame at position.
    async fn render_audio_at(
        &self,
        position: i64,
        clips: &[(usize, &Clip)],
    ) -> EditResult<Option<AudioFrame>> {
        let audio_clips: Vec<&Clip> = clips
            .iter()
            .filter(|(_, clip)| clip.is_audio())
            .map(|(_, clip)| *clip)
            .collect();

        if audio_clips.is_empty() {
            return Ok(None);
        }

        // Create output frame
        let mut output = AudioFrame::new(
            self.config.sample_format,
            self.config.sample_rate,
            self.config.channels.clone(),
        );
        output.timestamp = Timestamp::new(position, self.timeline.timebase);

        // Mix audio tracks
        for clip in &audio_clips {
            if clip.muted {
                continue;
            }

            let source_pos = clip.timeline_to_source(position);
            let _source_frame = self.get_source_audio_frame(clip, source_pos)?;

            // This is a placeholder - actual implementation would:
            // 1. Load source audio from clip.source
            // 2. Apply clip.effects
            // 3. Apply clip.opacity (volume)
            // 4. Mix into output frame
        }

        Ok(Some(output))
    }

    /// Get source frame from clip.
    fn get_source_frame(&self, _clip: &Clip, _position: i64) -> EditResult<VideoFrame> {
        // Placeholder - would load from clip.source file
        Ok(VideoFrame::new(
            self.config.pixel_format,
            self.config.width,
            self.config.height,
        ))
    }

    /// Get source audio frame from clip.
    fn get_source_audio_frame(&self, _clip: &Clip, _position: i64) -> EditResult<AudioFrame> {
        // Placeholder - would load from clip.source file
        Ok(AudioFrame::new(
            self.config.sample_format,
            self.config.sample_rate,
            self.config.channels.clone(),
        ))
    }

    /// Start background rendering.
    pub fn start_background_render(&mut self) -> BackgroundRenderer {
        BackgroundRenderer::new(self.timeline.clone(), self.config.clone())
    }

    /// Clear frame cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Rendered frame containing video and audio.
#[derive(Clone, Debug)]
pub struct RenderFrame {
    /// Timeline position.
    pub position: i64,
    /// Timestamp.
    pub timestamp: Timestamp,
    /// Video frame.
    pub video: Option<VideoFrame>,
    /// Audio frame.
    pub audio: Option<AudioFrame>,
}

impl RenderFrame {
    /// Check if frame has video.
    #[must_use]
    pub fn has_video(&self) -> bool {
        self.video.is_some()
    }

    /// Check if frame has audio.
    #[must_use]
    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }
}

/// Render configuration.
#[derive(Clone, Debug)]
pub struct RenderConfig {
    /// Render video.
    pub render_video: bool,
    /// Render audio.
    pub render_audio: bool,
    /// Video width.
    pub width: u32,
    /// Video height.
    pub height: u32,
    /// Pixel format.
    pub pixel_format: PixelFormat,
    /// Sample rate.
    pub sample_rate: u32,
    /// Sample format.
    pub sample_format: SampleFormat,
    /// Audio channels.
    pub channels: ChannelLayout,
    /// Frame cache size.
    pub cache_size: usize,
    /// Number of render threads.
    pub num_threads: usize,
    /// Quality preset.
    pub quality: RenderQuality,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            render_video: true,
            render_audio: true,
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::Yuv420p,
            sample_rate: 48000,
            sample_format: SampleFormat::F32,
            channels: ChannelLayout::Stereo,
            cache_size: 30,
            num_threads: 4,
            quality: RenderQuality::High,
        }
    }
}

impl RenderConfig {
    /// Create config from timeline config.
    #[must_use]
    pub fn from_timeline_config(config: &TimelineConfig) -> Self {
        Self {
            width: config.width,
            height: config.height,
            sample_rate: config.sample_rate,
            channels: ChannelLayout::from_count(config.channels as usize),
            ..Default::default()
        }
    }
}

/// Render quality preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderQuality {
    /// Draft quality (fast, low quality).
    Draft,
    /// Preview quality (balanced).
    Preview,
    /// High quality (slow, high quality).
    High,
    /// Maximum quality (very slow, maximum quality).
    Maximum,
}

impl RenderQuality {
    /// Get quality factor (0.0 to 1.0).
    #[must_use]
    pub fn factor(&self) -> f32 {
        match self {
            Self::Draft => 0.25,
            Self::Preview => 0.5,
            Self::High => 0.75,
            Self::Maximum => 1.0,
        }
    }
}

/// Frame cache for rendered frames.
#[derive(Debug)]
struct FrameCache {
    /// Cache storage.
    frames: VecDeque<(i64, RenderFrame)>,
    /// Maximum cache size.
    capacity: usize,
}

impl FrameCache {
    /// Create a new frame cache.
    fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Get a frame from cache.
    fn get(&self, position: i64) -> Option<RenderFrame> {
        self.frames
            .iter()
            .find(|(pos, _)| *pos == position)
            .map(|(_, frame)| frame.clone())
    }

    /// Put a frame into cache.
    fn put(&mut self, position: i64, frame: RenderFrame) {
        // Remove oldest if at capacity
        if self.frames.len() >= self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back((position, frame));
    }

    /// Clear the cache.
    fn clear(&mut self) {
        self.frames.clear();
    }
}

/// Background renderer for non-blocking rendering.
#[cfg(not(target_arch = "wasm32"))]
pub struct BackgroundRenderer {
    /// Timeline to render.
    timeline: Arc<Timeline>,
    /// Render configuration.
    config: RenderConfig,
    /// Render task handle.
    handle: Option<tokio::task::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BackgroundRenderer {
    /// Create a new background renderer.
    #[must_use]
    pub fn new(timeline: Arc<Timeline>, config: RenderConfig) -> Self {
        Self {
            timeline,
            config,
            handle: None,
        }
    }

    /// Start rendering in the background.
    pub fn start(&mut self, start: i64, end: i64) -> mpsc::Receiver<RenderFrame> {
        let (tx, rx) = mpsc::channel(100);
        let timeline = self.timeline.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut renderer = TimelineRenderer::new(timeline, config);

            for position in start..end {
                match renderer.render_frame_at(position).await {
                    Ok(frame) => {
                        if tx.send(frame).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        self.handle = Some(handle);
        rx
    }

    /// Stop background rendering.
    pub async fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Check if rendering is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.handle
            .as_ref()
            .map_or(true, tokio::task::JoinHandle::is_finished)
    }
}

/// Real-time preview renderer.
pub struct PreviewRenderer {
    /// Timeline renderer.
    renderer: TimelineRenderer,
    /// Target frame rate.
    frame_rate: Rational,
    /// Current position.
    position: i64,
    /// Playing state.
    playing: bool,
}

impl PreviewRenderer {
    /// Create a new preview renderer.
    #[must_use]
    pub fn new(timeline: Arc<Timeline>, config: RenderConfig) -> Self {
        let frame_rate = timeline.frame_rate;
        Self {
            renderer: TimelineRenderer::new(timeline, config),
            frame_rate,
            position: 0,
            playing: false,
        }
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) {
        self.playing = false;
        self.position = 0;
    }

    /// Get next preview frame.
    pub async fn next_frame(&mut self) -> EditResult<Option<RenderFrame>> {
        if !self.playing {
            return Ok(None);
        }

        let frame = self.renderer.render_frame_at(self.position).await?;

        // Advance position
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_precision_loss)]
        let frame_duration = (1000.0 / self.frame_rate.to_f64()) as i64;
        self.position += frame_duration;

        // Check if we've reached the end
        if self.position >= self.renderer.timeline.duration {
            self.stop();
        }

        Ok(Some(frame))
    }

    /// Seek to position.
    pub fn seek(&mut self, position: i64) {
        self.position = position.clamp(0, self.renderer.timeline.duration);
    }

    /// Get current position.
    #[must_use]
    pub fn position(&self) -> i64 {
        self.position
    }

    /// Check if playing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }
}

/// Export renderer for final output.
pub struct ExportRenderer {
    /// Timeline renderer.
    renderer: TimelineRenderer,
    /// Export settings.
    settings: ExportSettings,
}

impl ExportRenderer {
    /// Create a new export renderer.
    #[must_use]
    pub fn new(timeline: Arc<Timeline>, settings: ExportSettings) -> Self {
        let config = RenderConfig {
            render_video: settings.video_enabled,
            render_audio: settings.audio_enabled,
            width: settings.width,
            height: settings.height,
            pixel_format: settings.pixel_format,
            sample_rate: settings.sample_rate,
            sample_format: settings.sample_format,
            channels: settings.channels.clone(),
            quality: settings.quality,
            ..Default::default()
        };

        Self {
            renderer: TimelineRenderer::new(timeline, config),
            settings,
        }
    }

    /// Export timeline to frames.
    pub async fn export(&mut self) -> EditResult<Vec<RenderFrame>> {
        let mut frames = Vec::new();
        let start = self.settings.start.unwrap_or(0);
        let end = self.settings.end.unwrap_or(self.renderer.timeline.duration);

        for position in start..end {
            let frame = self.renderer.render_frame_at(position).await?;
            frames.push(frame);
        }

        Ok(frames)
    }

    /// Export timeline as a stream.
    pub fn export_stream(&mut self) -> ExportStream {
        let start = self.settings.start.unwrap_or(0);
        let end = self.settings.end.unwrap_or(self.renderer.timeline.duration);

        ExportStream {
            renderer: self.renderer.clone_for_stream(),
            current: start,
            end,
        }
    }
}

/// Export settings.
#[derive(Clone, Debug)]
pub struct ExportSettings {
    /// Export video.
    pub video_enabled: bool,
    /// Export audio.
    pub audio_enabled: bool,
    /// Video width.
    pub width: u32,
    /// Video height.
    pub height: u32,
    /// Pixel format.
    pub pixel_format: PixelFormat,
    /// Sample rate.
    pub sample_rate: u32,
    /// Sample format.
    pub sample_format: SampleFormat,
    /// Audio channels.
    pub channels: ChannelLayout,
    /// Quality preset.
    pub quality: RenderQuality,
    /// Start position (None = beginning).
    pub start: Option<i64>,
    /// End position (None = end of timeline).
    pub end: Option<i64>,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            video_enabled: true,
            audio_enabled: true,
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::Yuv420p,
            sample_rate: 48000,
            sample_format: SampleFormat::F32,
            channels: ChannelLayout::Stereo,
            quality: RenderQuality::High,
            start: None,
            end: None,
        }
    }
}

/// Stream of exported frames.
pub struct ExportStream {
    renderer: TimelineRenderer,
    current: i64,
    end: i64,
}

// Note: This is a stub implementation. The actual Stream trait requires proper
// async support with a stored future, not creating a new future in poll_next.
// Consider using `tokio::stream::StreamExt` or `futures::stream::unfold` for proper implementation.
#[allow(dead_code)]
impl ExportStream {
    /// Create an async stream from the export stream.
    /// This should be used instead of directly implementing Stream.
    pub fn into_stream(self) -> impl futures::stream::Stream<Item = EditResult<RenderFrame>> {
        futures::stream::unfold(self, |mut state| async move {
            if state.current >= state.end {
                return None;
            }
            let position = state.current;
            state.current += 1;
            let result = state.renderer.render_frame_at(position).await;
            Some((result, state))
        })
    }
}

impl TimelineRenderer {
    /// Clone renderer for streaming.
    fn clone_for_stream(&self) -> Self {
        Self {
            timeline: self.timeline.clone(),
            config: self.config.clone(),
            cache: FrameCache::new(self.config.cache_size),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RawFrameCache — byte-buffer frame cache with LRU eviction
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of raw frames kept in [`RawFrameCache`] before LRU eviction.
pub const RAW_FRAME_CACHE_CAPACITY: usize = 32;

/// A cache that stores raw pixel byte buffers (e.g. decoded video frames) keyed
/// by frame number.
///
/// When the cache reaches [`RAW_FRAME_CACHE_CAPACITY`] entries the least-recently
/// used frame is evicted before inserting the new one.
///
/// # Example
///
/// ```
/// use oximedia_edit::render::RawFrameCache;
///
/// let mut cache = RawFrameCache::new(4);
/// let data = cache.get_or_render(0, || vec![0u8; 1024]);
/// assert_eq!(data.len(), 1024);
/// ```
pub struct RawFrameCache {
    /// Frame data keyed by frame number.
    store: HashMap<u64, Vec<u8>>,
    /// Insertion-order tracking for LRU eviction (front = oldest).
    order: VecDeque<u64>,
    /// Maximum number of frames to retain.
    capacity: usize,
}

impl RawFrameCache {
    /// Create a new cache with the given capacity (clamped to at least 1).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            store: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Return a reference to the cached bytes for `frame_num`, rendering and
    /// inserting them via `render_fn` if not already present.
    ///
    /// The rendered bytes are stored in the cache; on subsequent calls the same
    /// reference is returned without invoking `render_fn`.
    ///
    /// When the cache is full the **oldest** frame is evicted first (LRU by
    /// insertion order).
    pub fn get_or_render(&mut self, frame_num: u64, render_fn: impl FnOnce() -> Vec<u8>) -> &[u8] {
        if !self.store.contains_key(&frame_num) {
            // Evict oldest entry if at capacity.
            if self.store.len() >= self.capacity {
                if let Some(oldest) = self.order.pop_front() {
                    self.store.remove(&oldest);
                }
            }

            let data = render_fn();
            self.store.insert(frame_num, data);
            self.order.push_back(frame_num);
        }

        // Safety: key is guaranteed to be present after the block above.
        self.store.get(&frame_num).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Return a reference to the cached bytes for `frame_num` without rendering.
    ///
    /// Returns `None` if the frame is not in the cache.
    #[must_use]
    pub fn get(&self, frame_num: u64) -> Option<&[u8]> {
        self.store.get(&frame_num).map(Vec::as_slice)
    }

    /// Explicitly insert pre-rendered bytes for `frame_num`.
    ///
    /// If the frame already exists it is replaced.  If the cache is full the
    /// oldest frame is evicted.
    pub fn insert(&mut self, frame_num: u64, data: Vec<u8>) {
        if let std::collections::hash_map::Entry::Occupied(mut e) = self.store.entry(frame_num) {
            e.insert(data);
            return;
        }
        if self.store.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.store.remove(&oldest);
            }
        }
        self.store.insert(frame_num, data);
        self.order.push_back(frame_num);
    }

    /// Invalidate (remove) the cache entry for `frame_num`, if present.
    pub fn invalidate(&mut self, frame_num: u64) {
        if self.store.remove(&frame_num).is_some() {
            self.order.retain(|&f| f != frame_num);
        }
    }

    /// Clear all cached frames.
    pub fn clear(&mut self) {
        self.store.clear();
        self.order.clear();
    }

    /// Return the number of frames currently in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Return `true` if the cache contains no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Return the maximum number of frames this cache holds.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return `true` if a frame for `frame_num` is present in the cache.
    #[must_use]
    pub fn contains(&self, frame_num: u64) -> bool {
        self.store.contains_key(&frame_num)
    }
}

impl std::fmt::Debug for RawFrameCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawFrameCache")
            .field("len", &self.store.len())
            .field("capacity", &self.capacity)
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for RawFrameCache
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod raw_cache_tests {
    use super::{RawFrameCache, RAW_FRAME_CACHE_CAPACITY};

    #[test]
    fn test_raw_frame_cache_basic_get_or_render() {
        let mut cache = RawFrameCache::new(4);
        let mut render_count = 0usize;

        // First call: renders.
        let data = cache.get_or_render(0, || {
            render_count += 1;
            vec![1u8, 2, 3]
        });
        assert_eq!(data, &[1u8, 2, 3]);
        assert_eq!(render_count, 1);

        // Second call: cached — render_fn must not be invoked.
        let data2 = cache.get_or_render(0, || {
            render_count += 1;
            vec![99u8]
        });
        assert_eq!(data2, &[1u8, 2, 3]);
        assert_eq!(render_count, 1, "render_fn should not be called twice");
    }

    #[test]
    fn test_raw_frame_cache_lru_eviction() {
        let mut cache = RawFrameCache::new(4);

        // Fill cache to capacity.
        for i in 0u64..4 {
            cache.get_or_render(i, || vec![i as u8]);
        }
        assert_eq!(cache.len(), 4);

        // Insert one more frame — oldest (frame 0) should be evicted.
        cache.get_or_render(4, || vec![4u8]);
        assert_eq!(cache.len(), 4, "cache must not exceed capacity");
        assert!(
            !cache.contains(0),
            "oldest frame (0) should have been evicted"
        );
        assert!(
            cache.contains(4),
            "newly inserted frame (4) should be present"
        );
    }

    #[test]
    fn test_raw_frame_cache_capacity_32() {
        let cache = RawFrameCache::new(RAW_FRAME_CACHE_CAPACITY);
        assert_eq!(cache.capacity(), 32);
    }

    #[test]
    fn test_raw_frame_cache_get_missing() {
        let cache = RawFrameCache::new(4);
        assert!(cache.get(99).is_none());
    }

    #[test]
    fn test_raw_frame_cache_insert_and_get() {
        let mut cache = RawFrameCache::new(4);
        cache.insert(7, vec![10, 20, 30]);
        assert_eq!(cache.get(7), Some(&[10u8, 20, 30][..]));
    }

    #[test]
    fn test_raw_frame_cache_invalidate() {
        let mut cache = RawFrameCache::new(4);
        cache.insert(1, vec![1, 2]);
        cache.invalidate(1);
        assert!(!cache.contains(1));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_raw_frame_cache_clear() {
        let mut cache = RawFrameCache::new(4);
        for i in 0u64..4 {
            cache.insert(i, vec![i as u8]);
        }
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_raw_frame_cache_eviction_order() {
        let mut cache = RawFrameCache::new(3);
        cache.insert(10, vec![10]);
        cache.insert(20, vec![20]);
        cache.insert(30, vec![30]);

        // Frame 40 → evicts frame 10 (oldest).
        cache.insert(40, vec![40]);
        assert!(!cache.contains(10));
        assert!(cache.contains(20));
        assert!(cache.contains(30));
        assert!(cache.contains(40));

        // Frame 50 → evicts frame 20.
        cache.insert(50, vec![50]);
        assert!(!cache.contains(20));
        assert!(cache.contains(30));
        assert!(cache.contains(40));
        assert!(cache.contains(50));
    }

    #[test]
    fn test_raw_frame_cache_capacity_clamped_to_one() {
        let cache = RawFrameCache::new(0);
        assert_eq!(cache.capacity(), 1);
    }

    #[test]
    fn test_raw_frame_cache_is_empty_initially() {
        let cache = RawFrameCache::new(8);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_raw_frame_cache_debug_format() {
        let cache = RawFrameCache::new(4);
        let debug = format!("{cache:?}");
        assert!(debug.contains("RawFrameCache"), "debug output: {debug}");
    }
}

/// Transition renderer helper.
pub struct TransitionRenderer;

impl TransitionRenderer {
    /// Blend two video frames based on transition progress.
    #[must_use]
    pub fn blend_video(
        frame_a: &VideoFrame,
        _frame_b: &VideoFrame,
        transition: &Transition,
        progress: f64,
    ) -> VideoFrame {
        let output = frame_a.clone();

        // Apply transition based on type
        if transition.transition_type == crate::transition::TransitionType::Dissolve {
            // Simple alpha blend
            // This is a placeholder - actual implementation would blend pixel data
            #[allow(clippy::cast_possible_truncation)]
            #[allow(clippy::cast_sign_loss)]
            let _ = (progress * 255.0) as u8;
        } else {
            // Other transitions would be implemented here
        }

        output
    }

    /// Mix two audio frames based on transition progress.
    #[must_use]
    pub fn mix_audio(
        frame_a: &AudioFrame,
        _frame_b: &AudioFrame,
        _transition: &Transition,
        progress: f64,
    ) -> AudioFrame {
        let output = frame_a.clone();

        // Cross-fade audio
        #[allow(clippy::cast_possible_truncation)]
        let _ = progress as f32; // Alpha value calculated above

        // This is a placeholder - actual implementation would mix sample data

        output
    }
}
