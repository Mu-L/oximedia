# oximedia-archive-pro TODO

## Current Status
- 66 source files across modules: `package` (bagit, oais, tar, zip), `checksum` (generate, verify, tree), `metadata` (premis, mets, embed, extract), `fixity` (schedule, verify, report), `format_migration`, `format_registry`, `format_validator`, `version` (history, control, diff), `risk` (assess, monitor, alert), `policy` (define, enforce), `migrate` (planner, execute, validate), `docs` (generate, descriptive, technical), `emulation` (prepare, package, planning), `audit_trail`, `ingest`, `retention`, `oais_model`, `migration_plan`, `cold_storage`, `deep_archive`, `disaster_recovery`, `deaccession`, `provenance_chain`, `replication_verify`, `restore_workflow`, `storage_quota`, `workflow_state`, `access_policy`, `archive_report`, `archive_stats`, `bit_rot_detection`, `integrity_check`, `metadata_crosswalk`
- BagIt and OAIS (SIP/AIP/DIP) packaging, multi-algorithm checksums, PREMIS/METS metadata
- Format migration planning and execution, risk assessment, emulation support
- Dependencies: oximedia-core, oximedia-archive, quick-xml, tar, oxiarc-archive, sha2, blake3, walkdir, chrono

## Enhancements
- [x] Add BagIt v1.0 spec compliance verification in `package/bagit` (fetch.txt, tag manifests)
- [x] Implement OAIS DIP generation from AIP in `package/oais` (dissemination information package)
- [x] Add incremental BagIt update in `package/bagit` (add/remove files without full re-manifest) (verified 2026-05-16; src/package/bagit.rs:884 test_incremental_add_file, test_incremental_remove_file:919, add_file:685)
- [x] Implement Merkle tree verification in `checksum/tree` for efficient partial verification
- [x] Add PREMIS rights metadata support in `metadata/premis` (access restrictions, license info)
- [x] Implement METS structural map generation in `metadata/mets` for complex multi-file objects (verified 2026-05-16; src/metadata/mets.rs:421 build_auto_structural_map, MetsDiv, test_mets_structural_map_generation:528)
- [ ] Add `format_validator` support for TIFF, DPX, and OpenEXR validation rules (partial 2026-05-16: format_validator.rs:52 FormatFamily::Tiff with magic bytes; DPX and OpenEXR variants missing)
- [x] Implement `cold_storage` tier management with automated warm-up and cool-down transitions
- [x] Add `disaster_recovery` plan validation: verify backup completeness and recoverability
- [x] Implement `provenance_chain` cryptographic signing for tamper-evident audit trails

## New Features
- [x] Add Dublin Core metadata support in `metadata` alongside PREMIS/METS
- [x] Implement IIIF (International Image Interoperability Framework) manifest generation (verified 2026-05-16; src/iiif_manifest.rs:121 IiifManifest, IiifManifestBuilder:190, Presentation API 3.0:3, 512 lines)
- [ ] Add DataCite DOI metadata generation for archived research media (verified-open 2026-05-16: no DataCite/datacite module in archive-pro sources; dublin_core.rs mentions doi as identifier field only)
- [ ] Implement automated format migration triggers based on `risk/monitor` alerts (verified-open 2026-05-16: risk/monitor.rs has RiskMonitor/MonitoringReport but no auto_trigger/MigrationTrigger wired to format_migration)
- [x] Add geographic replication support with consistency verification in `replication_verify` (verified 2026-05-16; src/replication_verify.rs:12 ReplicaLocation, FileReplicaInfo.evaluate:98, ReplicationVerifier:182, consistency_percent:172, 451 lines)
- [x] Implement `deaccession` workflow with approval chain and audit logging (verified 2026-05-16; src/deaccession.rs:106 DeaccessionRequest, DeaccessionStatus Approved/UnderReview/Executing:60, approve/reject:187, 488 lines)
- [x] Add preservation cost estimation in `archive_stats` (storage growth, migration cost projections) (verified 2026-05-16; src/cost_estimator.rs:322 PreservationCostEstimator, CostEstimatorConfig:287, default_25_year:516, 609 lines)
- [x] Implement LOCKSS/CLOCKSS-compatible package generation for distributed preservation (verified 2026-05-16; src/lockss_package.rs:1 LockssPackage, LockssNetwork::Lockss/Clockss:22-25, ArchivalUnit:39, 514 lines)
- [ ] Add `metadata_crosswalk` support for PBCore (public broadcasting metadata standard) (verified-open 2026-05-16: metadata_crosswalk.rs has MetadataCrosswalk/crosswalk rules framework but no PBCore-specific schema or mappings)

## Performance
- [x] Parallelize BagIt manifest generation with rayon in `package/bagit` for large bags
- [ ] Implement streaming TAR creation in `package/tar` (avoid buffering entire archive in memory)
- [ ] Add concurrent checksum verification across multiple algorithms in `checksum/verify`
- [ ] Optimize `walkdir` traversal with early filtering by file extension in `ingest`
- [ ] Use zero-copy XML writing in `metadata/premis` and `metadata/mets` via quick-xml streaming

## Testing
- [ ] Add BagIt validation test against Library of Congress BagIt conformance suite
- [x] Test OAIS SIP -> AIP -> DIP lifecycle with round-trip metadata preservation
- [ ] Test `format_migration` planner with simulated format registry obsolescence data
- [ ] Add `fixity/schedule` test with synthetic file modifications between check intervals
- [ ] Test `risk/assess` scoring against known high-risk format scenarios (e.g., Flash, RealMedia)
- [ ] Test `checksum/tree` Merkle verification with single-file corruption injection
- [ ] Add integration test: full ingest -> package -> verify -> migrate -> re-verify workflow

## Documentation
- [ ] Add OAIS reference model diagram with module-to-function mapping
- [ ] Document supported metadata schemas and crosswalk mappings
- [ ] Add preservation planning decision flowchart (when to migrate vs emulate)
