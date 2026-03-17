# oximedia-review TODO

## Current Status
- 46 modules (12 subdirectory modules) covering review sessions, annotations, approval workflows, comments/threads, version comparison, drawing tools, export (PDF/CSV/EDL), notifications, real-time collaboration, task tracking
- Core types: SessionId, CommentId, DrawingId, TaskId, VersionId, SessionConfig/Builder, User/UserRole, AnnotationType, WorkflowType
- Dependencies: oximedia-core, oximedia-timecode, serde, chrono, uuid, tokio

## Enhancements
- [ ] Add `SessionConfigBuilder::build()` validation that returns Result instead of using `unwrap_or_default` for required fields
- [ ] Extend `approval_workflow` with conditional approval rules (e.g., auto-approve if all issues marked resolved)
- [ ] Add `comment_thread` resolution tracking (open/resolved/wont-fix) with audit trail
- [ ] Implement `review_metrics` with cycle time analytics (time from submission to approval per stage)
- [ ] Extend `drawing` tools with measurement annotations (rulers, angle markers, safe-area overlays)
- [ ] Add `review_diff` visual diff highlighting for comparing version changes (pixel-level diff overlay)
- [ ] Implement `feedback_round` automatic numbering and deadline tracking per round
- [ ] Add `review_tag` hierarchical tagging with category groups (technical, creative, compliance)

## New Features
- [ ] Add a `review_api` module exposing REST endpoints for external tool integration (Slack, Jira, Shotgrid)
- [ ] Implement `review_link` for generating shareable review URLs with expiration and password protection
- [ ] Add `review_playlist` module for organizing multiple clips into a sequential review session
- [ ] Implement `comparison_mode` with A/B split, onion skin, and difference overlays in `compare`
- [ ] Add `review_snapshot` module for capturing frame grabs with annotations baked in as PNG export
- [ ] Implement `review_automation` for auto-triggering reviews on new version uploads
- [ ] Add `review_permission` fine-grained permissions (can annotate, can approve, can export, can invite)
- [ ] Implement `offline_review` for downloading review packages and syncing comments when reconnected

## Performance
- [ ] Add pagination to `comment` listing for sessions with 1000+ comments
- [ ] Implement delta-based `realtime` sync instead of full-state broadcast for annotation updates
- [ ] Cache rendered `drawing` overlays in `compare` to avoid recomputation on scrub
- [ ] Add lazy loading for `version` history to avoid fetching all versions on session open

## Testing
- [ ] Add tests for `approval_workflow` multi-stage progression with mixed approve/reject decisions
- [ ] Test `realtime` collaboration with simulated concurrent annotation from multiple users
- [ ] Add tests for `export` module generating valid PDF, CSV, and EDL output formats
- [ ] Test `SessionConfigBuilder` with missing required fields and verify graceful defaults
- [ ] Add integration tests for `notify` with mock email and webhook endpoints

## Documentation
- [ ] Document the full review lifecycle: create session -> invite -> annotate -> approve/reject -> export
- [ ] Add workflow diagrams for Simple, MultiStage, Parallel, and Sequential workflow types
- [ ] Document the drawing coordinate system and how annotations map to frame timecodes
- [ ] Add integration guide for connecting review sessions with external project management tools
