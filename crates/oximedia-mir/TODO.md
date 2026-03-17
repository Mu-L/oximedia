# oximedia-mir TODO

## Current Status
- 52 source files/directories covering tempo, beat, key, chord, melody, structure, genre, mood, spectral, rhythm, harmonic analysis
- Additional modules: fingerprint, playlist, cover_detect, chorus_detect, vocal_detect, instrument_detection, source_separation, fade_detect, tuning_detect
- Feature-gated per analysis type (tempo, beat, key, chord, melody, structure, genre, mood, spectral, rhythm, harmonic)
- MirAnalyzer provides unified analysis pipeline returning AnalysisResult with all enabled features

## Enhancements
- [x] Improve `to_mono()` in MirAnalyzer to actually detect and convert stereo -- currently just clones input
- [x] Add confidence thresholds to MirConfig so low-confidence results can be automatically filtered
- [x] Implement streaming/incremental analysis in MirAnalyzer for real-time use (process chunk-by-chunk)
- [ ] Add tempo stability metric to TempoResult (distinguish steady BPM from rubato/accelerando)
- [x] Improve chord_recognition to handle 7th, diminished, augmented, and suspended chords beyond major/minor
- [x] Add time-varying key detection for songs with key changes (modulations)
- [ ] Extend genre_classify with sub-genre classification (e.g., not just "rock" but "progressive rock")
- [ ] Add multi-track analysis support -- analyze stems separately then combine MIR results

## New Features
- [x] Implement audio-to-MIDI conversion using pitch_track and onset_strength data
- [x] Add rhythm complexity metric (syncopation, polyrhythm detection) to rhythm module
- [ ] Implement audio thumbnailing -- extract the most representative 15-30s clip using structure analysis
- [ ] Add music similarity search using fingerprint module with locality-sensitive hashing
- [ ] Implement real-time DJ features: beat-matching suggestions, compatible key detection (Camelot wheel)
- [ ] Add lyrics timing alignment support (given lyrics text, align to audio using vocal_detect + onset_strength)
- [ ] Implement audio watermark detection using spectral analysis

## Performance
- [x] Replace rustfft with OxiFFT (COOLJAPAN policy) in spectral analysis
- [x] Remove ndarray dependency -- use Vec<f32> with manual stride operations
- [x] Parallelize independent analysis branches in MirAnalyzer::analyze() using rayon (tempo/key/spectral are independent)
- [ ] Add early-exit in TempoDetector when confidence exceeds threshold to avoid full autocorrelation scan
- [ ] Cache chromagram computation so chord_recognition and key_detection share the same chroma features

## Testing
- [ ] Add test with known-BPM audio (e.g., synthesized 120 BPM click track) verifying TempoDetector accuracy
- [ ] Test key detection against known musical keys (C major scale, A minor arpeggio)
- [ ] Add regression test for chord recognition with isolated major/minor triads
- [ ] Test structure analysis with synthetic audio that has clear verse/chorus boundaries (volume/timbre changes)
- [ ] Validate MelodyExtractor output for monophonic sine sweep input

## Documentation
- [ ] Document the feature flags and which modules each feature gate controls
- [ ] Add usage examples for fingerprint-based music identification workflow
- [ ] Document the chromagram computation pipeline shared between key and chord analysis
