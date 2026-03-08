use super::*;

#[derive(Debug, Clone)]
pub struct RelayTarget {
    /// RTMP URL to forward to (rtmp://host:port/app/stream).
    pub url: String,
    /// Whether this relay is currently active.
    pub active: bool,
    /// Bytes forwarded so far.
    pub bytes_forwarded: u64,
    /// Packets dropped due to back-pressure.
    pub packets_dropped: u64,
}

impl RelayTarget {
    /// Creates a new relay target.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            active: true,
            bytes_forwarded: 0,
            packets_dropped: 0,
        }
    }
}

/// Relay manager for forwarding streams to other RTMP endpoints.
///
/// When a publisher pushes media the relay manager fans it out to all
/// configured target URLs via independent broadcast channels.  Each target
/// gets its own `broadcast::Receiver` so slow targets cannot stall the
/// publisher or other targets.
pub struct RelayManager {
    /// Relay targets keyed by stream key.
    targets: Arc<RwLock<HashMap<String, Vec<RelayTarget>>>>,
    /// Media channels for each active stream.
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<MediaPacket>>>>,
}

impl RelayManager {
    /// Creates a new relay manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            targets: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a relay target for the given stream key.
    pub async fn add_target(&self, stream_key: impl Into<String>, url: impl Into<String>) {
        let key = stream_key.into();
        let mut targets = self.targets.write().await;
        targets
            .entry(key)
            .or_insert_with(Vec::new)
            .push(RelayTarget::new(url));
    }

    /// Registers a media channel for a stream that is starting to publish.
    ///
    /// Returns a `broadcast::Receiver` the relay loop should subscribe to.
    pub async fn register_stream(
        &self,
        stream_key: impl Into<String>,
        tx: broadcast::Sender<MediaPacket>,
    ) -> broadcast::Receiver<MediaPacket> {
        let key = stream_key.into();
        let rx = tx.subscribe();
        let mut channels = self.channels.write().await;
        channels.insert(key, tx);
        rx
    }

    /// Forwards a media packet to all targets for a given stream.
    ///
    /// In a production implementation each target would be serviced by a
    /// tokio task that opens a real TCP connection.  Here we track statistics
    /// and mark unreachable targets inactive.
    pub async fn forward(&self, stream_key: &str, packet: &MediaPacket) {
        let mut targets = self.targets.write().await;
        if let Some(list) = targets.get_mut(stream_key) {
            for target in list.iter_mut() {
                if !target.active {
                    continue;
                }
                // In production: send via the connection pool.  Here we
                // update statistics to demonstrate the accounting path.
                target.bytes_forwarded += packet.data.len() as u64;
            }
        }
    }

    /// Removes the channel for a stream that has stopped publishing.
    pub async fn unregister_stream(&self, stream_key: &str) {
        let mut channels = self.channels.write().await;
        channels.remove(stream_key);
    }

    /// Returns statistics for a stream.
    pub async fn stats(&self, stream_key: &str) -> Vec<RelayTarget> {
        let targets = self.targets.read().await;
        targets.get(stream_key).cloned().unwrap_or_default()
    }

    /// Marks a target as inactive (e.g. after a failed connection).
    pub async fn mark_inactive(&self, stream_key: &str, url: &str) {
        let mut targets = self.targets.write().await;
        if let Some(list) = targets.get_mut(stream_key) {
            for target in list.iter_mut() {
                if target.url == url {
                    target.active = false;
                }
            }
        }
    }
}

impl Default for RelayManager {
    fn default() -> Self {
        Self::new()
    }
}
