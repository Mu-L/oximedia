//! Workflow trigger system.
//!
//! Provides flexible trigger types for initiating workflow execution,
//! including schedule-based, file arrival, API, event-based triggers.

#![allow(dead_code)]

use std::collections::HashMap;

/// Trigger type for workflow execution.
#[derive(Debug, Clone)]
pub enum TriggerType {
    /// Cron-style schedule trigger.
    Schedule(ScheduleTrigger),
    /// File arrival trigger.
    FileArrival(FileArrivalTrigger),
    /// API call trigger.
    ApiCall,
    /// Event-based trigger.
    EventBased(EventTrigger),
    /// Manual start trigger.
    ManualStart,
    /// Dependency-based trigger.
    Dependency,
}

/// Schedule trigger using cron expressions.
#[derive(Debug, Clone)]
pub struct ScheduleTrigger {
    /// Cron expression (e.g. "0 9 * * 1-5" for weekdays at 9am).
    pub cron_expr: String,
    /// Timezone identifier (e.g. "UTC", "`America/New_York`").
    pub timezone: String,
    /// Maximum number of runs (None = unlimited).
    pub max_runs: Option<u32>,
}

impl ScheduleTrigger {
    /// Create a new schedule trigger.
    #[must_use]
    pub fn new(cron_expr: impl Into<String>, timezone: impl Into<String>) -> Self {
        Self {
            cron_expr: cron_expr.into(),
            timezone: timezone.into(),
            max_runs: None,
        }
    }

    /// Set maximum runs.
    #[must_use]
    pub fn with_max_runs(mut self, max_runs: u32) -> Self {
        self.max_runs = Some(max_runs);
        self
    }

    /// Calculate next fire time in milliseconds.
    ///
    /// Simplified implementation: parses HH:MM from cron expression
    /// (fields: second minute hour day month weekday).
    /// Returns the next fire time in ms from `now_ms`.
    #[must_use]
    pub fn next_fire_ms(&self, now_ms: u64) -> u64 {
        // Parse HH:MM from cron: expect format "S M H ..."
        // We extract field index 2 (hour) and 1 (minute).
        let parts: Vec<&str> = self.cron_expr.split_whitespace().collect();
        if parts.len() < 3 {
            // Default: fire in 1 hour
            return now_ms + 3_600_000;
        }

        let minute: u64 = parts[1].parse().unwrap_or(0);
        let hour: u64 = parts[2].parse().unwrap_or(0);

        // Current time components from ms
        let now_secs = now_ms / 1000;
        let seconds_in_day = now_secs % 86400;
        let current_hour = seconds_in_day / 3600;
        let current_minute = (seconds_in_day % 3600) / 60;
        let day_start_ms = now_ms - (seconds_in_day * 1000);

        let target_ms = day_start_ms + hour * 3_600_000 + minute * 60_000;

        if target_ms > now_ms {
            target_ms
        } else if hour == current_hour && minute == current_minute {
            // Same minute - fire in next minute
            now_ms + 60_000
        } else {
            // Tomorrow same time
            target_ms + 86_400_000
        }
    }
}

/// File arrival trigger configuration.
#[derive(Debug, Clone)]
pub struct FileArrivalTrigger {
    /// Directory path to watch.
    pub watch_path: String,
    /// File pattern to match (glob-style, supports `*` wildcard).
    pub pattern: String,
    /// Minimum file size in bytes.
    pub min_size_bytes: u64,
    /// Wait for file to be stable for this many seconds.
    pub stable_for_secs: u32,
}

impl FileArrivalTrigger {
    /// Create a new file arrival trigger.
    #[must_use]
    pub fn new(
        watch_path: impl Into<String>,
        pattern: impl Into<String>,
        min_size_bytes: u64,
        stable_for_secs: u32,
    ) -> Self {
        Self {
            watch_path: watch_path.into(),
            pattern: pattern.into(),
            min_size_bytes,
            stable_for_secs,
        }
    }

    /// Check if a file path and size matches this trigger's criteria.
    ///
    /// Supports glob-style pattern with `*` wildcard matching any sequence of characters.
    #[must_use]
    pub fn matches(&self, path: &str, size: u64) -> bool {
        if size < self.min_size_bytes {
            return false;
        }

        // Extract filename from path
        let filename = path.rsplit('/').next().unwrap_or(path);

        glob_match(&self.pattern, filename)
    }
}

/// Glob-style pattern matching with `*` wildcard.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_bytes = pattern.as_bytes();
    let text_bytes = text.as_bytes();
    glob_match_inner(pattern_bytes, text_bytes)
}

