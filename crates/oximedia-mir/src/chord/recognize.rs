//! Chord recognition using chroma-based template matching.

use crate::types::{ChordLabel, ChordResult};
use crate::utils::stft;
use crate::{MirError, MirResult};

/// Chord templates for basic chord types.
const CHORD_TEMPLATES: &[ChordTemplate] = &[
    // Major chords
    ChordTemplate {
        name: "C",
        root: 0,
        template: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "C#",
        root: 1,
        template: [0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "D",
        root: 2,
        template: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "Eb",
        root: 3,
        template: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
    },
    ChordTemplate {
        name: "E",
        root: 4,
        template: [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
    },
    ChordTemplate {
        name: "F",
        root: 5,
        template: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "F#",
        root: 6,
        template: [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    },
    ChordTemplate {
        name: "G",
        root: 7,
        template: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    },
    ChordTemplate {
        name: "Ab",
        root: 8,
        template: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "A",
        root: 9,
        template: [0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "Bb",
        root: 10,
        template: [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    },
    ChordTemplate {
        name: "B",
        root: 11,
        template: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    },
    // Minor chords
    ChordTemplate {
        name: "Cm",
        root: 0,
        template: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "C#m",
        root: 1,
        template: [0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "Dm",
        root: 2,
        template: [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "Ebm",
        root: 3,
        template: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    },
    ChordTemplate {
        name: "Em",
        root: 4,
        template: [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    },
    ChordTemplate {
        name: "Fm",
        root: 5,
        template: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "F#m",
        root: 6,
        template: [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "Gm",
        root: 7,
        template: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
    },
    ChordTemplate {
        name: "Abm",
        root: 8,
        template: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
    },
    ChordTemplate {
        name: "Am",
        root: 9,
        template: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    },
    ChordTemplate {
        name: "Bbm",
        root: 10,
        template: [0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    },
    ChordTemplate {
        name: "Bm",
        root: 11,
        template: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    },
];

/// Chord template for matching.
#[derive(Debug, Clone)]
struct ChordTemplate {
    name: &'static str,
    #[allow(dead_code)]
    root: u8,
    template: [f32; 12],
}

/// Chord recognizer.
pub struct ChordRecognizer {
    sample_rate: f32,
    window_size: usize,
    hop_size: usize,
}

impl ChordRecognizer {
    /// Create a new chord recognizer.
    #[must_use]
    pub fn new(sample_rate: f32, window_size: usize, hop_size: usize) -> Self {
        Self {
            sample_rate,
            window_size,
            hop_size,
        }
    }

    /// Recognize chords in audio signal.
    ///
    /// # Errors
    ///
    /// Returns error if chord recognition fails.
    #[allow(clippy::cast_precision_loss)]
    pub fn recognize(&self, signal: &[f32]) -> MirResult<ChordResult> {
        // Compute chromagram
        let chroma_frames = self.compute_chromagram(signal)?;

        if chroma_frames.is_empty() {
            return Err(MirError::InsufficientData(
                "No chroma frames for chord recognition".to_string(),
            ));
        }

        // Recognize chord for each frame
        let mut chord_labels = Vec::new();
        let mut current_chord: Option<(String, usize, f32)> = None; // (label, start_frame, confidence)

        for (frame_idx, chroma) in chroma_frames.iter().enumerate() {
            let (label, confidence) = self.match_chord(chroma);

            match &mut current_chord {
                Some((current_label, _start_frame, total_conf)) if current_label == &label => {
                    // Same chord continues
                    *total_conf += confidence;
                }
                _ => {
                    // New chord detected
                    if let Some((prev_label, start_frame, total_conf)) = current_chord.take() {
                        let start_time =
                            start_frame as f32 * self.hop_size as f32 / self.sample_rate;
                        let end_time = frame_idx as f32 * self.hop_size as f32 / self.sample_rate;
                        let duration = (frame_idx - start_frame) as f32;
                        let avg_confidence = if duration > 0.0 {
                            total_conf / duration
                        } else {
                            0.0
                        };

                        chord_labels.push(ChordLabel {
                            start: start_time,
                            end: end_time,
                            label: prev_label,
                            confidence: avg_confidence,
                        });
                    }
                    current_chord = Some((label, frame_idx, confidence));
                }
            }
        }

        // Add final chord
        if let Some((label, start_frame, total_conf)) = current_chord {
            let start_time = start_frame as f32 * self.hop_size as f32 / self.sample_rate;
            let end_time = chroma_frames.len() as f32 * self.hop_size as f32 / self.sample_rate;
            let duration = (chroma_frames.len() - start_frame) as f32;
            let avg_confidence = if duration > 0.0 {
                total_conf / duration
            } else {
                0.0
            };

            chord_labels.push(ChordLabel {
                start: start_time,
                end: end_time,
                label,
                confidence: avg_confidence,
            });
        }

        // Analyze chord progressions
        let progressions = self.extract_progressions(&chord_labels);

        // Compute harmonic complexity
        let complexity = self.compute_complexity(&chord_labels);

        Ok(ChordResult {
            chords: chord_labels,
            progressions,
            complexity,
        })
    }

    /// Compute chromagram from signal.
    fn compute_chromagram(&self, signal: &[f32]) -> MirResult<Vec<[f32; 12]>> {
        let frames = stft(signal, self.window_size, self.hop_size)?;
        let mut chroma_frames = Vec::with_capacity(frames.len());

        for frame in &frames {
            let chroma = self.frame_to_chroma(frame);
            chroma_frames.push(chroma);
        }

        Ok(chroma_frames)
    }

    /// Convert FFT frame to chroma vector.
    #[allow(clippy::cast_precision_loss)]
    fn frame_to_chroma(&self, frame: &[rustfft::num_complex::Complex<f32>]) -> [f32; 12] {
        let mut chroma = [0.0; 12];
        let num_bins = frame.len() / 2;
        let ref_freq = 16.35; // C0

        for (bin, complex) in frame[1..num_bins].iter().enumerate() {
            let magnitude = complex.norm();
            let freq = (bin + 1) as f32 * self.sample_rate / self.window_size as f32;

            if freq < 20.0 {
                continue;
            }

            let pitch_class = self.freq_to_pitch_class(freq, ref_freq);
            chroma[pitch_class] += magnitude;
        }

        // Normalize
        let sum: f32 = chroma.iter().sum();
        if sum > 0.0 {
            for c in &mut chroma {
                *c /= sum;
            }
        }

        chroma
    }

    /// Convert frequency to pitch class.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn freq_to_pitch_class(&self, freq: f32, ref_freq: f32) -> usize {
        let semitones = 12.0 * (freq / ref_freq).log2();
        (semitones.round() as i32).rem_euclid(12) as usize
    }

    /// Match chroma to chord template.
    fn match_chord(&self, chroma: &[f32; 12]) -> (String, f32) {
        let mut best_match = ("N".to_string(), 0.0);

        for template in CHORD_TEMPLATES {
            let similarity = self.cosine_similarity(chroma, &template.template);
            if similarity > best_match.1 {
                best_match = (template.name.to_string(), similarity);
            }
        }

        best_match
    }

    /// Compute cosine similarity between two vectors.
    fn cosine_similarity(&self, a: &[f32; 12], b: &[f32; 12]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    /// Extract chord progressions.
    fn extract_progressions(&self, chords: &[ChordLabel]) -> Vec<String> {
        let mut progressions = Vec::new();

        for window in chords.windows(4) {
            let progression = window
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>()
                .join(" - ");
            progressions.push(progression);
        }

        progressions
    }

    /// Compute harmonic complexity.
    fn compute_complexity(&self, chords: &[ChordLabel]) -> f32 {
        if chords.len() < 2 {
            return 0.0;
        }

        let mut changes = 0;
        for i in 1..chords.len() {
            if chords[i].label != chords[i - 1].label {
                changes += 1;
            }
        }

        // Normalize by duration
        (changes as f32 / chords.len() as f32).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chord_recognizer_creation() {
        let recognizer = ChordRecognizer::new(44100.0, 2048, 512);
        assert_eq!(recognizer.sample_rate, 44100.0);
    }

    #[test]
    fn test_chord_templates_count() {
        assert!(CHORD_TEMPLATES.len() >= 24); // At least 12 major + 12 minor
    }

    #[test]
    fn test_cosine_similarity() {
        let recognizer = ChordRecognizer::new(44100.0, 2048, 512);
        let a = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let sim = recognizer.cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }
}
