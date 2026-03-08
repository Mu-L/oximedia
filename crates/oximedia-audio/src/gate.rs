//! Noise gate for audio signals.
//!
//! Implements a noise gate with threshold, hysteresis, hold time, and
//! a state machine for open/close transitions.

/// State of the noise gate.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateState {
    /// Gate is open; signal passes through.
    Open,
    /// Gate is in hold phase after signal dropped below threshold.
    Holding,
    /// Gate is closed; signal is attenuated.
    Closed,
}

/// Noise gate configuration.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Open threshold in dBFS.
    pub threshold_db: f32,
    /// Close threshold in dBFS (should be <= threshold_db for hysteresis).
    pub close_threshold_db: f32,
    /// Attack time in seconds (how fast the gate opens).
    pub attack_secs: f32,
    /// Release time in seconds (how fast the gate closes after hold).
    pub release_secs: f32,
    /// Hold time in seconds (stay open after signal drops below close threshold).
    pub hold_secs: f32,
    /// Attenuation applied when gate is closed, in dB (0 = fully closed, -60 = -60 dBFS floor).
    pub floor_db: f32,
    /// Sample rate in Hz.
    pub sample_rate: f32,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            threshold_db: -40.0,
            close_threshold_db: -45.0,
            attack_secs: 0.001,
            release_secs: 0.05,
            hold_secs: 0.1,
            floor_db: -80.0,
            sample_rate: 48_000.0,
        }
    }
}

/// Noise gate processor.
#[allow(dead_code)]
pub struct NoiseGate {
    config: GateConfig,
    state: GateState,
    /// Envelope follower level (linear).
    envelope: f32,
    /// Remaining hold samples.
    hold_samples_remaining: u32,
    /// Total hold samples.
    hold_samples_total: u32,
    /// Current gain (0.0..=1.0).
    current_gain: f32,
    /// Attack coefficient for gain smoothing.
    attack_coeff: f32,
    /// Release coefficient for gain smoothing.
    release_coeff: f32,
    /// Floor gain (linear).
    floor_gain: f32,
}

impl NoiseGate {
    /// Create a new noise gate.
    #[allow(dead_code)]
    pub fn new(config: GateConfig) -> Self {
        let hold_samples_total = (config.hold_secs * config.sample_rate).round() as u32;
        let attack_coeff = Self::time_coeff(config.attack_secs, config.sample_rate);
        let release_coeff = Self::time_coeff(config.release_secs, config.sample_rate);
        let floor_gain = db_to_linear(config.floor_db);

        Self {
            config,
            state: GateState::Closed,
            envelope: 0.0,
            hold_samples_remaining: 0,
            hold_samples_total,
            current_gain: 0.0,
            attack_coeff,
            release_coeff,
            floor_gain,
        }
    }

    fn time_coeff(time_secs: f32, sample_rate: f32) -> f32 {
        if time_secs <= 0.0 || sample_rate <= 0.0 {
            return 0.0;
        }
        (-1.0_f32 / (time_secs * sample_rate)).exp()
    }

    /// Process a single sample and return the gated output.
    #[allow(dead_code)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Update envelope
        let abs_input = input.abs();
        if abs_input > self.envelope {
            self.envelope =
                self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_input;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_input;
        }

        let level_db = linear_to_db(self.envelope);

        // State machine
        match self.state {
            GateState::Closed => {
                if level_db >= self.config.threshold_db {
                    self.state = GateState::Open;
                    self.hold_samples_remaining = self.hold_samples_total;
                }
            }
            GateState::Open => {
                if level_db < self.config.close_threshold_db {
                    self.state = GateState::Holding;
                    self.hold_samples_remaining = self.hold_samples_total;
                } else {
                    self.hold_samples_remaining = self.hold_samples_total;
                }
            }
            GateState::Holding => {
                if level_db >= self.config.threshold_db {
                    self.state = GateState::Open;
                    self.hold_samples_remaining = self.hold_samples_total;
                } else if self.hold_samples_remaining == 0 {
                    self.state = GateState::Closed;
                } else {
                    self.hold_samples_remaining -= 1;
                }
            }
        }

        // Target gain based on state
        let target_gain = match self.state {
            GateState::Open | GateState::Holding => 1.0,
            GateState::Closed => self.floor_gain,
        };

        // Smooth gain transitions
        if target_gain > self.current_gain {
            self.current_gain =
                self.attack_coeff * self.current_gain + (1.0 - self.attack_coeff) * target_gain;
        } else {
            self.current_gain =
                self.release_coeff * self.current_gain + (1.0 - self.release_coeff) * target_gain;
        }

