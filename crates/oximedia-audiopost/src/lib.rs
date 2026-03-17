//! Professional audio post-production suite for `OxiMedia`.
//!
//! `oximedia-audiopost` provides comprehensive audio post-production capabilities including:
//!
//! - **ADR (Automated Dialogue Replacement)**: Session management, recording, and synchronization
//! - **Foley**: Recording, editing, and library management
//! - **Sound Design**: Synthesizers, effects, and spatial audio
//! - **Mixing Console**: Professional channel strips, aux sends, and master section
//! - **Advanced Effects**: Dynamic processing, time-based effects, modulation, and spectral processing
//! - **Audio Restoration**: Noise reduction, artifact removal, and enhancement
//! - **Stem Management**: Multi-stem creation, mixing, and export
//! - **Loudness Management**: Standards compliance (EBU R128, ATSC A/85, etc.)
//! - **Automation**: Volume, pan, and parameter automation with multiple modes
//! - **Delivery**: Professional export formats and deliverable specifications
//!
//! # Example: ADR Session
//!
//! ```
//! use oximedia_audiopost::adr::{AdrSession, AdrCue};
//! use oximedia_audiopost::timecode::Timecode;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an ADR session
//! let mut session = AdrSession::new("Scene 42", 48000);
//!
//! // Add a cue
//! let cue = AdrCue::new(
//!     "Actor: 'To be or not to be'",
//!     Timecode::from_frames(1000, 24.0),
//!     Timecode::from_frames(1100, 24.0),
//! );
//! session.add_cue(cue);
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Mixing Console
//!
//! ```
//! use oximedia_audiopost::mixing::{MixingConsole, ChannelStrip};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a mixing console
//! let mut console = MixingConsole::new(48000, 512)?;
//!
//! // Add a channel
//! let channel = console.add_channel("Dialogue")?;
//!
//! // Configure the channel strip
//! console.set_channel_gain(channel, 6.0)?;
//! console.set_channel_pan(channel, 0.0)?; // Center
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_arguments)]

pub mod adr;
pub mod adr_manager;
pub mod audio_bus;
pub mod audio_report;
pub mod automation;
pub mod broadcast_delivery;
pub mod bus_routing;
pub mod channel_mapping;
pub mod clip_gain;
pub mod cue_sheet;
pub mod delivery;
pub mod delivery_spec;
pub mod dialogue;
pub mod edit_decision_audio;
pub mod effects;
pub mod error;
pub mod foley;
pub mod foley_manager;
pub mod hardware;
pub mod loudness;
pub mod loudness_session;
pub mod metering;
pub mod mix_session;
pub mod mixing;
pub mod music_licensing;
pub mod noise_profile;
pub mod phase_alignment;
pub mod pipeline;
pub mod restoration;
pub mod reverb_profile;
pub mod room_acoustics;
pub mod session;
pub mod session_template;
pub mod sound_design;
pub mod sound_library;
pub mod spectral_editor;
pub mod stem_export;
pub mod stems;
pub mod surround;
pub mod take_manager;
pub mod timecode;
pub mod timecode_chase;
pub mod track_layout;
pub mod workflow;

// Re-export commonly used items
pub use error::{AudioPostError, AudioPostResult};
pub use pipeline::{
    AudioCodec, AudioExportConfig, ContainerFormat, DialogueLeveler, SurroundFormat, SurroundPanner,
};

/// Audio post-production version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
