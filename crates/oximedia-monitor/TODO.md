# oximedia-monitor TODO

## Current Status
- 48 source files/directories providing comprehensive monitoring, alerting, and observability
- Core: OximediaMonitor with MetricsCollector, SqliteStorage, QueryEngine, AlertManager
- Metrics: system (CPU, memory, disk, network), application (encoding, jobs, workers), quality (PSNR, SSIM, VMAF)
- Alerting: rules engine, pipeline, multi-channel (email, Slack, Discord, webhook, SMS, file)
- Storage: RingBuffer (in-memory) + SQLite (historical), retention policies
- Advanced: anomaly detection, correlation, SLA/SLO tracking, capacity planning, resource forecasting
- Features: default (none), gpu, integrations, email, sqlite, full
- Additional subdirs: audio, cc, compliance, focus, multiviewer, reference, scopes, status, technical, timecode

## Enhancements
- [x] Make the SQLite feature optional but still allow OximediaMonitor to work without it (in-memory only mode)
- [x] Add metric batching in MetricsCollector to reduce SQLite write frequency under high load
- [x] Implement alert deduplication in alerting_pipeline to suppress repeated firings within a cooldown window
- [x] Add exponential backoff to webhook/email alert delivery failures in AlertManager
- [ ] Extend anomaly detection with seasonal decomposition (detect hourly/daily patterns in encoding throughput)
- [x] Add configurable metric aggregation granularity (1s, 10s, 1m, 5m) in metric_aggregation
- [ ] Implement metric cardinality limits in metric_store to prevent unbounded label growth

## New Features
- [ ] Add OpenTelemetry (OTLP) export support alongside existing Prometheus format in metric_export
- [ ] Implement distributed tracing with W3C Trace Context propagation (extend trace_span module)
- [ ] Add StatsD ingestion endpoint to accept metrics from external processes
- [ ] Implement metric recording/playback for debugging -- record live metrics, replay offline
- [ ] Add GPU metrics collection for non-NVIDIA GPUs (Apple Metal via IOKit, AMD via sysfs on Linux)
- [ ] Implement dashboard templating system with variable substitution in dashboard module
- [ ] Add PagerDuty and OpsGenie integration to alerting channels

## Performance
- [ ] Replace SQLite writes with batch INSERT using transactions (collect N samples, write once)
- [ ] Use dashmap more aggressively for hot-path metric lookups instead of Arc<Mutex<HashMap>>
- [ ] Implement metric downsampling for historical data (keep 1s resolution for 1h, 1m for 24h, 5m for 30d)
- [ ] Add connection pooling for SQLite to reduce open/close overhead in concurrent access
- [ ] Profile and optimize sysinfo collection -- skip metrics that are not enabled in config

## Testing
- [ ] Add integration test for the full alert pipeline: metric threshold -> rule evaluation -> notification dispatch
- [ ] Test SLO tracking with synthetic uptime data (99.9% SLO with simulated downtime)
- [ ] Add test for capacity_planner with linear growth trend data verifying forecast accuracy
- [ ] Test metric_export Prometheus format output against promtool lint
- [ ] Add stress test: push 10K metrics/second and verify storage handles the load without data loss

## Documentation
- [ ] Document the feature flag matrix (sqlite, gpu, email, integrations) and which modules each enables
- [ ] Add deployment guide for running with external Prometheus/Grafana stack
- [ ] Document the alert rule expression syntax with examples for common conditions
