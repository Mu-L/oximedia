//! Accessibility tools for inclusive media production in `OxiMedia`.
//!
//! `oximedia-access` provides comprehensive accessibility features for media content:
//!
//! - **Audio Description** - Generate and manage descriptive audio for visual content
//! - **Closed Captions** - Advanced caption generation and styling
//! - **Sign Language** - Sign language video overlay support
//! - **Transcripts** - Generate text transcripts from audio
//! - **Translation** - Translate subtitles to multiple languages
//! - **Text-to-Speech** - Convert text to natural speech
//! - **Speech-to-Text** - Transcribe spoken content
//! - **Visual Enhancement** - Contrast and color blindness adaptation
//! - **Audio Enhancement** - Clarity and noise reduction
//! - **Speed Control** - Adjustable playback with pitch preservation
//! - **Compliance** - WCAG, Section 508, and EBU compliance checking
//!
//! # Audio Description
//!
//! Audio description provides narration of visual content for blind and visually impaired users:
//!
//! ```ignore
//! use oximedia_access::audio_desc::{AudioDescriptionGenerator, AudioDescriptionType};
//! use oximedia_access::audio_desc::script::AudioDescriptionScript;
//!
//! // Create script
//! let mut script = AudioDescriptionScript::new();
//! script.add_entry(1000, 3000, "A sunset over mountains.");
//! script.add_entry(5000, 7000, "Characters walk through forest.");
//!
//! // Generate audio description
//! let generator = AudioDescriptionGenerator::new();
//! let ad_audio = generator.generate(&script, AudioDescriptionType::Standard)?;
//!
//! // Mix into main audio
//! let mixer = AudioDescriptionMixer::new();
//! mixer.mix(main_audio, ad_audio)?;
//! ```
//!
//! # Closed Captions
//!
//! Generate and style closed captions with smart positioning:
//!
//! ```ignore
//! use oximedia_access::caption::{CaptionGenerator, CaptionStyle};
//!
//! let generator = CaptionGenerator::new();
//! let captions = generator.generate_from_audio(audio_data)?;
//!
//! let style = CaptionStyle::default()
//!     .with_font_size(42)
//!     .with_background_color(0, 0, 0, 200);
//! ```
//!
//! # Compliance Standards
//!
//! Check content against accessibility standards:
//!
//! ```ignore
//! use oximedia_access::compliance::{WcagChecker, ComplianceLevel};
//!
//! let checker = WcagChecker::new(ComplianceLevel::AA);
//! let report = checker.check_media(media_file)?;
//!
//! if report.is_compliant() {
//!     println!("Content meets WCAG 2.1 Level AA");
//! }
//! ```
//!
//! # Features
//!
//! - **Multi-language Support**: Translate to 20+ languages
//! - **Voice Selection**: Multiple TTS voices and styles
//! - **Smart Timing**: Auto-place audio descriptions in dialogue gaps
//! - **Style Templates**: Pre-configured caption styles for different uses
//! - **Compliance Validation**: Automated checking against accessibility standards
//! - **Batch Processing**: Process entire media libraries
//! - **Quality Control**: Verify accuracy and synchronization

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod access_log;
pub mod audio;
pub mod audio_desc;
pub mod audit;
pub mod caption;
pub mod cognitive_load;
pub mod color_blind;
pub mod compliance;
pub mod content_filter;
mod error;
pub mod extended_desc;
pub mod focus_manager;
pub mod high_contrast;
pub mod keyboard_nav;
pub mod live_caption;
pub mod login_rate;
pub mod media_alt_text;
pub mod navigation_landmark;
pub mod permission_set;
pub mod rbac;
pub mod reading_level;
pub mod screen_reader;
pub mod session_manager;
pub mod sign;
pub mod speed;
pub mod stt;
pub mod token;
pub mod transcript;
pub mod translate;
pub mod tts;
pub mod user_group;
pub mod visual;
pub mod wcag;

// Re-export main types
pub use error::{AccessError, AccessResult};

// Re-export key types from each module
pub use audio::AudioClarityEnhancer;
pub use audio_desc::{AudioDescriptionGenerator, AudioDescriptionMixer, AudioDescriptionType};
pub use caption::{CaptionGenerator, CaptionStyle};
pub use compliance::{ComplianceChecker, ComplianceReport};
pub use sign::SignLanguageOverlay;
pub use speed::SpeedController;
pub use stt::SpeechToText;
pub use transcript::TranscriptGenerator;
pub use translate::SubtitleTranslator;
pub use tts::TextToSpeech;
pub use visual::ContrastEnhancer;
