use super::*;

/// Active stream being published.
#[derive(Clone)]
pub struct ActiveStream {
    /// Stream metadata.
    pub metadata: StreamMetadata,
    /// Publisher connection ID.
    pub publisher_id: u64,
    /// Media broadcast channel.
    pub media_tx: broadcast::Sender<MediaPacket>,
}

/// Stream registry managing active streams.
pub struct StreamRegistry {
    /// Active streams (key: "app/stream_key").
    streams: RwLock<HashMap<String, ActiveStream>>,
}

impl StreamRegistry {
    /// Creates a new stream registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a new stream.
    pub async fn register_stream(
        &self,
        key: String,
        metadata: StreamMetadata,
        publisher_id: u64,
    ) -> NetResult<broadcast::Sender<MediaPacket>> {
        let mut streams = self.streams.write().await;

        if streams.contains_key(&key) {
            return Err(NetError::invalid_state(format!(
                "Stream already exists: {key}"
            )));
        }

        let (tx, _rx) = broadcast::channel(1000);

        let active_stream = ActiveStream {
            metadata,
            publisher_id,
            media_tx: tx.clone(),
        };

        streams.insert(key, active_stream);
        Ok(tx)
    }

    /// Unregisters a stream.
    pub async fn unregister_stream(&self, key: &str) {
        let mut streams = self.streams.write().await;
        streams.remove(key);
    }

    /// Gets a stream for playback.
    pub async fn get_stream(&self, key: &str) -> Option<ActiveStream> {
        let streams = self.streams.read().await;
        streams.get(key).cloned()
    }

    /// Returns the number of active streams.
    pub async fn stream_count(&self) -> usize {
        let streams = self.streams.read().await;
        streams.len()
    }
}

impl Default for StreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}
