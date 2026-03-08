//! Integration tests for oximedia-gaming.

use oximedia_gaming::{CaptureSource, EncoderPreset, GameStreamer, StreamConfig};

#[tokio::test]
async fn test_basic_streaming_lifecycle() {
    let config = StreamConfig::builder()
        .source(CaptureSource::PrimaryMonitor)
        .resolution(1920, 1080)
        .framerate(60)
        .bitrate(6000)
        .build()
        .unwrap();

    let mut streamer = GameStreamer::new(config).await.unwrap();

    // Start streaming
    streamer.start().await.unwrap();
    assert!(streamer.is_streaming());

    // Check stats
    let stats = streamer.get_stats();
    assert_eq!(stats.fps, 60);
    assert_eq!(stats.bitrate, 6000);

    // Stop streaming
    streamer.stop().await.unwrap();
    assert!(!streamer.is_streaming());
}

#[tokio::test]
async fn test_pause_resume() {
    let config = StreamConfig::builder().build().unwrap();
    let mut streamer = GameStreamer::new(config).await.unwrap();

    streamer.start().await.unwrap();
    assert!(streamer.is_streaming());

    streamer.pause().unwrap();
    assert!(!streamer.is_streaming());

    streamer.resume().unwrap();
    assert!(streamer.is_streaming());

    streamer.stop().await.unwrap();
}

#[tokio::test]
async fn test_replay_buffer_integration() {
    let config = StreamConfig::builder().replay_buffer(30).build().unwrap();

    let mut streamer = GameStreamer::new(config).await.unwrap();
    streamer.enable_replay_buffer(60).unwrap();

    streamer.start().await.unwrap();

    // Save replay
    streamer.save_replay("/tmp/test_replay.mp4").await.unwrap();

    streamer.stop().await.unwrap();
}

#[test]
fn test_all_resolutions() {
    let resolutions = [
        (1280, 720),  // 720p
        (1920, 1080), // 1080p
        (2560, 1440), // 1440p
        (3840, 2160), // 4K
    ];

    for (width, height) in resolutions {
        let config = StreamConfig::builder()
            .resolution(width, height)
            .build()
            .unwrap();

        assert_eq!(config.resolution, (width, height));
    }
}

#[test]
fn test_all_framerates() {
    let framerates = [30, 60, 120, 144, 240];

    for fps in framerates {
        let config = StreamConfig::builder().framerate(fps).build().unwrap();

        assert_eq!(config.framerate, fps);
    }
}

#[test]
fn test_encoder_presets() {
    let presets = [
        EncoderPreset::UltraLowLatency,
        EncoderPreset::LowLatency,
        EncoderPreset::Balanced,
        EncoderPreset::HighQuality,
    ];

    for preset in presets {
        let config = StreamConfig::builder()
            .encoder_preset(preset)
            .build()
            .unwrap();

        assert_eq!(config.encoder_preset, preset);
    }
}

#[test]
fn test_capture_sources() {
    let sources = [
        CaptureSource::PrimaryMonitor,
        CaptureSource::Monitor(0),
        CaptureSource::Monitor(1),
        CaptureSource::Window,
        CaptureSource::Region,
    ];

    for source in sources {
        let config = StreamConfig::builder().source(source).build().unwrap();

        assert_eq!(config.source, source);
    }
}

#[test]
fn test_bitrate_ranges() {
    let bitrates = [
        500,   // Minimum
        2500,  // Low
        6000,  // Standard
        10000, // High
        20000, // Very High
        40000, // 4K
    ];

    for bitrate in bitrates {
        let config = StreamConfig::builder().bitrate(bitrate).build().unwrap();

        assert_eq!(config.bitrate, bitrate);
    }
}

#[tokio::test]
async fn test_high_framerate_streaming() {
    let config = StreamConfig::builder()
        .resolution(1920, 1080)
        .framerate(144)
        .encoder_preset(EncoderPreset::UltraLowLatency)
        .bitrate(10000)
        .build()
        .unwrap();

    let mut streamer = GameStreamer::new(config).await.unwrap();
    streamer.start().await.unwrap();

    let stats = streamer.get_stats();
    assert_eq!(stats.fps, 144);

    streamer.stop().await.unwrap();
}

