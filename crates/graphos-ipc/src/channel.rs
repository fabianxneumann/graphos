use core::sync::atomic::{AtomicBool, Ordering};
use graphos_core::NodeId;
use crate::mailbox::Mailbox;
use crate::message::Message;
use crate::error::IpcError;

/// Channel ID (index into channel registry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelId(pub u32);

/// Bidirectional channel between two nodes.
pub struct Channel {
    pub id: ChannelId,
    pub node_a: NodeId,
    pub node_b: NodeId,
    pub a_to_b: Mailbox,
    pub b_to_a: Mailbox,
    pub closed: AtomicBool,
}

impl Channel {
    /// Create a new channel. Requires pre-allocated buffers for both mailboxes.
    ///
    /// # Safety
    /// - `buf_a_to_b` and `buf_b_to_a` must point to valid memory for `capacity` Messages.
    /// - `capacity` must be a power of 2.
    /// - Buffers must live as long as this Channel.
    pub unsafe fn new(
        id: ChannelId,
        node_a: NodeId,
        node_b: NodeId,
        buf_a_to_b: *mut Message,
        buf_b_to_a: *mut Message,
        capacity: u32,
    ) -> Self {
        Self {
            id,
            node_a,
            node_b,
            a_to_b: Mailbox::new(buf_a_to_b, capacity),
            b_to_a: Mailbox::new(buf_b_to_a, capacity),
            closed: AtomicBool::new(false),
        }
    }

    /// Send from A to B.
    pub fn send_a_to_b(&self, msg: Message) -> Result<(), IpcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }
        self.a_to_b.push(msg)
    }

    /// Send from B to A.
    pub fn send_b_to_a(&self, msg: Message) -> Result<(), IpcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(IpcError::ChannelClosed);
        }
        self.b_to_a.push(msg)
    }

    /// Receive at B (messages from A).
    pub fn recv_at_b(&self) -> Result<Message, IpcError> {
        if self.closed.load(Ordering::Acquire) && self.a_to_b.is_empty() {
            return Err(IpcError::ChannelClosed);
        }
        self.a_to_b.pop()
    }

    /// Receive at A (messages from B).
    pub fn recv_at_a(&self) -> Result<Message, IpcError> {
        if self.closed.load(Ordering::Acquire) && self.b_to_a.is_empty() {
            return Err(IpcError::ChannelClosed);
        }
        self.b_to_a.pop()
    }

    /// Close channel.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    /// Is channel closed?
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

/// Channel Registry — manages all active channels.
pub struct ChannelRegistry {
    channels: *mut Option<Channel>,
    count: u32,
    capacity: u32,
}

unsafe impl Send for ChannelRegistry {}
unsafe impl Sync for ChannelRegistry {}

impl ChannelRegistry {
    /// Create a new registry from pre-allocated storage.
    ///
    /// # Safety
    /// - `storage` must point to valid memory for `capacity` Option<Channel> entries.
    /// - Storage must be zero-initialized (all None).
    /// - Storage must live as long as this ChannelRegistry.
    pub unsafe fn new(storage: *mut Option<Channel>, capacity: u32) -> Self {
        Self {
            channels: storage,
            count: 0,
            capacity,
        }
    }

    /// Get a channel by ID.
    pub fn get(&self, id: ChannelId) -> Option<&Channel> {
        if id.0 >= self.capacity {
            return None;
        }
        // Safety: id.0 < capacity, so the pointer offset is in bounds.
        unsafe {
            let slot = &*self.channels.add(id.0 as usize);
            slot.as_ref()
        }
    }

    /// Allocate the next available channel ID.
    pub fn next_id(&mut self) -> Option<ChannelId> {
        if self.count >= self.capacity {
            return None;
        }
        let id = ChannelId(self.count);
        self.count += 1;
        Some(id)
    }
}
