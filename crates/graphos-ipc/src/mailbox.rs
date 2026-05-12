use core::sync::atomic::{AtomicU32, Ordering};
use crate::message::Message;
use crate::error::IpcError;

/// SPSC lock-free ring-buffer mailbox for message passing.
pub struct Mailbox {
    buffer: *mut Message,
    capacity: u32,
    head: AtomicU32,
    tail: AtomicU32,
}

// Safety: Synchronization is done via atomics. The SPSC contract
// ensures only one thread writes head and one thread writes tail.
unsafe impl Send for Mailbox {}
unsafe impl Sync for Mailbox {}

impl Mailbox {
    /// Create from pre-allocated buffer.
    ///
    /// # Safety
    /// - `buffer` must point to valid memory for `capacity` Messages.
    /// - `capacity` must be a power of 2.
    /// - The buffer must live as long as this Mailbox.
    pub unsafe fn new(buffer: *mut Message, capacity: u32) -> Self {
        Self {
            buffer,
            capacity,
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
        }
    }

    #[inline]
    fn mask(&self) -> u32 {
        self.capacity - 1
    }

    /// Push a message (producer side).
    pub fn push(&self, msg: Message) -> Result<(), IpcError> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        let next_head = (head + 1) & self.mask();
        if next_head == (tail & self.mask()) {
            return Err(IpcError::MailboxFull);
        }

        // Safety: We are the only producer, and head slot is not occupied
        // because we checked it's not overlapping tail.
        unsafe {
            let slot = self.buffer.add((head & self.mask()) as usize);
            core::ptr::write(slot, msg);
        }

        self.head.store(next_head, Ordering::Release);
        Ok(())
    }

    /// Pop a message (consumer side).
    pub fn pop(&self) -> Result<Message, IpcError> {
        self.try_pop().ok_or(IpcError::MailboxEmpty)
    }

    /// Try to pop a message without blocking.
    pub fn try_pop(&self) -> Option<Message> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail == head {
            return None;
        }

        // Safety: We are the only consumer, and tail slot contains a valid message.
        let msg = unsafe {
            let slot = self.buffer.add((tail & self.mask()) as usize);
            core::ptr::read(slot)
        };

        let next_tail = (tail + 1) & self.mask();
        self.tail.store(next_tail, Ordering::Release);

        Some(msg)
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    /// Check if full.
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        ((head + 1) & self.mask()) == (tail & self.mask())
    }

    /// Number of messages currently in the buffer.
    pub fn len(&self) -> u32 {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        (head.wrapping_sub(tail)) & self.mask()
    }
}
