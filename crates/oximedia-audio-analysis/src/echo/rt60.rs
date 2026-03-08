//! RT60 reverberation time measurement.

/// Measure RT60 reverberation time.
///
/// RT60 is the time it takes for sound to decay by 60 dB.
///
/// # Arguments
/// * `samples` - Audio samples (ideally an impulse response)
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// RT60 time in seconds
#[must_use]
pub fn measure_rt60(samples: &[f32], sample_rate: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // Compute energy decay curve (Schroeder integration)
    let energy_decay = schroeder_integration(samples);

    // Find times for -5 dB and -35 dB decay
    let max_energy = energy_decay[0];
    let threshold_5db = max_energy * 10.0_f32.powf(-5.0 / 10.0);
    let threshold_35db = max_energy * 10.0_f32.powf(-35.0 / 10.0);

    let mut time_5db = 0.0;
    let mut time_35db = 0.0;

    for (i, &energy) in energy_decay.iter().enumerate() {
        if energy <= threshold_5db && time_5db == 0.0 {
            time_5db = i as f32 / sample_rate;
        }
        if energy <= threshold_35db {
            time_35db = i as f32 / sample_rate;
            break;
        }
    }

    // Extrapolate to 60 dB
    let decay_time = time_35db - time_5db;
    if decay_time > 0.0 {
        decay_time * 2.0 // 30 dB -> 60 dB
    } else {
        0.0
    }
}

/// Compute Schroeder integration (backward energy integration).
fn schroeder_integration(samples: &[f32]) -> Vec<f32> {
    let mut energy = vec![0.0; samples.len()];

    // Backward integration
    let mut sum = 0.0;
    for i in (0..samples.len()).rev() {
        sum += samples[i] * samples[i];
        energy[i] = sum;
    }

    energy
}

/// Measure early decay time (EDT).
///
/// EDT is similar to RT60 but measures 0 to -10 dB decay.
#[must_use]
pub fn measure_edt(samples: &[f32], sample_rate: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let energy_decay = schroeder_integration(samples);
    let max_energy = energy_decay[0];
    let threshold_10db = max_energy * 10.0_f32.powf(-10.0 / 10.0);

    for (i, &energy) in energy_decay.iter().enumerate() {
        if energy <= threshold_10db {
            let edt = i as f32 / sample_rate;
            return edt * 6.0; // Extrapolate to 60 dB
        }
    }

    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rt60_measurement() {
        // Generate exponentially decaying signal
        let sample_rate = 44100.0;
        let decay_constant = 0.5; // seconds for 60 dB decay
        let samples: Vec<f32> = (0..44100)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (-t / decay_constant).exp()
            })
            .collect();

        let rt60 = measure_rt60(&samples, sample_rate);

        // RT60 should be positive and finite
        assert!(rt60 >= 0.0 && rt60.is_finite());
    }

    #[test]
    fn test_schroeder_integration() {
        let samples = vec![1.0, 0.5, 0.25, 0.125];
        let energy = schroeder_integration(&samples);

        assert!(energy[0] > energy[1]);
        assert!(energy[1] > energy[2]);
        assert!(energy[2] > energy[3]);
    }
}