fn glob_match_inner(pattern: &[u8], text: &[u8]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some(&b'*'), _) => {
            // Try matching * with 0 characters, then 1, 2, ... characters
            glob_match_inner(&pattern[1..], text)
                || (!text.is_empty() && glob_match_inner(pattern, &text[1..]))
        }
        (None, Some(_)) | (Some(_), None) => false,
        (Some(&p), Some(&t)) => p == t && glob_match_inner(&pattern[1..], &text[1..]),
    }
}

/// Event-based trigger.
#[derive(Debug, Clone)]
pub struct EventTrigger {
    /// Type of event to watch for.
    pub event_type: String,
    /// Key-value filter conditions that must all match.
    pub filter: HashMap<String, String>,
}

impl EventTrigger {
    /// Create a new event trigger.
    #[must_use]
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            filter: HashMap::new(),
        }
    }

    /// Add a filter condition.
    #[must_use]
    pub fn with_filter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.filter.insert(key.into(), value.into());
        self
    }

    /// Check if an event matches this trigger's conditions.
    #[must_use]
    pub fn matches(&self, event: &HashMap<String, String>) -> bool {
        for (key, expected_value) in &self.filter {
            match event.get(key) {
                Some(actual_value) if actual_value == expected_value => {}
                _ => return false,
            }
        }
        true
    }
}

/// Condition that combines multiple triggers.
#[derive(Debug, Clone)]
pub enum TriggerCondition {
    /// All triggers must fire.
    All(Vec<TriggerType>),
    /// Any trigger can fire.
    Any(Vec<TriggerType>),
    /// No triggers should fire (negation).
    None(Vec<TriggerType>),
}

/// Engine that manages triggers and evaluates them.
#[derive(Debug, Default)]
pub struct TriggerEngine {
    /// Map from workflow ID to its list of triggers.
    triggers: HashMap<String, Vec<TriggerType>>,
}

