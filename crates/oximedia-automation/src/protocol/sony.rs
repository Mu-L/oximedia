//! Sony 9-pin RS-422 protocol implementation.

use crate::protocol::serial::SerialPort;
use crate::Result;
use bytes::{BufMut, BytesMut};
use tracing::{debug, info};

/// Sony 9-pin command codes.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum SonyCommand {
    /// Stop command
    Stop = 0x00,
    /// Play command
    Play = 0x01,
    /// Record command
    Record = 0x02,
    /// Fast forward command
    FastForward = 0x10,
    /// Rewind command
    Rewind = 0x20,
}

/// Sony protocol handler.
pub struct SonyProtocol {
    serial: SerialPort,
}

impl SonyProtocol {
    /// Create a new Sony protocol handler.
    pub async fn new(port: &str) -> Result<Self> {
        info!("Creating Sony 9-pin protocol on port: {}", port);

        let serial = SerialPort::new(port, 38400)?;

        Ok(Self { serial })
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<()> {
        self.serial.close()
    }

    /// Send play command.
    pub async fn send_play(&mut self) -> Result<()> {
        debug!("Sending Sony play command");
        self.send_command(SonyCommand::Play).await
    }

    /// Send stop command.
    pub async fn send_stop(&mut self) -> Result<()> {
        debug!("Sending Sony stop command");
        self.send_command(SonyCommand::Stop).await
    }

    /// Send record command.
    pub async fn send_record(&mut self) -> Result<()> {
        debug!("Sending Sony record command");
        self.send_command(SonyCommand::Record).await
    }

    /// Send fast forward command.
    pub async fn send_fast_forward(&mut self) -> Result<()> {
        debug!("Sending Sony fast forward command");
        self.send_command(SonyCommand::FastForward).await
    }

    /// Send rewind command.
    pub async fn send_rewind(&mut self) -> Result<()> {
        debug!("Sending Sony rewind command");
        self.send_command(SonyCommand::Rewind).await
    }

    /// Send a Sony 9-pin command.
    async fn send_command(&mut self, command: SonyCommand) -> Result<()> {
        let mut buffer = BytesMut::with_capacity(16);

        // Sony 9-pin packet format: [CMD1][CMD2][DATA1][DATA2][DATA3][DATA4][CHK]
        buffer.put_u8(command as u8);
        buffer.put_u8(0x00); // CMD2
        buffer.put_u8(0x00); // DATA1
        buffer.put_u8(0x00); // DATA2
        buffer.put_u8(0x00); // DATA3
        buffer.put_u8(0x00); // DATA4

        // Calculate checksum
        let checksum = self.calculate_checksum(&buffer);
        buffer.put_u8(checksum);

        self.serial.write(&buffer)?;

        Ok(())
    }

    /// Calculate Sony protocol checksum.
    fn calculate_checksum(&self, data: &[u8]) -> u8 {
        data.iter().fold(0u8, |acc, &byte| acc.wrapping_add(byte))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_checksum() {
        let protocol = SonyProtocol {
            serial: SerialPort::mock(),
        };

        let data = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
        let checksum = protocol.calculate_checksum(&data);
        assert_eq!(checksum, 0x01);
    }
}
