//! Effects chain management for the audio mixer.

// ---------------------------------------------------------------------------
// AudioEffect trait
// ---------------------------------------------------------------------------

/// A single audio effect that can process a mono sample buffer in-place.
pub trait AudioEffect: Send + Sync {
    /// Process the sample buffer in-place.
    fn process(&mut self, samples: &mut [f32]);

    /// Human-readable effect name.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Delay
// ---------------------------------------------------------------------------

/// Tape-style delay / echo effect.
#[derive(Debug, Clone)]
pub struct DelayEffect {
    /// Delay length in samples.
    pub delay_samples: usize,
    /// Feedback factor (0.0 = no feedback, <1.0 to avoid runaway).
    pub feedback: f32,
    /// Wet/dry mix (0.0 = dry, 1.0 = full wet).
    pub mix: f32,
    /// Internal circular buffer.
    buffer: Vec<f32>,
    /// Current write position in the buffer.
    write_pos: usize,
}

impl DelayEffect {
    /// Create a new delay effect.
    ///
    /// The internal buffer is allocated to `delay_samples` (minimum 1).
    #[must_use]
    pub fn new(delay_samples: usize, feedback: f32, mix: f32) -> Self {
        let len = delay_samples.max(1);
        Self {
            delay_samples: len,
            feedback: feedback.clamp(0.0, 0.999),
            mix: mix.clamp(0.0, 1.0),
            buffer: vec![0.0_f32; len],
            write_pos: 0,
        }
    }
}

impl AudioEffect for DelayEffect {
    fn process(&mut self, samples: &mut [f32]) {
        let buf_len = self.buffer.len();
        for sample in samples.iter_mut() {
            // Read delayed sample
            let read_pos =
                (self.write_pos + buf_len - self.delay_samples.min(buf_len - 1)) % buf_len;
            let delayed = self.buffer[read_pos];

            // Write current + feedback into buffer
            self.buffer[self.write_pos] = *sample + delayed * self.feedback;
            self.write_pos = (self.write_pos + 1) % buf_len;

            // Mix dry and wet
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
    }

    fn name(&self) -> &'static str {
        "Delay"
    }
}

// ---------------------------------------------------------------------------
// Chorus
// ---------------------------------------------------------------------------

/// Simple LFO-modulated chorus effect (stub).
#[derive(Debug, Clone)]
pub struct ChorusEffect {
    /// LFO rate in Hz.
    pub rate_hz: f32,
    /// Modulation depth in samples.
    pub depth_samples: f32,
    /// Wet/dry mix.
    pub mix: f32,
    /// Current LFO phase (radians).
    lfo_phase: f32,
}

impl ChorusEffect {
    /// Create a new chorus effect.
    #[must_use]
    pub fn new(rate_hz: f32, depth_samples: f32, mix: f32) -> Self {
        Self {
            rate_hz: rate_hz.max(0.0),
            depth_samples: depth_samples.max(0.0),
            mix: mix.clamp(0.0, 1.0),
            lfo_phase: 0.0,
        }
    }
}

impl AudioEffect for ChorusEffect {
    fn process(&mut self, samples: &mut [f32]) {
        use std::f32::consts::TAU;
        // Stub: apply subtle sine-wave amplitude modulation as placeholder.
        for sample in samples.iter_mut() {
            let lfo = self.lfo_phase.sin() * self.depth_samples * 0.01;
            *sample = *sample * (1.0 - self.mix) + *sample * (1.0 + lfo) * self.mix;
            self.lfo_phase = (self.lfo_phase + TAU * self.rate_hz / 48_000.0) % TAU;
        }
    }

    fn name(&self) -> &'static str {
        "Chorus"
    }
}

// ---------------------------------------------------------------------------
// Reverb
// ---------------------------------------------------------------------------

/// Simple Schroeder-inspired reverb stub using parallel comb filters.
#[derive(Debug, Clone)]
pub struct ReverbEffect {
    /// Room size factor (0.0–1.0 controls comb filter delays).
    pub room_size: f32,
    /// High-frequency damping (0.0 = bright, 1.0 = dark).
    pub damping: f32,
    /// Wet/dry mix.
    pub mix: f32,
    /// Comb filter buffers (one per filter).
    comb_buffers: Vec<Vec<f32>>,
    /// Write positions for each comb filter.
    comb_positions: Vec<usize>,
    /// Low-pass filter states for damping.
    lp_states: Vec<f32>,
}

impl ReverbEffect {
    /// Comb filter delay lengths (in samples at 44100 Hz; scaled by `room_size`).
    const BASE_DELAYS: [usize; 4] = [1557, 1617, 1491, 1422];

    /// Create a new reverb effect.
    #[must_use]
    pub fn new(room_size: f32, damping: f32, mix: f32) -> Self {
        let rs = room_size.clamp(0.0, 1.0);
        let comb_buffers: Vec<Vec<f32>> = Self::BASE_DELAYS
            .iter()
            .map(|&d| {
                let len = ((d as f32 * (0.5 + rs * 0.5)) as usize).max(1);
                vec![0.0_f32; len]
            })
            .collect();
        let n = comb_buffers.len();
        Self {
            room_size: rs,
            damping: damping.clamp(0.0, 1.0),
            mix: mix.clamp(0.0, 1.0),
            comb_buffers,
            comb_positions: vec![0; n],
            lp_states: vec![0.0_f32; n],
        }
    }
}

