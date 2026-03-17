# oximedia-archive-pro TODO

## Current Status
- 66 source files across modules: `package` (bagit, oais, tar, zip), `checksum` (generate, verify, tree), `metadata` (premis, mets, embed, extract), `fixity` (schedule, verify, report), `format_migration`, `format_registry`, `format_validator`, `version` (history, control, diff), `risk` (assess, monitor, alert), `policy` (define, enforce), `migrate` (planner, execute, validate), `docs` (generate, descriptive, technical), `emulation` (prepare, package, planning), `audit_trail`, `ingest`, `retention`, `oais_model`, `migration_plan`, `cold_storage`, `deep_archive`, `disaster_recovery`, `deaccession`, `provenance_chain`, `replication_verify`, `restore_workflow`, `storage_quota`, `workflow_state`, `access_policy`, `archive_report`, `archive_stats`, `bit_rot_detection`, `integrity_check`, `metadata_crosswalk`
- BagIt and OAIS (SIP/AIP/DIP) packaging, multi-algorithm checksums, PREMIS/METS metadata
- Format migration planning and execution, risk assessment, emulation support
- Dependencies: oximedia-core, oximedia-archive, quick-xml, tar, oxiarc-archive, sha2, blake3, walkdir, chrono

## Enhancements
- [x] Add BagIt v1.0 spec compliance verification in `package/bagit` (fetch.txt, tag manifests)
- [x] Implement OAIS DIP generation from AIP in `package/oais` (dissemination information package)
- [ ] Add incremental BagIt update in `package/bagit` (add/remove files without full re-manifest)
- [x] Implement Merkle tree verification in `checksum/tree` for efficient partial verification
- [x] Add PREMIS rights metadata support in `metadata/premis` (access restrictions, license info)
- [ ] Implement METS structural map generation in `metadata/mets` for complex multi-file objects
- [ ] Add `format_validator` support for TIFF, DPX, and OpenEXR validation rules
- [x] Implement `cold_storage` tier management with automated warm-up and cool-down transitions
- [x] Add `disaster_recovery` plan validation: verify backup completeness and recoverability
- [x] Implement `provenance_chain` cryptographic signing for tamper-evident audit trails

## New Features
- [x] Add Dublin Core metadata support in `metadata` alongside PREMIS/METS
- [ ] Implement IIIF (International Image Interoperability Framework) manifest generation
- [ ] Add DataCite DOI metadata generation for archived research media
- [ ] Implement automated format migration triggers based on `risk/monitor` alerts
- [ ] Add geographic replication support with consistency verification in `replication_verify`
- [ ] Implement `deaccession` workflow with approval chain and audit logging
- [ ] Add preservation cost estimation in `archive_stats` (storage growth, migration cost projections)
- [ ] Implement LOCKSS/CLOCKSS-compatible package generation for distributed preservation
- [ ] Add `metadata_crosswalk` support for PBCore (public broadcasting metadata standard)

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
