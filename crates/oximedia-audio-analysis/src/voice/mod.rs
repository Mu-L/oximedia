//! Voice analysis module.

pub mod age;
pub mod characteristics;
pub mod emotion;
pub mod gender;
pub mod speaker;

pub use age::estimate_age;
pub use characteristics::{VoiceAnalyzer, VoiceCharacteristics};
pub use emotion::{detect_emotion, Emotion};
pub use gender::{detect_gender, Gender};
pub use speaker::SpeakerIdentifier;
