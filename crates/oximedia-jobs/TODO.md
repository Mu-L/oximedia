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
- [ ] Add `persistence.rs` support for PostgreSQL backend alongside SQLite for production deployments
- [ ] Implement job result storage in `job_history.rs` with configurable retention period
- [ ] Extend `job_priority_boost.rs` with configurable priority decay for aged jobs
- [ ] Add `rate_limiter.rs` support for per-user and per-tag rate limits (not just global)
- [ ] Implement progress estimation in `job_metrics.rs` based on historical job durations
- [ ] Extend `job_template.rs` with conditional stage execution based on previous stage output

## New Features
- [x] Add a `webhook_notifier.rs` module for sending job status change notifications to external URLs — HMAC-SHA256 signing, per-endpoint event filter, retry with exponential backoff, delivery history, `HttpClient` trait with `NoopHttpClient` / `FailingHttpClient` test doubles; 20 unit tests
- [x] Implement a `cron_scheduler.rs` module for recurring job execution with cron expression parsing — 5-field POSIX syntax, step/range/list, `next_trigger` look-ahead up to 4 years, `CronScheduler::tick()` with enable/disable; 27 unit tests
- [x] Implement WAL-based persistence using file-backed append-only JSON lines (`wal.rs`) — `WalOp::{Upsert,Delete,Checkpoint}`, last-write-wins replay, in-place compaction with atomic rename, configurable auto-compact threshold, crash-safe `sync_all` on every write; 17 unit tests
- [ ] Add a `job_queue_api.rs` REST API module (axum-based) for remote job submission and monitoring
- [ ] Implement a `distributed_lock.rs` module for coordinating workers across multiple processes
- [ ] Add a `job_migration.rs` module for migrating jobs between queue instances
- [ ] Implement a `workflow_dsl.rs` module for defining complex job workflows in YAML/JSON
- [ ] Add a `resource_pool.rs` module for GPU/hardware resource allocation and sharing across workers
- [ ] Implement a `job_replay.rs` module for replaying failed jobs with modified parameters

## Performance
- [ ] Add batch job submission in `queue.rs` to reduce per-job SQLite transaction overhead
- [ ] Implement WAL mode for SQLite in `persistence.rs` for better concurrent read/write performance
- [ ] Add connection pooling with prepared statement caching to `persistence.rs`
- [ ] Optimize `dependency_graph.rs` topological sort for large dependency chains (>10000 jobs)
- [ ] Add work-stealing between workers in `worker_pool.rs` for better load distribution
- [ ] Implement lazy deserialization of JobPayload to avoid parsing unused job parameters

## Testing
- [ ] Add stress tests for concurrent job submission/cancellation (100+ simultaneous operations)
- [ ] Test `retry_policy.rs` circuit breaker behavior under sustained failure conditions
- [ ] Add tests for `job_priority_boost.rs` starvation prevention with mixed priority workloads
- [ ] Test `persistence.rs` crash recovery by killing the process mid-transaction
- [ ] Add `scheduler.rs` tests for complex pipeline DAGs with diamond dependencies
- [ ] Test `worker_pool.rs` auto-scaling behavior under varying load patterns

## Documentation
- [ ] Add a job lifecycle state diagram (Pending -> Running -> Completed/Failed/Cancelled)
- [ ] Document the retry and backoff strategy configuration options
- [ ] Add examples for common transcoding pipelines (ingest -> transcode -> thumbnail -> notify)
