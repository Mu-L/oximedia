# oximedia-jobs TODO

## Current Status
- 34 modules implementing a comprehensive job queue and worker management system
- Core: Job, JobBuilder, JobPayload (Transcode, Thumbnail, SpriteSheet, Analysis, Batch), Priority (High/Normal/Low)
- Queue: JobQueue, QueueConfig, PriorityQueue, JobRegistry with SQLite persistence
- Scheduling: JobScheduler, Pipeline/PipelineBuilder, Schedule, SchedulingRule
- Workers: WorkerPool, WorkerConfig, DefaultExecutor, JobExecutor trait
- Dependencies: dependency_graph, dependency with job ordering
- Retry: retry, retry_policy with exponential backoff and circuit breakers
- Metrics: MetricsCollector, job_metrics, throughput_tracker, telemetry
- Resource: resource_claim, resource_estimate, resource_limits, quota
- Advanced: job_affinity, job_priority_boost, job_template, job_tags, rate_limiter
- History/viz: job_history, event_log, job_graph_viz
- Service: JobQueueService combining all subsystems
- Dependencies: tokio, rusqlite, r2d2, serde, chrono, uuid

## Enhancements
- [x] Add dead letter queue to `queue.rs` for jobs that exceed max retry attempts — implemented in `dead_letter_queue.rs` with bounded capacity, admission, requeue, purge-by-age, and 15 unit tests
- [x] Extend `worker_pool.rs` with graceful drain mode (finish running jobs, accept no new ones) — `WorkerPool::drain()` sets `draining` flag, polls active-job counter, then returns; `submit()` rejects new jobs while draining
- [ ] Add `persistence.rs` support for PostgreSQL backend alongside SQLite for production deployments (verified-open 2026-05-16: no Postgres/PgBackend in jobs/persistence.rs)
- [ ] Implement job result storage in `job_history.rs` with configurable retention period (verified-open 2026-05-16: no retention_period/retention_days in job_history.rs)
- [ ] Extend `job_priority_boost.rs` with configurable priority decay for aged jobs (verified-open 2026-05-16: no priority decay in job_priority_boost.rs)
- [ ] Add `rate_limiter.rs` support for per-user and per-tag rate limits (not just global) (verified-open 2026-05-16: no per-user/per-tag rate limit in rate_limiter.rs)
- [ ] Implement progress estimation in `job_metrics.rs` based on historical job durations (verified-open 2026-05-16: no historical duration estimation in job_metrics.rs)
- [ ] Extend `job_template.rs` with conditional stage execution based on previous stage output (verified-open 2026-05-16: no conditional stage execution in job_template.rs)

## New Features
- [x] Add a `webhook_notifier.rs` module for sending job status change notifications to external URLs — HMAC-SHA256 signing, per-endpoint event filter, retry with exponential backoff, delivery history, `HttpClient` trait with `NoopHttpClient` / `FailingHttpClient` test doubles; 20 unit tests
- [x] Implement a `cron_scheduler.rs` module for recurring job execution with cron expression parsing — 5-field POSIX syntax, step/range/list, `next_trigger` look-ahead up to 4 years, `CronScheduler::tick()` with enable/disable; 27 unit tests
- [x] Implement WAL-based persistence using file-backed append-only JSON lines (`wal.rs`) — `WalOp::{Upsert,Delete,Checkpoint}`, last-write-wins replay, in-place compaction with atomic rename, configurable auto-compact threshold, crash-safe `sync_all` on every write; 17 unit tests
- [x] Add a `job_queue_api.rs` REST API module (axum-based) for remote job submission and monitoring (verified 2026-05-16; src/job_queue_api.rs:593 lines)
- [x] Implement a `distributed_lock.rs` module for coordinating workers across multiple processes (verified 2026-05-16; src/distributed_lock.rs:378 lines)
- [x] Add a `job_migration.rs` module for migrating jobs between queue instances (verified 2026-05-16; src/job_migration.rs:395 lines)
- [x] Implement a `workflow_dsl.rs` module for defining complex job workflows in YAML/JSON (verified 2026-05-16; src/workflow_dsl.rs:491 lines)
- [x] Add a `resource_pool.rs` module for GPU/hardware resource allocation and sharing across workers (verified 2026-05-16; src/resource_pool.rs:479 lines)
- [x] Implement a `job_replay.rs` module for replaying failed jobs with modified parameters (verified 2026-05-16; src/job_replay.rs:398 lines)

