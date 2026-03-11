//! Alert notification channels.

pub mod discord;
#[cfg(feature = "email")]
pub mod email;
pub mod file;
pub mod slack;
pub mod sms;
pub mod webhook;

pub use discord::DiscordChannel;
#[cfg(feature = "email")]
pub use email::EmailChannel;
pub use file::FileChannel;
pub use slack::SlackChannel;
pub use sms::SmsChannel;
pub use webhook::WebhookChannel;

use crate::alert::Alert;
use async_trait::async_trait;

/// Alert channel trait.
#[async_trait]
pub trait AlertChannel: Send + Sync {
    /// Send an alert through this channel.
    async fn send(&self, alert: &Alert) -> crate::error::MonitorResult<()>;
}
