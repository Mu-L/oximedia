//! Automation event bus for broadcast workflow messaging.
//!
//! Provides a publish/subscribe event system for decoupled automation components.

/// An automation event published to the event bus.
#[derive(Debug, Clone)]
pub struct AutomationEvent {
    /// Unique event identifier.
    pub id: u64,
    /// Category or type of the event.
    pub event_type: String,
    /// Source component that generated the event.
    pub source: String,
    /// Arbitrary payload associated with the event.
    pub payload: String,
    /// Millisecond timestamp when the event was created.
    pub timestamp_ms: u64,
}

impl AutomationEvent {
    /// Create a new automation event.
    pub fn new(id: u64, event_type: &str, source: &str, payload: &str, timestamp_ms: u64) -> Self {
        Self {
            id,
            event_type: event_type.to_string(),
            source: source.to_string(),
            payload: payload.to_string(),
            timestamp_ms,
        }
    }

    /// Return how many milliseconds ago this event was created relative to `now`.
    pub fn age_ms(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp_ms)
    }
}

/// A subscription that determines which events a subscriber receives.
#[derive(Debug, Clone)]
pub struct EventSubscription {
    /// Identifier for the subscribing component.
    pub subscriber_id: u64,
    /// If `Some`, only events with a matching `event_type` are delivered.
    pub event_type_filter: Option<String>,
    /// If `Some`, only events with a matching `source` are delivered.
    pub source_filter: Option<String>,
}

impl EventSubscription {
    /// Create a new subscription.
    pub fn new(
        subscriber_id: u64,
        event_type_filter: Option<&str>,
        source_filter: Option<&str>,
    ) -> Self {
        Self {
            subscriber_id,
            event_type_filter: event_type_filter.map(str::to_string),
            source_filter: source_filter.map(str::to_string),
        }
    }

    /// Return `true` if `event` satisfies this subscription's filters.
    pub fn matches(&self, event: &AutomationEvent) -> bool {
        if let Some(ref et) = self.event_type_filter {
            if &event.event_type != et {
                return false;
            }
        }
        if let Some(ref sf) = self.source_filter {
            if &event.source != sf {
                return false;
            }
        }
        true
    }
}

/// A simple in-process event bus for automation components.
#[derive(Debug, Default)]
pub struct EventBus {
    events: Vec<AutomationEvent>,
    subscriptions: Vec<EventSubscription>,
    next_id: u64,
}

impl EventBus {
    /// Create a new, empty event bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish an event and return the assigned event id.
    pub fn publish(&mut self, event_type: &str, source: &str, payload: &str) -> u64 {
        self.publish_at(event_type, source, payload, 0)
    }

    /// Publish an event with an explicit timestamp and return the assigned event id.
    pub fn publish_at(
        &mut self,
        event_type: &str,
        source: &str,
        payload: &str,
        timestamp_ms: u64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.events.push(AutomationEvent::new(
            id,
            event_type,
            source,
            payload,
            timestamp_ms,
        ));
        id
    }

    /// Register a subscription and return the subscription's `subscriber_id`.
    pub fn subscribe(&mut self, sub: EventSubscription) -> u64 {
        let id = sub.subscriber_id;
        self.subscriptions.push(sub);
        id
    }

    /// Return all events that match the subscription registered for `subscriber_id`.
    pub fn events_for(&self, subscriber_id: u64) -> Vec<&AutomationEvent> {
        let sub = self
            .subscriptions
            .iter()
            .find(|s| s.subscriber_id == subscriber_id);
        match sub {
            None => vec![],
            Some(sub) => self.events.iter().filter(|e| sub.matches(e)).collect(),
        }
    }

    /// Remove events older than `max_age_ms` milliseconds relative to `now_ms`.
    pub fn clear_old(&mut self, max_age_ms: u64, now_ms: u64) {
        self.events.retain(|e| e.age_ms(now_ms) <= max_age_ms);
    }

