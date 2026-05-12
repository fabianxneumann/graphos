#![no_std]

pub mod error;
pub mod message;
pub mod mailbox;
pub mod channel;

pub use error::IpcError;
pub use message::{Message, MessageType};
pub use mailbox::Mailbox;
pub use channel::{Channel, ChannelId, ChannelRegistry};