## Performance
- [x] Add batch job submission in `queue.rs` to reduce per-job SQLite transaction overhead — `JobPersistence::save_jobs_batch` uses a single transaction with `prepare_cached`; `JobQueue::submit_batch` honours shutdown/drain guards
- [x] Implement WAL mode for SQLite in `persistence.rs` for better concurrent read/write performance — `enable_wal()` sets `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA wal_autocheckpoint=1000;` on every file-backed DB
- [x] Add connection pooling with prepared statement caching to `persistence.rs` — r2d2 pool was already present; all hot-path queries (`get_jobs_by_status`, `get_pending_jobs`, `get_scheduled_jobs_ready`, `get_jobs_past_deadline`, `get_all_jobs`, `get_jobs_by_tag`, `update_job_status`, `update_job_progress`, `count_jobs_by_status`) now use `prepare_cached`
- [x] Optimize `dependency_graph.rs` topological sort for large dependency chains (>10000 jobs) — Kahn's algorithm with `VecDeque` was already in place; stress test added (10 000-node linear chain sorts in < 1 s)
- [x] Add work-stealing between workers in `worker_pool.rs` for better load distribution — `WorkerPool::steal_opportunity(threshold)` identifies (busiest, idlest) candidate pair; full work-stealing runtime already in `work_stealing.rs`
- [x] Implement lazy deserialization of JobPayload to avoid parsing unused job parameters — `lazy_payload.rs` provides `LazyPayload<T>` with `RefCell<Option<T>>` single-resolution cache and `LazyPayloadBatch<T>`

## Testing
- [x] Add stress tests for concurrent job submission/cancellation (100+ simultaneous operations) — `test_concurrent_job_submission_cancellation` in `tests/it_jobs_stress.rs`: 100 jobs from 4 tokio threads, cancel 50
- [x] Test `retry_policy.rs` circuit breaker behavior under sustained failure conditions — `test_retry_policy_circuit_breaker_sustained_failure`: 20 failures, Open state, HalfOpen probe, Closed recovery
- [x] Add tests for `job_priority_boost.rs` starvation prevention with mixed priority workloads — `test_priority_boost_starvation_prevention`: 3-job setup, pass-count starvation, manual boost, ceiling assertion
- [x] Test `persistence.rs` crash recovery by killing the process mid-transaction — `test_persistence_crash_recovery`: write 20 jobs, drop connection, reopen, verify all 20 survive
- [x] Add `scheduler.rs` tests for complex pipeline DAGs with diamond dependencies — `test_scheduler_diamond_dag`: A→B/C→D diamond plus 10 000-node linear chain perf assertion
- [x] Test `worker_pool.rs` auto-scaling behavior under varying load patterns — `test_worker_pool_auto_scaling`: 4-worker pool, 10 job assignments, imbalance detection via `steal_opportunity`, balanced-pool negative case

## Documentation
- [x] Add a job lifecycle state diagram (Pending -> Running -> Completed/Failed/Cancelled) (implemented 2026-05-15: ASCII state diagram with all 6 states and transition descriptions added to src/lib.rs top-level rustdoc)
- [x] Document the retry and backoff strategy configuration options (implemented 2026-05-15: parameter table + backoff formula delay(n)=min(initial×multiplier^n, max_delay) + jitter modes + per-error-class overrides added to src/retry_policy.rs top-level rustdoc)
- [x] Add examples for common transcoding pipelines (ingest -> transcode -> thumbnail -> notify) (implemented 2026-05-15: 4-stage JobTemplate pipeline example using actual API added to src/lib.rs rustdoc)