#[tokio::test]
async fn test_4k_streaming() {
    let config = StreamConfig::builder()
        .resolution(3840, 2160)
        .framerate(60)
        .encoder_preset(EncoderPreset::HighQuality)
        .bitrate(20000)
        .build()
        .unwrap();

    let mut streamer = GameStreamer::new(config).await.unwrap();
    streamer.start().await.unwrap();

    let stats = streamer.get_stats();
    assert_eq!(stats.bitrate, 20000);

    streamer.stop().await.unwrap();
}

#[test]
fn test_webcam_microphone_integration() {
    let config = StreamConfig::builder()
        .webcam(true)
        .microphone(true)
        .build()
        .unwrap();

    assert!(config.enable_webcam);
    assert!(config.enable_microphone);
}

#[test]
fn test_config_builder_defaults() {
    let config = StreamConfig::builder().build().unwrap();

    assert_eq!(config.resolution, (1920, 1080));
    assert_eq!(config.framerate, 60);
    assert_eq!(config.bitrate, 6000);
    assert_eq!(config.encoder_preset, EncoderPreset::LowLatency);
    assert!(!config.enable_webcam);
    assert!(!config.enable_microphone);
    assert_eq!(config.replay_buffer_seconds, None);
}

#[tokio::test]
async fn test_streaming_stats_accuracy() {
    let config = StreamConfig::builder()
        .framerate(60)
        .bitrate(8000)
        .build()
        .unwrap();

    let streamer = GameStreamer::new(config).await.unwrap();
    let stats = streamer.get_stats();

    assert_eq!(stats.fps, 60);
    assert_eq!(stats.bitrate, 8000);
    assert_eq!(stats.dropped_frames, 0);
}

#[test]
fn test_invalid_configurations() {
    // Zero resolution
    assert!(StreamConfig::builder().resolution(0, 0).build().is_err());

    // Zero framerate
    assert!(StreamConfig::builder().framerate(0).build().is_err());

    // Low bitrate
    assert!(StreamConfig::builder().bitrate(100).build().is_err());

    // Very high framerate
    assert!(StreamConfig::builder().framerate(300).build().is_err());
}

#[tokio::test]
async fn test_double_start_error() {
    let config = StreamConfig::builder().build().unwrap();
    let mut streamer = GameStreamer::new(config).await.unwrap();

    streamer.start().await.unwrap();

    // Second start should fail
    assert!(streamer.start().await.is_err());

    streamer.stop().await.unwrap();
}

#[test]
fn test_pause_when_not_running() {
    let config = StreamConfig::builder().build().unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut streamer = rt.block_on(GameStreamer::new(config)).unwrap();

    // Pause when not running should fail
    assert!(streamer.pause().is_err());
}

#[test]
fn test_resume_when_not_paused() {
    let config = StreamConfig::builder().build().unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut streamer = rt.block_on(GameStreamer::new(config)).unwrap();

    // Resume when not paused should fail
    assert!(streamer.resume().is_err());
}

#[test]
fn test_replay_buffer_duration_limits() {
    let config = StreamConfig::builder().build().unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut streamer = rt.block_on(GameStreamer::new(config)).unwrap();

    // Too short
    assert!(streamer.enable_replay_buffer(2).is_err());

    // Too long
    assert!(streamer.enable_replay_buffer(400).is_err());

    // Valid
    assert!(streamer.enable_replay_buffer(30).is_ok());
    assert!(streamer.enable_replay_buffer(60).is_ok());
    assert!(streamer.enable_replay_buffer(120).is_ok());
}

#[tokio::test]
async fn test_multiple_config_changes() {
    let config = StreamConfig::builder()
        .resolution(1280, 720)
        .framerate(30)
        .build()
        .unwrap();

    let mut streamer = GameStreamer::new(config).await.unwrap();
    streamer.start().await.unwrap();

    let stats = streamer.get_stats();
    assert_eq!(stats.fps, 30);

    streamer.stop().await.unwrap();

    // Create new config
    let config2 = StreamConfig::builder()
        .resolution(1920, 1080)
        .framerate(60)
        .build()
        .unwrap();

    let mut streamer2 = GameStreamer::new(config2).await.unwrap();
    streamer2.start().await.unwrap();

    let stats2 = streamer2.get_stats();
    assert_eq!(stats2.fps, 60);

    streamer2.stop().await.unwrap();
}
