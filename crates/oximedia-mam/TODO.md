# oximedia-mam TODO

## Current Status
- 51 modules implementing a comprehensive Media Asset Management system
- Core: MamSystem orchestrator, MamConfig, asset manager, collection manager
- Database: PostgreSQL via sqlx with migrations
- Search: Tantivy full-text search, search_index, catalog_search, smart_search (BM25, Jaccard), asset_search
- API: RESTful (actix-web) and GraphQL (async-graphql) endpoints
- Auth: JWT authentication (jsonwebtoken), bcrypt password hashing, RBAC permissions
- Ingest: ingest, ingest_pipeline, ingest_workflow, batch_ingest
- Assets: asset_lifecycle, asset_status, asset_collection, asset_relations, asset_tag, asset_tag_index, asset_tagging
- Organization: folders, folder_hierarchy, collection_manager, media_catalog, media_linking
- Workflow: workflow, workflow_integration, workflow_trigger, event_bus
- Storage: storage manager, proxy generation, transcoding_profile, transfer_manager
- Metadata: metadata_template, media_format_info
- Advanced: ai_tagging, version_control, versioning, audit logging, webhook, export_package
- Governance: retention_policy, rights_summary, delivery_log, usage_analytics, media_project
- Dependencies: sqlx, tantivy, actix-web, async-graphql, tokio, reqwest, serde, chrono, uuid

## Enhancements
- [x] Add pagination and cursor-based navigation to `asset_search.rs` for large result sets
- [x] Extend `smart_search.rs` with fuzzy matching and typo tolerance (Levenshtein distance)
- [x] Add `retention_policy.rs` support for legal hold overrides that prevent deletion
- [x] Implement `webhook.rs` retry with exponential backoff for failed webhook deliveries
- [x] Extend `permissions.rs` with attribute-based access control (ABAC) in addition to RBAC
- [ ] Add `audit.rs` log export to SIEM-compatible formats (CEF, LEEF)
- [ ] Implement `batch_ingest.rs` support for watch folders with automatic ingest on file arrival
- [ ] Extend `proxy.rs` with adaptive bitrate proxy generation (HLS/DASH) for browser preview

## New Features
- [ ] Add an `archive_tier.rs` module for cold/glacier storage tier management with retrieval scheduling
- [ ] Implement a `collaboration.rs` module for real-time comments, annotations, and review markers on assets
- [ ] Add a `notification.rs` module for email/Slack/Teams notifications on asset and workflow events
- [ ] Implement a `custom_field.rs` module for user-defined metadata fields with type validation
- [ ] Add a `duplicate_detect.rs` module for perceptual hash-based duplicate asset detection
- [ ] Implement an `access_request.rs` module for request/approve access workflows
- [ ] Add a `reporting.rs` module for scheduled report generation (storage usage, asset stats, user activity)
- [ ] Implement a `federated_search.rs` module for searching across multiple MAM instances
- [ ] Add a `trash_bin.rs` module with soft-delete and configurable auto-purge

## Performance
- [ ] Add Tantivy index warming on startup in `search_index.rs` for faster first-query response
- [ ] Implement connection pool tuning in `database.rs` based on concurrent request load
- [ ] Add Redis-based caching layer for frequently accessed assets and search results
- [ ] Optimize `folder_hierarchy.rs` tree queries with materialized path or nested set model
- [ ] Add batch database operations in `asset.rs` for bulk metadata updates (reduce round-trips)
- [ ] Implement incremental search index updates in `search_index.rs` instead of full reindex

## Testing
- [ ] Add integration tests for the full ingest pipeline (file upload -> metadata extraction -> indexing -> proxy gen)
- [ ] Test `permissions.rs` RBAC with complex role hierarchies and permission inheritance
- [ ] Add `workflow_trigger.rs` tests with concurrent asset events firing multiple triggers
- [ ] Test `smart_search.rs` BM25 scoring accuracy with known relevance judgments
- [ ] Add `version_control.rs` tests for branching and merging asset versions
- [ ] Test `transfer_manager.rs` with simulated network failures and partial transfers

## Documentation
- [ ] Add an entity-relationship diagram for the database schema
- [ ] Document the GraphQL API schema with query/mutation examples
- [ ] Add a deployment guide covering PostgreSQL setup, Tantivy index configuration, and storage backends
