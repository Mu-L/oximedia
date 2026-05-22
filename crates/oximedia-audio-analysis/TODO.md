# oximedia-audio-analysis TODO

## Current Status
- 80+ source files across modules: `spectral` (centroid, flatness, crest, bandwidth, flux, rolloff, chroma, zcr, fft_frame, analyze), `voice` (gender, age, emotion, speaker, characteristics), `pitch` (track, contour, vibrato, detect), `formant` (analyze, track, vowel), `dynamics` (range, crest, rms), `transient` (detect, envelope), `forensics` (authenticity, edit, compression, noise), `music` (harmony, rhythm, timbre, instrument), `separate` (vocal, drums, instrumental, sources), `echo` (detect, room, rt60), `distortion` (detect, clipping, thd), `noise` (classify, profile, snr), plus `beat`, `cepstral`, `chroma`, `energy`, `energy_contour`, `harmony`, `loudness`, `loudness_curve`, `loudness_range`, `onset`, `pitch_detect`, `pitch_tracker`, `psychoacoustic`, `rhythm`, `silence_detect`, `spectral_contrast`, `spectral_features`, `spectral_flux`, `stereo_field`, `tempo_analysis`, `timbre`, `formant_track`
- Main `AudioAnalyzer` coordinating spectral, voice, pitch, formant, dynamics, transient analysis
- Frame-by-frame real-time analysis support via `analyze_frame`
- Patent-free algorithms: YIN pitch detection, LPC formant analysis, harmonic-percussive separation
- Dependencies: oximedia-core, oximedia-audio, oximedia-mir, rustfft, ndarray, rayon, serde

## Enhancements
- [x] Add MFCC (Mel-Frequency Cepstral Coefficients) computation in `cepstral` for audio feature extraction
- [x] Implement chromagram normalization options in `chroma` (L1, L2, max norm)
- [x] Add confidence scoring to `voice/gender` detection (currently binary, should be probabilistic) (verified 2026-05-16; src/voice/gender.rs:73 detect_gender_with_confidence, confidence score:79)
- [x] Implement multi-speaker separation in `voice/speaker` (speaker diarization) (verified 2026-05-16; src/voice/diarization.rs:545 lines SpeakerDiarization)
- [x] Add vibrato rate and extent quantification in `pitch/vibrato` (currently detection only) (verified 2026-05-16; src/pitch/vibrato.rs:7 VibratoResult.rate in Hz:11 + extent in cents:13)
- [x] Implement formant bandwidth estimation alongside frequency in `formant/analyze`
- [x] Add automatic key detection in `music/harmony` (identify musical key from chroma features)
- [x] Improve `forensics/edit` splice detection with phase discontinuity analysis (implemented 2026-05-15: PhaseDiscontinuityDetector uses second-derivative d²φ/dt² of instantaneous phase; SpliceProbability{frame_idx,confidence}; wired into EditDetector pipeline; tests: test_phase_continuity_clean_signal, test_phase_discontinuity_detected_at_splice)
- [x] Add ENF (Electrical Network Frequency) analysis to `forensics` for recording authentication
- [x] Implement `noise/classify` with more categories: hum, hiss, rumble, click, broadband

## New Features
- [x] Add mel spectrogram computation module for machine learning feature extraction
- [x] Implement constant-Q transform (CQT) for music-oriented frequency analysis (verified 2026-05-16; src/cqt.rs:313 lines CqtAnalyzer)
- [x] Add audio scene classification (indoor/outdoor, quiet/noisy, speech/music/mixed)
- [x] Implement instrument onset detection per-instrument in `music/instrument` (implemented 2026-05-15: InstrumentBand enum (Kick/Bass/MidRange/Treble/HiHat), InstrumentOnsetDetector with detect_onsets_per_instrument→HashMap<InstrumentBand,Vec<f64>>; band-limited HWR spectral flux with adaptive threshold; tests: test_kick_onset_in_low_band, test_instrument_onset_bands_independent)
- [x] Add singing voice detection and singing quality assessment (verified 2026-05-16; src/singing.rs:363 lines SingingDetector)
- [x] Implement audio segmentation: speech/music/silence automatic boundary detection (verified 2026-05-16; src/segmentation.rs:434 lines AudioSegmenter)
- [x] Add sound event detection (applause, laughter, coughing, siren, etc.) (verified 2026-05-16; src/event_detection.rs:526 lines SoundEvent enum Applause/Laughter/Siren:21)
- [x] Implement audio quality degradation detection (encoding artifacts, bandwidth limitation) (verified 2026-05-16; src/quality_degradation.rs:378 lines QualityDegradationDetector)
- [x] Add cross-recording comparison for speaker verification across different sessions (implemented 2026-05-15: src/voice/cross_verification.rs — CrossRecordingVerifier, SpeakerVerificationResult{cosine_similarity,euclidean_distance,is_same_speaker}, CrossSessionResult with centroid + min/max/mean pairwise matrix; tests: test_same_speaker_high_similarity, test_different_speaker_low_similarity, test_cross_session_centroid)
- [x] Implement vocal effort estimation (whisper, normal, shout) in `voice` (verified 2026-05-16; src/voice/vocal_effort.rs:351 lines VocalEffortEstimator)

## Performance
- [x] Replace `ndarray` with pure-Rust matrix operations per COOLJAPAN Pure Rust Policy
- [x] Replace `rustfft` with OxiFFT per COOLJAPAN Policy
- [ ] Pre-allocate FFT scratch buffers in `SpectralAnalyzer` to avoid per-frame allocation (verified-open 2026-05-16: not yet implemented)
- [ ] Add overlap-save method for efficient long-duration spectral analysis (verified-open 2026-05-16: not yet implemented)
- [ ] Implement parallel analysis of independent modules in `AudioAnalyzer::analyze` with rayon (verified-open 2026-05-16: no rayon par_iter in lib.rs analyze())
- [ ] Cache window function coefficients across `SpectralAnalyzer` instances (static lazy) (verified-open 2026-05-16: not yet implemented)
- [ ] Optimize `formant/analyze` LPC computation with Levinson-Durbin recursion in-place (verified-open 2026-05-16: not yet implemented)

## Testing
- [ ] Test `pitch/track` YIN accuracy against PTDB-TUG pitch reference dataset values
- [ ] Add `voice/emotion` detection test with synthetic signals (known pitch/energy patterns)
- [ ] Test `forensics/authenticity` with intentionally spliced audio files
- [ ] Test `distortion/thd` computation against reference sinusoidal test signals
- [ ] Add `echo/rt60` measurement test with synthetic exponentially decaying impulse response
- [ ] Test `separate/vocal` separation quality using synthetic mixed vocal+instrumental signals
- [ ] Test `noise/snr` computation accuracy with known white noise at specific SNR levels
- [ ] Add `beat` detection accuracy test against annotated rhythm datasets

## Documentation
- [ ] Document each analysis module's algorithm, computational complexity, and accuracy characteristics
- [ ] Add signal flow diagram for the `AudioAnalyzer` processing pipeline
- [ ] Document recommended `AnalysisConfig` presets for common use cases (speech analysis, music analysis, forensics)