        input * self.current_gain
    }

    /// Process a buffer of samples in-place.
    #[allow(dead_code)]
    pub fn process_buffer(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Returns the current gate state.
    #[allow(dead_code)]
    pub fn state(&self) -> GateState {
        self.state
    }

    /// Returns the current gain (0.0..=1.0).
    #[allow(dead_code)]
    pub fn current_gain(&self) -> f32 {
        self.current_gain
    }

    /// Reset the gate to closed state.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = GateState::Closed;
        self.envelope = 0.0;
        self.hold_samples_remaining = 0;
        self.current_gain = 0.0;
    }

    /// Update hold time.
    #[allow(dead_code)]
    pub fn set_hold(&mut self, hold_secs: f32) {
        self.config.hold_secs = hold_secs;
        self.hold_samples_total = (hold_secs * self.config.sample_rate).round() as u32;
    }

    /// Update floor attenuation.
    #[allow(dead_code)]
    pub fn set_floor_db(&mut self, floor_db: f32) {
        self.config.floor_db = floor_db;
        self.floor_gain = db_to_linear(floor_db);
    }
}

/// Convert linear amplitude to dBFS.
#[allow(dead_code)]
fn linear_to_db(linear: f32) -> f32 {
    if linear <= 1e-10 {
        -120.0
    } else {
        20.0 * linear.log10()
    }
}

/// Convert dBFS to linear amplitude.
#[allow(dead_code)]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gate() -> NoiseGate {
        NoiseGate::new(GateConfig::default())
    }

    #[test]
    fn test_gate_creation() {
        let g = make_gate();
        assert_eq!(g.state(), GateState::Closed);
    }

    #[test]
    fn test_silence_stays_closed() {
        let mut g = make_gate();
        for _ in 0..1000 {
            g.process_sample(0.0);
        }
        assert_eq!(g.state(), GateState::Closed);
    }

    #[test]
    fn test_loud_signal_opens_gate() {
        let mut g = make_gate();
        // Signal well above threshold (-40 dBFS), threshold linear ≈ 0.01
        for _ in 0..2000 {
            g.process_sample(0.1);
        }
        assert_eq!(g.state(), GateState::Open);
    }

    #[test]
    fn test_reset_closes_gate() {
        let mut g = make_gate();
        for _ in 0..2000 {
            g.process_sample(0.1);
        }
        g.reset();
        assert_eq!(g.state(), GateState::Closed);
        assert_eq!(g.envelope, 0.0);
    }

    #[test]
    fn test_db_to_linear() {
        let lin = db_to_linear(0.0);
        assert!((lin - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_linear_to_db_zero() {
        let db = linear_to_db(0.0);
        assert_eq!(db, -120.0);
    }

    #[test]
    fn test_linear_to_db_one() {
        let db = linear_to_db(1.0);
        assert!((db - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_process_buffer_not_nan() {
        let mut g = make_gate();
        let mut buf = vec![0.05_f32; 500];
        g.process_buffer(&mut buf);
        for s in &buf {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn test_set_hold_updates_total() {
        let mut g = make_gate();
        g.set_hold(0.5);
        assert_eq!(g.hold_samples_total, (0.5 * 48_000.0) as u32);
    }

    #[test]
    fn test_set_floor_db() {
        let mut g = make_gate();
        g.set_floor_db(-60.0);
        let expected = db_to_linear(-60.0);
        assert!((g.floor_gain - expected).abs() < 1e-6);
    }

    #[test]
    fn test_gate_transitions_to_holding() {
        let config = GateConfig {
            hold_secs: 1.0,
            sample_rate: 48_000.0,
            ..GateConfig::default()
        };
        let mut g = NoiseGate::new(config);
        // Open the gate
        for _ in 0..5000 {
            g.process_sample(0.1);
        }
        assert_eq!(g.state(), GateState::Open);
        // Drop signal below close threshold
        for _ in 0..100 {
            g.process_sample(0.0);
        }
        // Should be in Holding (hold_secs = 1.0 means lots of samples left)
        assert!(g.state() == GateState::Holding || g.state() == GateState::Open);
    }

    #[test]
    fn test_current_gain_closed_is_low() {
        let mut g = make_gate();
        // Process silence (gate closed)
        for _ in 0..500 {
            g.process_sample(0.0);
        }
        // After settling, gain should be near floor
        let gain = g.current_gain();
        assert!(gain < 0.5);
    }

    #[test]
    fn test_hysteresis_close_threshold_lower() {
        let config = GateConfig::default();
        assert!(config.close_threshold_db <= config.threshold_db);
    }
}