impl AudioEffect for ReverbEffect {
    fn process(&mut self, samples: &mut [f32]) {
        let feedback = 0.84_f32 * self.room_size.max(0.1);
        let damp = self.damping;

        for sample in samples.iter_mut() {
            let mut wet = 0.0_f32;

            for i in 0..self.comb_buffers.len() {
                let len = self.comb_buffers[i].len();
                let pos = self.comb_positions[i];
                let delayed = self.comb_buffers[i][pos];

                // Low-pass damping
                self.lp_states[i] = delayed * (1.0 - damp) + self.lp_states[i] * damp;
                self.comb_buffers[i][pos] = *sample + self.lp_states[i] * feedback;
                self.comb_positions[i] = (pos + 1) % len;

                wet += delayed;
            }

            #[allow(clippy::cast_precision_loss)]
            let scale = 1.0 / self.comb_buffers.len() as f32;
            *sample = *sample * (1.0 - self.mix) + wet * scale * self.mix;
        }
    }

    fn name(&self) -> &'static str {
        "Reverb"
    }
}

// ---------------------------------------------------------------------------
// Effects Chain
// ---------------------------------------------------------------------------

/// An ordered chain of audio effects applied in sequence.
pub struct EffectsChain {
    effects: Vec<Box<dyn AudioEffect>>,
}

impl EffectsChain {
    /// Create an empty effects chain.
    #[must_use]
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Append an effect to the end of the chain.
    pub fn add(&mut self, effect: Box<dyn AudioEffect>) {
        self.effects.push(effect);
    }

    /// Remove the effect at `idx`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.len()`.
    pub fn remove(&mut self, idx: usize) {
        self.effects.remove(idx);
    }

    /// Process a sample buffer through every effect in order.
    pub fn process_block(&mut self, samples: &mut [f32]) {
        for effect in &mut self.effects {
            effect.process(samples);
        }
    }

    /// Number of effects in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Returns `true` if the chain contains no effects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

impl Default for EffectsChain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_effect_name() {
        let d = DelayEffect::new(100, 0.5, 0.5);
        assert_eq!(d.name(), "Delay");
    }

    #[test]
    fn test_delay_dry_signal_when_mix_zero() {
        let mut d = DelayEffect::new(10, 0.0, 0.0);
        let mut samples = [0.5_f32, 0.3, 0.1];
        let original = samples;
        d.process(&mut samples);
        for (s, o) in samples.iter().zip(original.iter()) {
            assert!((s - o).abs() < 1e-6, "s={s} o={o}");
        }
    }

    #[test]
    fn test_delay_buffer_length_minimum_one() {
        let d = DelayEffect::new(0, 0.5, 0.5);
        assert_eq!(d.buffer.len(), 1);
    }

    #[test]
    fn test_delay_feedback_clamp() {
        let d = DelayEffect::new(10, 1.5, 0.5);
        assert!(d.feedback < 1.0);
    }

    #[test]
    fn test_chorus_effect_name() {
        let c = ChorusEffect::new(1.5, 5.0, 0.3);
        assert_eq!(c.name(), "Chorus");
    }

    #[test]
    fn test_chorus_does_not_blow_up() {
        let mut c = ChorusEffect::new(1.5, 5.0, 0.5);
        let mut samples = vec![0.1_f32; 512];
        c.process(&mut samples);
        for s in &samples {
            assert!(s.is_finite(), "non-finite sample after chorus");
        }
    }

    #[test]
    fn test_reverb_effect_name() {
        let r = ReverbEffect::new(0.5, 0.5, 0.3);
        assert_eq!(r.name(), "Reverb");
    }

    #[test]
    fn test_reverb_does_not_blow_up() {
        let mut r = ReverbEffect::new(0.7, 0.5, 0.4);
        let mut samples = vec![0.1_f32; 1024];
        r.process(&mut samples);
        for s in &samples {
            assert!(s.is_finite(), "non-finite sample after reverb");
        }
    }

    #[test]
    fn test_effects_chain_starts_empty() {
        let chain = EffectsChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn test_effects_chain_add() {
        let mut chain = EffectsChain::new();
        chain.add(Box::new(DelayEffect::new(100, 0.3, 0.5)));
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_effects_chain_remove() {
        let mut chain = EffectsChain::new();
        chain.add(Box::new(DelayEffect::new(100, 0.3, 0.5)));
        chain.add(Box::new(ChorusEffect::new(1.0, 3.0, 0.3)));
        chain.remove(0);
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_effects_chain_process_block_passthrough() {
        // An empty chain should leave samples unchanged.
        let mut chain = EffectsChain::new();
        let mut samples = [0.1_f32, 0.2, 0.3, 0.4];
        let original = samples;
        chain.process_block(&mut samples);
        for (s, o) in samples.iter().zip(original.iter()) {
            assert!((s - o).abs() < 1e-6);
        }
    }

    #[test]
    fn test_effects_chain_default_is_empty() {
        let chain: EffectsChain = Default::default();
        assert!(chain.is_empty());
    }
}
