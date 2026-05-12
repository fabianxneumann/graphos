#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    MailboxFull,
    MailboxEmpty,
    InvalidChannel,
    ChannelClosed,
    BufferTooSmall,
}