    /// Return the total number of events currently held in the bus.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Return the total number of subscriptions registered.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_new() {
        let ev = AutomationEvent::new(1, "clip_start", "playout", "clip_id=42", 1000);
        assert_eq!(ev.id, 1);
        assert_eq!(ev.event_type, "clip_start");
        assert_eq!(ev.source, "playout");
        assert_eq!(ev.payload, "clip_id=42");
        assert_eq!(ev.timestamp_ms, 1000);
    }

    #[test]
    fn test_event_age_ms_normal() {
        let ev = AutomationEvent::new(0, "t", "s", "", 500);
        assert_eq!(ev.age_ms(1000), 500);
    }

    #[test]
    fn test_event_age_ms_zero_when_same() {
        let ev = AutomationEvent::new(0, "t", "s", "", 500);
        assert_eq!(ev.age_ms(500), 0);
    }

    #[test]
    fn test_event_age_ms_saturating() {
        let ev = AutomationEvent::new(0, "t", "s", "", 1000);
        // now < timestamp => saturating_sub returns 0
        assert_eq!(ev.age_ms(500), 0);
    }

    #[test]
    fn test_subscription_matches_no_filter() {
        let sub = EventSubscription::new(1, None, None);
        let ev = AutomationEvent::new(0, "any_type", "any_source", "", 0);
        assert!(sub.matches(&ev));
    }

    #[test]
    fn test_subscription_matches_event_type_filter_pass() {
        let sub = EventSubscription::new(1, Some("clip_start"), None);
        let ev = AutomationEvent::new(0, "clip_start", "playout", "", 0);
        assert!(sub.matches(&ev));
    }

    #[test]
    fn test_subscription_matches_event_type_filter_fail() {
        let sub = EventSubscription::new(1, Some("clip_start"), None);
        let ev = AutomationEvent::new(0, "clip_end", "playout", "", 0);
        assert!(!sub.matches(&ev));
    }

    #[test]
    fn test_subscription_matches_source_filter_pass() {
        let sub = EventSubscription::new(2, None, Some("switcher"));
        let ev = AutomationEvent::new(0, "cut", "switcher", "", 0);
        assert!(sub.matches(&ev));
    }

    #[test]
    fn test_subscription_matches_source_filter_fail() {
        let sub = EventSubscription::new(2, None, Some("switcher"));
        let ev = AutomationEvent::new(0, "cut", "router", "", 0);
        assert!(!sub.matches(&ev));
    }

    #[test]
    fn test_subscription_matches_both_filters() {
        let sub = EventSubscription::new(3, Some("alert"), Some("eas"));
        let good = AutomationEvent::new(0, "alert", "eas", "", 0);
        let bad_type = AutomationEvent::new(1, "info", "eas", "", 0);
        let bad_src = AutomationEvent::new(2, "alert", "monitor", "", 0);
        assert!(sub.matches(&good));
        assert!(!sub.matches(&bad_type));
        assert!(!sub.matches(&bad_src));
    }

    #[test]
    fn test_bus_publish_increments_ids() {
        let mut bus = EventBus::new();
        let id0 = bus.publish("a", "s", "p");
        let id1 = bus.publish("b", "s", "p");
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(bus.event_count(), 2);
    }

    #[test]
    fn test_bus_subscribe_returns_subscriber_id() {
        let mut bus = EventBus::new();
        let sub = EventSubscription::new(99, None, None);
        let returned = bus.subscribe(sub);
        assert_eq!(returned, 99);
        assert_eq!(bus.subscription_count(), 1);
    }

    #[test]
    fn test_bus_events_for_no_subscription() {
        let bus = EventBus::new();
        let events = bus.events_for(0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_bus_events_for_filtered() {
        let mut bus = EventBus::new();
        bus.publish_at("clip_start", "playout", "", 100);
        bus.publish_at("clip_end", "playout", "", 200);
        bus.publish_at("clip_start", "playout", "", 300);

        let sub = EventSubscription::new(1, Some("clip_start"), None);
        bus.subscribe(sub);

        let events = bus.events_for(1);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.event_type == "clip_start"));
    }

    #[test]
    fn test_bus_clear_old() {
        let mut bus = EventBus::new();
        bus.publish_at("a", "s", "", 100);
        bus.publish_at("b", "s", "", 500);
        bus.publish_at("c", "s", "", 900);

        // now = 1000; max_age = 400 => keep events with age <= 400, i.e. timestamp >= 600
        bus.clear_old(400, 1000);
        assert_eq!(bus.event_count(), 1);
    }
}
