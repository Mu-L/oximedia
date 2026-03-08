use super::*;

/// Server connection handler.
pub struct ServerConnection {
    /// Connection information.
    info: ConnectionInfo,
    /// TCP stream.
    stream: TcpStream,
    /// Handshake handler.
    handshake: Handshake,
    /// Chunk stream.
    chunk_stream: ChunkStream,
    /// Configuration.
    config: RtmpServerConfig,
    /// Read buffer.
    read_buffer: BytesMut,
    /// Current timestamp.
    timestamp: u32,
    /// Transaction ID counter.
    transaction_id: f64,
    /// Message sender.
    message_tx: mpsc::UnboundedSender<OutgoingMessage>,
    /// Message receiver.
    message_rx: mpsc::UnboundedReceiver<OutgoingMessage>,
    /// Stream registry.
    stream_registry: Arc<StreamRegistry>,
    /// Authentication handler.
    auth_handler: Arc<dyn AuthHandler>,
    /// Media packet broadcaster (when publishing).
    media_broadcaster: Option<broadcast::Sender<MediaPacket>>,
    /// Media packet receiver (when playing).
    media_receiver: Option<broadcast::Receiver<MediaPacket>>,
    /// Stream metadata.
    stream_metadata: Option<StreamMetadata>,
}

impl ServerConnection {
    /// Creates a new server connection.
    pub(super) fn new(
        id: u64,
        stream: TcpStream,
        address: SocketAddr,
        config: RtmpServerConfig,
        stream_registry: Arc<StreamRegistry>,
        auth_handler: Arc<dyn AuthHandler>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let mut chunk_stream = ChunkStream::new();
        chunk_stream.set_tx_chunk_size(config.chunk_size);
        chunk_stream.set_rx_chunk_size(config.chunk_size);

        Self {
            info: ConnectionInfo::new(id, address),
            stream,
            handshake: Handshake::new(),
            chunk_stream,
            config,
            read_buffer: BytesMut::with_capacity(8192),
            timestamp: 0,
            transaction_id: 1.0,
            message_tx,
            message_rx,
            stream_registry,
            auth_handler,
            media_broadcaster: None,
            media_receiver: None,
            stream_metadata: None,
        }
    }

    /// Returns connection information.
    #[must_use]
    pub const fn info(&self) -> &ConnectionInfo {
        &self.info
    }

    /// Runs the connection handler.
    pub async fn run(mut self) -> NetResult<()> {
        // Perform handshake
        if let Err(e) = self.perform_handshake().await {
            self.cleanup().await;
            return Err(e);
        }

        self.info.state = ServerConnectionState::WaitingConnect;

        // Send initial control messages
        self.send_initial_messages().await?;

        // Main message loop.
        //
        // To avoid overlapping mutable borrows we process each source in turn
        // within a single sequential loop body:
        //   1. Read & process incoming RTMP data from the TCP stream.
        //   2. Drain any outgoing messages queued by the server (non-blocking).
        //   3. Forward any media packets from the broadcast channel (non-blocking).
        //
        // Because `read_and_process_messages` is `&mut self` and the borrow ends
        // before we touch `message_rx` or `media_receiver`, there are no
        // overlapping borrows.
        loop {
            // Phase 1: read and process incoming RTMP chunks.
            match self.read_and_process_messages().await {
                Ok(()) => {}
                Err(NetError::Eof) => break,
                Err(e) => {
                    self.cleanup().await;
                    return Err(e);
                }
            }

            // Phase 2: drain outgoing messages (non-blocking).
            loop {
                match self.message_rx.try_recv() {
                    Ok(msg) => {
                        if let Err(e) = self
                            .send_message_internal(msg.message, msg.chunk_stream_id)
                            .await
                        {
                            self.cleanup().await;
                            return Err(e);
                        }
                    }
                    Err(_) => break,
                }
            }

            // Phase 3: forward pending media packets (non-blocking).
            //
            // We cannot hold a borrow on `self.media_receiver` while calling
            // `self.send_media_packet` (which also borrows `self` mutably).
            // Instead, collect all pending packets first (dropping the borrow),
            // then send them.
            {
                enum RecvOutcome {
                    Packet(MediaPacket),
                    Empty,
                    Closed,
                }

                loop {
                    let outcome = if let Some(rx) = &mut self.media_receiver {
                        match rx.try_recv() {
                            Ok(pkt) => RecvOutcome::Packet(pkt),
                            Err(broadcast::error::TryRecvError::Closed) => RecvOutcome::Closed,
                            Err(_) => RecvOutcome::Empty,
                        }
                    } else {
                        RecvOutcome::Empty
                    };

                    match outcome {
                        RecvOutcome::Packet(packet) => {
                            if let Err(e) = self.send_media_packet(packet).await {
                                self.cleanup().await;
                                return Err(e);
                            }
                        }
                        RecvOutcome::Closed => {
                            // Broadcaster dropped – clean up and exit.
                            self.cleanup().await;
                            return Ok(());
                        }
                        RecvOutcome::Empty => break,
                    }
                }
            }
        }

        self.cleanup().await;
        self.info.state = ServerConnectionState::Closed;
        Ok(())
    }

