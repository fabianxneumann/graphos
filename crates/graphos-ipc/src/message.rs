use graphos_core::NodeId;

/// Message types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    Data = 0x0001,
    Signal = 0x0002,
    Request = 0x0003,
    Reply = 0x0004,
    Error = 0x0005,
    GraphEvent = 0x0010,
}

/// 64-byte message — one cache line
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct Message {
    pub sender: NodeId,
    pub receiver: NodeId,
    pub msg_type: MessageType,
    pub payload_len: u16,
    pub timestamp: u32,
    pub payload: [u8; 24],
}

const _: () = assert!(core::mem::size_of::<Message>() == 64);

impl Message {
    /// Create a new message with empty payload.
    pub fn new(sender: NodeId, receiver: NodeId, msg_type: MessageType) -> Self {
        Self {
            sender,
            receiver,
            msg_type,
            payload_len: 0,
            timestamp: 0,
            payload: [0u8; 24],
        }
    }

    /// Create a message with inline payload (copies up to 24 bytes).
    pub fn with_payload(
        sender: NodeId,
        receiver: NodeId,
        msg_type: MessageType,
        data: &[u8],
    ) -> Self {
        let len = if data.len() > 24 { 24 } else { data.len() };
        let mut payload = [0u8; 24];
        // Copy byte-by-byte (no memcpy dependency issues in no_std)
        let mut i = 0;
        while i < len {
            payload[i] = data[i];
            i += 1;
        }
        Self {
            sender,
            receiver,
            msg_type,
            payload_len: len as u16,
            timestamp: 0,
            payload,
        }
    }

    /// Return the valid payload slice.
    pub fn payload_as_slice(&self) -> &[u8] {
        &self.payload[..self.payload_len as usize]
    }
}
