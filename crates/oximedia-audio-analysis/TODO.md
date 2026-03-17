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
- [ ] Add confidence scoring to `voice/gender` detection (currently binary, should be probabilistic)
- [ ] Implement multi-speaker separation in `voice/speaker` (speaker diarization)
- [ ] Add vibrato rate and extent quantification in `pitch/vibrato` (currently detection only)
- [x] Implement formant bandwidth estimation alongside frequency in `formant/analyze`
- [x] Add automatic key detection in `music/harmony` (identify musical key from chroma features)
- [ ] Improve `forensics/edit` splice detection with phase discontinuity analysis
- [x] Add ENF (Electrical Network Frequency) analysis to `forensics` for recording authentication
- [x] Implement `noise/classify` with more categories: hum, hiss, rumble, click, broadband

## New Features
- [x] Add mel spectrogram computation module for machine learning feature extraction
- [ ] Implement constant-Q transform (CQT) for music-oriented frequency analysis
- [x] Add audio scene classification (indoor/outdoor, quiet/noisy, speech/music/mixed)
- [ ] Implement instrument onset detection per-instrument in `music/instrument`
- [ ] Add singing voice detection and singing quality assessment
- [ ] Implement audio segmentation: speech/music/silence automatic boundary detection
- [ ] Add sound event detection (applause, laughter, coughing, siren, etc.)
- [ ] Implement audio quality degradation detection (encoding artifacts, bandwidth limitation)
- [ ] Add cross-recording comparison for speaker verification across different sessions
- [ ] Implement vocal effort estimation (whisper, normal, shout) in `voice`

## Performance
- [x] Replace `ndarray` with pure-Rust matrix operations per COOLJAPAN Pure Rust Policy
- [x] Replace `rustfft` with OxiFFT per COOLJAPAN Policy
- [ ] Pre-allocate FFT scratch buffers in `SpectralAnalyzer` to avoid per-frame allocation
- [ ] Add overlap-save method for efficient long-duration spectral analysis
- [ ] Implement parallel analysis of independent modules in `AudioAnalyzer::analyze` with rayon
- [ ] Cache window function coefficients across `SpectralAnalyzer` instances (static lazy)
- [ ] Optimize `formant/analyze` LPC computation with Levinson-Durbin recursion in-place

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