    /// Cleans up the connection.
    async fn cleanup(&self) {
        // Unregister stream if publishing
        if self.info.state == ServerConnectionState::Publishing {
            if !self.info.app.is_empty() && !self.info.stream_name.is_empty() {
                let stream_key = format!("{}/{}", self.info.app, self.info.stream_name);
                self.stream_registry.unregister_stream(&stream_key).await;
            }
        }
    }

    /// Performs server-side handshake.
    async fn perform_handshake(&mut self) -> NetResult<()> {
        self.update_timestamp();
        self.handshake.set_epoch(self.timestamp);

        // Read C0+C1
        let mut buf = vec![0u8; C0_SIZE + HANDSHAKE_SIZE];
        timeout(self.config.read_timeout, self.stream.read_exact(&mut buf))
            .await
            .map_err(|_| NetError::timeout("Handshake read timeout"))?
            .map_err(|e| NetError::handshake(format!("Failed to read C0+C1: {e}")))?;

        self.info.bytes_received += buf.len() as u64;

        // Parse C0+C1
        self.handshake.parse_c0c1(&buf)?;

        // Generate and send S0+S1+S2
        let s0s1s2 = self.handshake.generate_s0s1s2();
        timeout(self.config.write_timeout, self.stream.write_all(&s0s1s2))
            .await
            .map_err(|_| NetError::timeout("Handshake write timeout"))?
            .map_err(|e| NetError::handshake(format!("Failed to send S0+S1+S2: {e}")))?;

        self.info.bytes_sent += s0s1s2.len() as u64;

        // Read C2
        let mut c2 = vec![0u8; HANDSHAKE_SIZE];
        timeout(self.config.read_timeout, self.stream.read_exact(&mut c2))
            .await
            .map_err(|_| NetError::timeout("Handshake C2 read timeout"))?
            .map_err(|e| NetError::handshake(format!("Failed to read C2: {e}")))?;

        self.info.bytes_received += c2.len() as u64;

        // Parse C2
        self.handshake.parse_c2(&c2)?;

        Ok(())
    }

    /// Sends initial control messages.
    async fn send_initial_messages(&mut self) -> NetResult<()> {
        // Send window acknowledgement size
        let msg = RtmpMessage::Control(ControlMessage::WindowAckSize(self.config.window_ack_size));
        self.send_message_internal(msg, 2).await?;

        // Send set peer bandwidth
        let msg = RtmpMessage::Control(ControlMessage::SetPeerBandwidth {
            size: self.config.window_ack_size,
            limit_type: 2, // Dynamic
        });
        self.send_message_internal(msg, 2).await?;

        // Send chunk size
        let msg = RtmpMessage::Control(ControlMessage::SetChunkSize(self.config.chunk_size));
        self.send_message_internal(msg, 2).await?;

        Ok(())
    }

