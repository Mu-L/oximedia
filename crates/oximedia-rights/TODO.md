# oximedia-rights TODO

## Current Status
- 40 modules (13 subdirectory modules) covering rights tracking, license management, expiration handling, territory restrictions, usage tracking, clearance workflows, royalty calculation, watermarking, DRM metadata, audit trails, compliance reporting
- Core types: RightsManager (with SQLx database backend), RightsError enum with 13 variants
- Conditional compilation: `database` module and `RightsManager` excluded on wasm32
- Dependencies: oximedia-core, oximedia-watermark, sqlx, chrono, uuid, serde

## Enhancements
- [ ] Add in-memory `RightsManager` alternative for wasm32 targets using HashMap-based storage
- [x] Extend `territory` with hierarchical region support (continent -> country -> state/province)
- [x] Add `rights_conflict` automatic resolution suggestions when overlapping rights are detected
- [x] Implement `royalty_engine` tiered royalty rates based on usage volume thresholds
- [x] Extend `embargo_policy` with platform-specific embargo windows (theatrical -> streaming -> broadcast)
- [ ] Add `clearance_workflow` status notifications with configurable escalation on pending clearances
- [ ] Implement `rights_timeline` visualization data export for Gantt-chart style rights period display
- [ ] Add `license_template` variable interpolation for generating customized license agreements

## New Features
- [ ] Add a `rights_api` module with REST endpoints for rights queries (is-asset-available, check-territory, get-license)
- [ ] Implement `content_id` module for content identification (ISRC, ISAN, EIDR) linking to rights records
- [ ] Add `rights_import` module for bulk importing rights data from CSV/JSON/XML
- [ ] Implement `rights_export` for generating machine-readable rights manifests (EIDR, DDEX, CWR formats)
- [x] Add `sub_licensing` module for managing sub-license chains with parent-child relationship tracking
- [ ] Implement `rights_calendar` for calendar-based views of expiring and upcoming rights windows
- [ ] Add `compliance_report` generator for regulatory requirements (GDPR, CCPA data rights)
- [ ] Implement `automated_takedown` module for triggering actions when rights expire or are revoked
- [ ] Add `rights_search` full-text search across rights holders, territories, and license terms

## Performance
- [ ] Add database connection pooling in `RightsManager` (currently creates single connection)
- [ ] Implement query caching in `rights_check` for frequently accessed assets with short TTL
- [ ] Add batch rights lookup in `rights_database` to reduce per-asset query overhead
- [ ] Index `territory` and `expiration` columns in the SQLite schema for faster filtered queries

## Testing
- [ ] Add tests for `royalty_calc` with complex tiered rate structures and multi-territory splits
- [ ] Test `embargo_window` edge cases (overlapping windows, midnight boundary, timezone handling)
- [ ] Add tests for `rights_conflict` detection with multiple overlapping territorial grants
- [ ] Test `clearance_workflow` state machine transitions (requested -> approved/denied -> expired)
- [ ] Add integration tests for `watermark` integration with actual image/video data

## Documentation
- [ ] Document the rights data model: RightsHolder -> License -> Territory -> Usage relationship
- [ ] Add decision flowchart for rights checking (territory check -> date check -> usage check -> clearance check)
- [ ] Document royalty calculation formulas and configuration options in `royalty_engine`
- [ ] Add guide for setting up DRM metadata integration with common DRM providers