impl TriggerEngine {
    /// Create a new trigger engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            triggers: HashMap::new(),
        }
    }

    /// Add a trigger for a workflow.
    pub fn add_trigger(&mut self, workflow_id: &str, trigger: TriggerType) {
        self.triggers
            .entry(workflow_id.to_string())
            .or_default()
            .push(trigger);
    }

    /// Evaluate file arrival event and return workflow IDs that should be triggered.
    #[must_use]
    pub fn evaluate_file_arrival(&self, path: &str, size: u64) -> Vec<String> {
        let mut triggered = Vec::new();

        for (workflow_id, triggers) in &self.triggers {
            for trigger in triggers {
                if let TriggerType::FileArrival(file_trigger) = trigger {
                    if file_trigger.matches(path, size) {
                        triggered.push(workflow_id.clone());
                        break; // Only trigger once per workflow
                    }
                }
            }
        }

        triggered
    }

    /// Evaluate an event and return workflow IDs that should be triggered.
    #[must_use]
    pub fn evaluate_event(&self, event: &HashMap<String, String>) -> Vec<String> {
        let mut triggered = Vec::new();

        for (workflow_id, triggers) in &self.triggers {
            for trigger in triggers {
                if let TriggerType::EventBased(event_trigger) = trigger {
                    if event_trigger.matches(event) {
                        triggered.push(workflow_id.clone());
                        break;
                    }
                }
            }
        }

        triggered
    }

    /// Get triggers for a workflow.
    #[must_use]
    pub fn get_triggers(&self, workflow_id: &str) -> &[TriggerType] {
        self.triggers.get(workflow_id).map_or(&[], Vec::as_slice)
    }

    /// Remove all triggers for a workflow.
    pub fn remove_workflow(&mut self, workflow_id: &str) {
        self.triggers.remove(workflow_id);
    }

    /// List all registered workflow IDs.
    #[must_use]
    pub fn workflow_ids(&self) -> Vec<&str> {
        self.triggers.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_trigger_next_fire_future_today() {
        // Set now to 8:00 AM (in ms), trigger at 9:00 AM
        let now_ms = 8 * 3_600_000_u64; // 8h in ms from day start
        let trigger = ScheduleTrigger::new("0 0 9 * * *", "UTC");
        let next = trigger.next_fire_ms(now_ms);
        // Should be 9:00 AM = 9 * 3_600_000
        assert_eq!(next, 9 * 3_600_000);
    }

    #[test]
    fn test_schedule_trigger_next_fire_tomorrow() {
        // Set now to 10:00 AM, trigger at 9:00 AM → tomorrow
        let now_ms = 10 * 3_600_000_u64;
        let trigger = ScheduleTrigger::new("0 0 9 * * *", "UTC");
        let next = trigger.next_fire_ms(now_ms);
        assert!(next > now_ms);
    }

    #[test]
    fn test_schedule_trigger_with_max_runs() {
        let trigger = ScheduleTrigger::new("0 0 9 * * *", "UTC").with_max_runs(5);
        assert_eq!(trigger.max_runs, Some(5));
    }

    #[test]
    fn test_file_arrival_trigger_matches_pattern() {
        let trigger = FileArrivalTrigger::new("/watch", "*.mp4", 1000, 5);
        assert!(trigger.matches("/watch/video.mp4", 5000));
        assert!(!trigger.matches("/watch/video.mp4", 500)); // too small
        assert!(!trigger.matches("/watch/video.mov", 5000)); // wrong ext
    }

    #[test]
    fn test_file_arrival_trigger_wildcard_pattern() {
        let trigger = FileArrivalTrigger::new("/ingest", "mxf_*_v2.mxf", 0, 0);
        assert!(trigger.matches("/ingest/mxf_cam1_v2.mxf", 0));
        assert!(!trigger.matches("/ingest/mxf_cam1_v1.mxf", 0));
    }

    #[test]
    fn test_file_arrival_trigger_no_wildcard() {
        let trigger = FileArrivalTrigger::new("/dir", "exact.mp4", 0, 0);
        assert!(trigger.matches("/dir/exact.mp4", 0));
        assert!(!trigger.matches("/dir/other.mp4", 0));
    }

    #[test]
    fn test_event_trigger_matches() {
        let trigger = EventTrigger::new("media.ready")
            .with_filter("format", "mp4")
            .with_filter("resolution", "4k");

        let mut event = HashMap::new();
        event.insert("format".to_string(), "mp4".to_string());
        event.insert("resolution".to_string(), "4k".to_string());
        event.insert("extra_field".to_string(), "ignored".to_string());

        assert!(trigger.matches(&event));
    }

    #[test]
    fn test_event_trigger_no_match() {
        let trigger = EventTrigger::new("media.ready").with_filter("format", "mp4");

        let mut event = HashMap::new();
        event.insert("format".to_string(), "mov".to_string());

        assert!(!trigger.matches(&event));
    }

    #[test]
    fn test_event_trigger_empty_filter() {
        let trigger = EventTrigger::new("any.event");
        let event = HashMap::new();
        assert!(trigger.matches(&event));
    }

    #[test]
    fn test_trigger_engine_add_and_evaluate_file() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(
            "workflow-1",
            TriggerType::FileArrival(FileArrivalTrigger::new("/ingest", "*.mxf", 1000, 5)),
        );
        engine.add_trigger(
            "workflow-2",
            TriggerType::FileArrival(FileArrivalTrigger::new("/ingest", "*.mp4", 1000, 5)),
        );

        let triggered = engine.evaluate_file_arrival("/ingest/clip.mxf", 50_000);
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0], "workflow-1");
    }

    #[test]
    fn test_trigger_engine_multiple_workflows() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(
            "wf-a",
            TriggerType::FileArrival(FileArrivalTrigger::new("/watch", "*.mp4", 0, 0)),
        );
        engine.add_trigger(
            "wf-b",
            TriggerType::FileArrival(FileArrivalTrigger::new("/watch", "*.mp4", 0, 0)),
        );

        let triggered = engine.evaluate_file_arrival("/watch/test.mp4", 1);
        assert_eq!(triggered.len(), 2);
    }

    #[test]
    fn test_trigger_engine_no_match() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(
            "wf-1",
            TriggerType::FileArrival(FileArrivalTrigger::new("/watch", "*.mxf", 0, 0)),
        );

        let triggered = engine.evaluate_file_arrival("/watch/test.mp4", 1);
        assert!(triggered.is_empty());
    }

    #[test]
    fn test_trigger_engine_remove_workflow() {
        let mut engine = TriggerEngine::new();
        engine.add_trigger(
            "wf-1",
            TriggerType::FileArrival(FileArrivalTrigger::new("/watch", "*.mp4", 0, 0)),
        );

        engine.remove_workflow("wf-1");

        let triggered = engine.evaluate_file_arrival("/watch/test.mp4", 1);
        assert!(triggered.is_empty());
    }

    #[test]
    fn test_trigger_condition_variants() {
        let triggers = vec![TriggerType::ManualStart, TriggerType::ApiCall];
        let _all = TriggerCondition::All(triggers.clone());
        let _any = TriggerCondition::Any(triggers.clone());
        let _none = TriggerCondition::None(triggers);
        // Just verify construction works
    }

    #[test]
    fn test_glob_match_star_extension() {
        assert!(glob_match("*.mp4", "video.mp4"));
        assert!(glob_match("*.mp4", ".mp4"));
        assert!(!glob_match("*.mp4", "video.mov"));
    }

    #[test]
    fn test_glob_match_multiple_stars() {
        assert!(glob_match("*_*_v2.*", "clip_cam1_v2.mxf"));
        assert!(!glob_match("*_*_v2.*", "clip_cam1_v1.mxf"));
    }
}