    /// Reads and processes messages.
    async fn read_and_process_messages(&mut self) -> NetResult<()> {
        // Read chunk data
        let mut temp_buf = vec![0u8; self.config.chunk_size as usize * 4];

        let n = timeout(self.config.read_timeout, self.stream.read(&mut temp_buf))
            .await
            .map_err(|_| NetError::timeout("Read timeout"))?
            .map_err(|e| NetError::connection(format!("Read failed: {e}")))?;

        if n == 0 {
            return Err(NetError::Eof);
        }

        self.read_buffer.extend_from_slice(&temp_buf[..n]);
        self.info.bytes_received += n as u64;

        // Process chunks
        let assembled = self.chunk_stream.process_chunk(&self.read_buffer)?;
        self.read_buffer.clear();

        // Handle messages
        for msg in assembled {
            self.handle_message(msg).await?;
        }

        Ok(())
    }

    /// Handles an assembled message.
    async fn handle_message(&mut self, msg: AssembledMessage) -> NetResult<()> {
        let timestamp = msg.header.timestamp;
        let stream_id = msg.header.message_stream_id;
        let rtmp_msg = self.decode_message(msg)?;

        match &rtmp_msg {
            RtmpMessage::Control(ctrl) => {
                self.handle_control_message(ctrl).await?;
            }
            RtmpMessage::Command(cmd) => {
                self.handle_command_message(cmd).await?;
            }
            RtmpMessage::Data(data) => {
                self.handle_data_message(data).await?;
            }
            RtmpMessage::Audio(payload) => {
                // Broadcast audio packet if publishing
                if self.info.state == ServerConnectionState::Publishing {
                    if let Some(tx) = &self.media_broadcaster {
                        let packet = MediaPacket {
                            packet_type: MediaPacketType::Audio,
                            timestamp,
                            stream_id,
                            data: payload.clone(),
                        };
                        let _ = tx.send(packet);
                    }
                }
            }
            RtmpMessage::Video(payload) => {
                // Broadcast video packet if publishing
                if self.info.state == ServerConnectionState::Publishing {
                    if let Some(tx) = &self.media_broadcaster {
                        let packet = MediaPacket {
                            packet_type: MediaPacketType::Video,
                            timestamp,
                            stream_id,
                            data: payload.clone(),
                        };
                        let _ = tx.send(packet);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handles a data message (metadata).
    async fn handle_data_message(&mut self, data: &DataMessage) -> NetResult<()> {
        if data.handler == "@setDataFrame" || data.handler == "onMetaData" {
            if let Some(metadata) = data.values.first() {
                if let Some(stream_metadata) = &mut self.stream_metadata {
                    stream_metadata.update_from_amf(metadata);
                }

                // Broadcast metadata packet if publishing
                if self.info.state == ServerConnectionState::Publishing {
                    if let Some(tx) = &self.media_broadcaster {
                        let mut encoder = AmfEncoder::new();
                        encoder.encode(&AmfValue::String(data.handler.clone()));
                        for value in &data.values {
                            encoder.encode(value);
                        }

                        let packet = MediaPacket {
                            packet_type: MediaPacketType::Data,
                            timestamp: 0,
                            stream_id: self.info.stream_id,
                            data: encoder.finish(),
                        };
                        let _ = tx.send(packet);
                    }
                }
            }
        }
        Ok(())
    }

    /// Sends a media packet to player.
    async fn send_media_packet(&mut self, packet: MediaPacket) -> NetResult<()> {
        let (csid, msg_type) = match packet.packet_type {
            MediaPacketType::Audio => (4, MessageType::Audio as u8),
            MediaPacketType::Video => (5, MessageType::Video as u8),
            MediaPacketType::Data => (6, MessageType::DataAmf0 as u8),
        };

        let header = MessageHeader::new(
            packet.timestamp,
            packet.data.len() as u32,
            msg_type,
            packet.stream_id,
        );

        let chunks = self
            .chunk_stream
            .encode_message(csid, &header, &packet.data);

        timeout(self.config.write_timeout, self.stream.write_all(&chunks))
            .await
            .map_err(|_| NetError::timeout("Media write timeout"))?
            .map_err(|e| NetError::connection(format!("Media write failed: {e}")))?;

        self.info.bytes_sent += chunks.len() as u64;

        Ok(())
    }

    /// Decodes an assembled message.
    fn decode_message(&self, msg: AssembledMessage) -> NetResult<RtmpMessage> {
        let msg_type = MessageType::from_id(msg.header.message_type).ok_or_else(|| {
            NetError::protocol(format!("Unknown message type: {}", msg.header.message_type))
        })?;

        if msg_type.is_control() {
            let ctrl = ControlMessage::decode(msg_type, &msg.payload)?;
            Ok(RtmpMessage::Control(ctrl))
        } else if msg_type.is_command() {
            self.decode_command(&msg.payload)
        } else if msg_type == MessageType::DataAmf0 {
            self.decode_data(&msg.payload)
        } else if msg_type == MessageType::Audio {
            Ok(RtmpMessage::Audio(msg.payload))
        } else if msg_type == MessageType::Video {
            Ok(RtmpMessage::Video(msg.payload))
        } else {
            Ok(RtmpMessage::Unknown {
                type_id: msg.header.message_type,
                payload: msg.payload,
            })
        }
    }

    /// Decodes a command message.
    fn decode_command(&self, data: &[u8]) -> NetResult<RtmpMessage> {
        let mut dec = AmfDecoder::new(data);

        let name = dec
            .decode()?
            .as_str()
            .ok_or_else(|| NetError::encoding("Command name must be string"))?
            .to_string();

        let transaction_id = dec
            .decode()?
            .as_number()
            .ok_or_else(|| NetError::encoding("Transaction ID must be number"))?;

        let command_object = if dec.has_remaining() {
            Some(dec.decode()?)
        } else {
            None
        };

        let mut args = Vec::new();
        while dec.has_remaining() {
            args.push(dec.decode()?);
        }

        Ok(RtmpMessage::Command(CommandMessage {
            name,
            transaction_id,
            command_object,
            args,
        }))
    }

    /// Decodes a data message.
    fn decode_data(&self, data: &[u8]) -> NetResult<RtmpMessage> {
        let mut dec = AmfDecoder::new(data);

        let handler = dec
            .decode()?
            .as_str()
            .ok_or_else(|| NetError::encoding("Handler must be string"))?
            .to_string();

        let mut values = Vec::new();
        while dec.has_remaining() {
            values.push(dec.decode()?);
        }

        Ok(RtmpMessage::Data(DataMessage { handler, values }))
    }

    /// Handles a control message.
    async fn handle_control_message(&mut self, ctrl: &ControlMessage) -> NetResult<()> {
        match ctrl {
            ControlMessage::SetChunkSize(size) => {
                self.chunk_stream.set_rx_chunk_size(*size);
            }
            ControlMessage::Acknowledgement(_seq) => {
                // Handle acknowledgement
            }
            ControlMessage::WindowAckSize(size) => {
                // Client wants acknowledgements at this window
                let _ = size;
            }
            ControlMessage::UserControl { event, data, .. } => {
                // Handle user control events
                let _ = (event, data);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handles a command message.
    async fn handle_command_message(&mut self, cmd: &CommandMessage) -> NetResult<()> {
        match cmd.name.as_str() {
            "connect" => self.handle_connect(cmd).await?,
            "createStream" => self.handle_create_stream(cmd).await?,
            "publish" => self.handle_publish(cmd).await?,
            "play" => self.handle_play(cmd).await?,
            "deleteStream" => self.handle_delete_stream(cmd).await?,
            "closeStream" => self.handle_close_stream(cmd).await?,
            "releaseStream" => self.handle_release_stream(cmd).await?,
            "FCPublish" => self.handle_fc_publish(cmd).await?,
            "FCUnpublish" => self.handle_fc_unpublish(cmd).await?,
            _ => {}
        }
        Ok(())
    }

    /// Handles connect command.
    async fn handle_connect(&mut self, cmd: &CommandMessage) -> NetResult<()> {
        // Extract connection parameters
        let mut app = String::new();
        let mut tc_url = String::new();
        let params = HashMap::new();

        if let Some(obj) = &cmd.command_object {
            if let Some(app_val) = obj.get("app").and_then(AmfValue::as_str) {
                app = app_val.to_string();
            }
            if let Some(tc_url_val) = obj.get("tcUrl").and_then(AmfValue::as_str) {
                tc_url = tc_url_val.to_string();
            }
        }

        // Authenticate connection
        let auth_result = self
            .auth_handler
            .authenticate_connect(&app, &tc_url, &params)
            .await;

        match auth_result {
            AuthResult::Success => {
                self.info.app = app;
                self.info.state = ServerConnectionState::Connected;

                // Send _result
                let mut props = HashMap::new();
                props.insert(
                    "fmsVer".to_string(),
                    AmfValue::String("OxiMedia/1.0".to_string()),
                );
                props.insert("capabilities".to_string(), AmfValue::Number(127.0));
                props.insert("mode".to_string(), AmfValue::Number(1.0));

                let mut info = HashMap::new();
                info.insert("level".to_string(), AmfValue::String("status".to_string()));
                info.insert(
                    "code".to_string(),
                    AmfValue::String("NetConnection.Connect.Success".to_string()),
                );
                info.insert(
                    "description".to_string(),
                    AmfValue::String("Connection succeeded".to_string()),
                );
                info.insert("objectEncoding".to_string(), AmfValue::Number(0.0));

                let result = CommandMessage::result(cmd.transaction_id, AmfValue::Object(info))
                    .with_command_object(AmfValue::Object(props));

                self.send_command(result, 3).await?;
            }
            AuthResult::Failed(reason) => {
                let mut info = HashMap::new();
                info.insert("level".to_string(), AmfValue::String("error".to_string()));
                info.insert(
                    "code".to_string(),
                    AmfValue::String("NetConnection.Connect.Rejected".to_string()),
                );
                info.insert("description".to_string(), AmfValue::String(reason));

                let error = CommandMessage::error(cmd.transaction_id, AmfValue::Object(info));

                self.send_command(error, 3).await?;

                return Err(NetError::authentication("Connection rejected"));
            }
        }

        Ok(())
    }

    /// Handles createStream command.
    async fn handle_create_stream(&mut self, cmd: &CommandMessage) -> NetResult<()> {
        // Allocate stream ID
        self.info.stream_id = 1;

        // Send _result with stream ID
        let result = CommandMessage::result(
            cmd.transaction_id,
            AmfValue::Number(f64::from(self.info.stream_id)),
        );

        self.send_command(result, 3).await?;

        Ok(())
    }

    /// Handles publish command.
    async fn handle_publish(&mut self, cmd: &CommandMessage) -> NetResult<()> {
        // Extract stream name and publish type
        let stream_name = cmd.args.first().and_then(AmfValue::as_str).unwrap_or("");

        let publish_type_str = cmd.args.get(1).and_then(AmfValue::as_str).unwrap_or("live");

        let publish_type = PublishType::from_str(publish_type_str).unwrap_or(PublishType::Live);

        // Authenticate publish
        let auth_result = self
            .auth_handler
            .authenticate_publish(&self.info.app, stream_name, publish_type)
            .await;

        match auth_result {
            AuthResult::Success => {
                self.info.stream_name = stream_name.to_string();

                // Create stream metadata
                let metadata = StreamMetadata::new(stream_name, &self.info.app);

                // Register stream
                let stream_key = format!("{}/{}", self.info.app, stream_name);
                let media_tx = self
                    .stream_registry
                    .register_stream(stream_key.clone(), metadata.clone(), self.info.id)
                    .await?;

                self.stream_metadata = Some(metadata);
                self.media_broadcaster = Some(media_tx);
                self.info.state = ServerConnectionState::Publishing;

                // Send stream begin
                let user_ctrl = RtmpMessage::Control(ControlMessage::UserControl {
                    event: crate::rtmp::message::UserControlEvent::StreamBegin,
                    data: self.info.stream_id,
                    extra: None,
                });
                self.send_message_internal(user_ctrl, 2).await?;

                // Send onStatus
                let mut info = HashMap::new();
                info.insert("level".to_string(), AmfValue::String("status".to_string()));
                info.insert(
                    "code".to_string(),
                    AmfValue::String("NetStream.Publish.Start".to_string()),
                );
                info.insert(
                    "description".to_string(),
                    AmfValue::String(format!("Publishing {stream_name}")),
                );

                let status = CommandMessage::new("onStatus", 0.0)
                    .with_command_object(AmfValue::Null)
                    .with_arg(AmfValue::Object(info));

                self.send_command(status, 3).await?;
            }
            AuthResult::Failed(reason) => {
                let mut info = HashMap::new();
                info.insert("level".to_string(), AmfValue::String("error".to_string()));
                info.insert(
                    "code".to_string(),
                    AmfValue::String("NetStream.Publish.BadName".to_string()),
                );
                info.insert("description".to_string(), AmfValue::String(reason));

                let status = CommandMessage::new("onStatus", 0.0)
                    .with_command_object(AmfValue::Null)
                    .with_arg(AmfValue::Object(info));

                self.send_command(status, 3).await?;

                return Err(NetError::authentication("Publish rejected"));
            }
        }

        Ok(())
    }

    /// Handles play command.
    async fn handle_play(&mut self, cmd: &CommandMessage) -> NetResult<()> {
        // Extract stream name
        let stream_name = cmd.args.first().and_then(AmfValue::as_str).unwrap_or("");

        // Authenticate play
        let auth_result = self
            .auth_handler
            .authenticate_play(&self.info.app, stream_name)
            .await;

        match auth_result {
            AuthResult::Success => {
                // Get stream from registry
                let stream_key = format!("{}/{}", self.info.app, stream_name);
                let active_stream = self.stream_registry.get_stream(&stream_key).await;

                if let Some(stream) = active_stream {
                    self.info.stream_name = stream_name.to_string();
                    self.info.state = ServerConnectionState::Playing;

                    // Subscribe to media
                    let media_rx = stream.media_tx.subscribe();
                    self.media_receiver = Some(media_rx);

                    // Send stream begin
                    let user_ctrl = RtmpMessage::Control(ControlMessage::UserControl {
                        event: crate::rtmp::message::UserControlEvent::StreamBegin,
                        data: self.info.stream_id,
                        extra: None,
                    });
                    self.send_message_internal(user_ctrl, 2).await?;

                    // Send onStatus (Stream.Play.Reset)
                    let mut reset_info = HashMap::new();
                    reset_info.insert("level".to_string(), AmfValue::String("status".to_string()));
                    reset_info.insert(
                        "code".to_string(),
                        AmfValue::String("NetStream.Play.Reset".to_string()),
                    );
                    reset_info.insert(
                        "description".to_string(),
                        AmfValue::String(format!("Playing {stream_name}")),
                    );

                    let reset_status = CommandMessage::new("onStatus", 0.0)
                        .with_command_object(AmfValue::Null)
                        .with_arg(AmfValue::Object(reset_info));

                    self.send_command(reset_status, 3).await?;

                    // Send onStatus (Stream.Play.Start)
                    let mut start_info = HashMap::new();
                    start_info.insert("level".to_string(), AmfValue::String("status".to_string()));
                    start_info.insert(
                        "code".to_string(),
                        AmfValue::String("NetStream.Play.Start".to_string()),
                    );
                    start_info.insert(
                        "description".to_string(),
                        AmfValue::String(format!("Started playing {stream_name}")),
                    );

                    let start_status = CommandMessage::new("onStatus", 0.0)
                        .with_command_object(AmfValue::Null)
                        .with_arg(AmfValue::Object(start_info));

                    self.send_command(start_status, 3).await?;
                } else {
                    // Stream not found
                    let mut info = HashMap::new();
                    info.insert("level".to_string(), AmfValue::String("error".to_string()));
                    info.insert(
                        "code".to_string(),
                        AmfValue::String("NetStream.Play.StreamNotFound".to_string()),
                    );
                    info.insert(
                        "description".to_string(),
                        AmfValue::String(format!("Stream not found: {stream_name}")),
                    );

                    let status = CommandMessage::new("onStatus", 0.0)
                        .with_command_object(AmfValue::Null)
                        .with_arg(AmfValue::Object(info));

                    self.send_command(status, 3).await?;

                    return Err(NetError::not_found(format!(
                        "Stream not found: {stream_name}"
                    )));
                }
            }
            AuthResult::Failed(reason) => {
                let mut info = HashMap::new();
                info.insert("level".to_string(), AmfValue::String("error".to_string()));
                info.insert(
                    "code".to_string(),
                    AmfValue::String("NetStream.Play.Failed".to_string()),
                );
                info.insert("description".to_string(), AmfValue::String(reason));

                let status = CommandMessage::new("onStatus", 0.0)
                    .with_command_object(AmfValue::Null)
                    .with_arg(AmfValue::Object(info));

                self.send_command(status, 3).await?;

                return Err(NetError::authentication("Play rejected"));
            }
        }

        Ok(())
    }

    /// Handles deleteStream command.
    async fn handle_delete_stream(&mut self, _cmd: &CommandMessage) -> NetResult<()> {
        self.info.stream_id = 0;
        self.info.state = ServerConnectionState::Connected;
        Ok(())
    }

    /// Handles closeStream command.
    async fn handle_close_stream(&mut self, _cmd: &CommandMessage) -> NetResult<()> {
        self.info.state = ServerConnectionState::Connected;
        Ok(())
    }

    /// Handles releaseStream command.
    async fn handle_release_stream(&mut self, cmd: &CommandMessage) -> NetResult<()> {
        // Send _result
        let result = CommandMessage::result(cmd.transaction_id, AmfValue::Undefined);
        self.send_command(result, 3).await?;
        Ok(())
    }

    /// Handles FCPublish command.
    async fn handle_fc_publish(&mut self, _cmd: &CommandMessage) -> NetResult<()> {
        // No response needed
        Ok(())
    }

    /// Handles FCUnpublish command.
    async fn handle_fc_unpublish(&mut self, _cmd: &CommandMessage) -> NetResult<()> {
        // No response needed
        Ok(())
    }

    /// Sends a command message.
    async fn send_command(&mut self, cmd: CommandMessage, csid: u32) -> NetResult<()> {
        let msg = RtmpMessage::Command(cmd);
        self.send_message_internal(msg, csid).await
    }

    /// Sends a message internally.
    async fn send_message_internal(&mut self, message: RtmpMessage, csid: u32) -> NetResult<()> {
        // Encode message payload
        let payload = self.encode_message_payload(&message)?;

        // Update timestamp
        self.update_timestamp();

        // Create message header
        let header = MessageHeader::new(
            self.timestamp,
            payload.len() as u32,
            message.type_id(),
            self.info.stream_id,
        );

        // Encode chunks
        let chunks = self.chunk_stream.encode_message(csid, &header, &payload);

        // Write to stream
        timeout(self.config.write_timeout, self.stream.write_all(&chunks))
            .await
            .map_err(|_| NetError::timeout("Write timeout"))?
            .map_err(|e| NetError::connection(format!("Write failed: {e}")))?;

        self.info.bytes_sent += chunks.len() as u64;

        Ok(())
    }

    /// Encodes message payload.
    fn encode_message_payload(&self, message: &RtmpMessage) -> NetResult<Bytes> {
        match message {
            RtmpMessage::Control(ctrl) => Ok(ctrl.encode()),
            RtmpMessage::Command(cmd) => self.encode_command(cmd),
            RtmpMessage::Data(data) => self.encode_data(data),
            RtmpMessage::Audio(bytes) => Ok(bytes.clone()),
            RtmpMessage::Video(bytes) => Ok(bytes.clone()),
            RtmpMessage::Unknown { payload, .. } => Ok(payload.clone()),
        }
    }

    /// Encodes a command message.
    fn encode_command(&self, cmd: &CommandMessage) -> NetResult<Bytes> {
        let mut enc = AmfEncoder::new();

        enc.encode(&AmfValue::String(cmd.name.clone()));
        enc.encode(&AmfValue::Number(cmd.transaction_id));

        if let Some(obj) = &cmd.command_object {
            enc.encode(obj);
        } else {
            enc.encode(&AmfValue::Null);
        }

        for arg in &cmd.args {
            enc.encode(arg);
        }

        Ok(enc.finish())
    }

    /// Encodes a data message.
    fn encode_data(&self, data: &DataMessage) -> NetResult<Bytes> {
        let mut enc = AmfEncoder::new();

        enc.encode(&AmfValue::String(data.handler.clone()));

        for value in &data.values {
            enc.encode(value);
        }

        Ok(enc.finish())
    }

    /// Updates the timestamp.
    fn update_timestamp(&mut self) {
        if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
            self.timestamp = duration.as_millis() as u32;
        }
    }

    /// Returns a message sender.
    #[must_use]
    pub fn message_sender(&self) -> mpsc::UnboundedSender<OutgoingMessage> {
        self.message_tx.clone()
    }
}
