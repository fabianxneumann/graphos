use crate::content_id::ContentId;
use crate::error::CasError;
use core::sync::atomic::{AtomicU32, Ordering};

/// Entry in the content store index
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ContentEntry {
    pub id: ContentId,
    pub offset: u64,
    pub size: u32,
    pub ref_count: u32,
}

/// Content-Addressed Store.
/// Manages a pre-allocated index table + data region.
pub struct ContentStore {
    index: *mut ContentEntry,
    index_capacity: u32,
    count: AtomicU32,
    data_region: *mut u8,
    data_capacity: usize,
    data_used: AtomicU32,
}

unsafe impl Send for ContentStore {}
unsafe impl Sync for ContentStore {}

impl ContentStore {
    /// Create from pre-allocated memory regions.
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `index_buf` points to valid memory for `index_capacity` ContentEntry items
    /// - `data_buf` points to valid memory of `data_capacity` bytes
    /// - Memory remains valid for the lifetime of this ContentStore
    pub unsafe fn new(
        index_buf: *mut ContentEntry,
        index_capacity: u32,
        data_buf: *mut u8,
        data_capacity: usize,
    ) -> Self {
        Self {
            index: index_buf,
            index_capacity,
            count: AtomicU32::new(0),
            data_region: data_buf,
            data_capacity,
            data_used: AtomicU32::new(0),
        }
    }

    /// Store data, returns its ContentId. Deduplicates automatically.
    pub fn store(&self, data: &[u8]) -> Result<ContentId, CasError> {
        if data.len() > u32::MAX as usize {
            return Err(CasError::DataTooLarge);
        }

        let id = ContentId::from_data(data);

        // Check for deduplication
        if self.exists(&id) {
            // Content already stored, just increment ref count
            let _ = self.add_ref(&id);
            return Ok(id);
        }

        // Check capacity
        let current_count = self.count.load(Ordering::Acquire);
        if current_count >= self.index_capacity {
            return Err(CasError::StoreFull);
        }

        let data_len = data.len() as u32;

        // Reserve space in data region
        let offset = self.data_used.fetch_add(data_len, Ordering::AcqRel);
        if (offset as usize) + (data_len as usize) > self.data_capacity {
            // Roll back (best effort -- concurrent stores may have moved past)
            self.data_used.fetch_sub(data_len, Ordering::Release);
            return Err(CasError::DataTooLarge);
        }

        // Copy data into data region
        unsafe {
            let dest = self.data_region.add(offset as usize);
            core::ptr::copy_nonoverlapping(data.as_ptr(), dest, data_len as usize);
        }

        // Reserve index slot
        let idx = self.count.fetch_add(1, Ordering::AcqRel);
        if idx >= self.index_capacity {
            self.count.fetch_sub(1, Ordering::Release);
            return Err(CasError::StoreFull);
        }

        // Write index entry
        let entry = ContentEntry {
            id,
            offset: offset as u64,
            size: data_len,
            ref_count: 1,
        };
        unsafe {
            core::ptr::write(self.index.add(idx as usize), entry);
        }

        Ok(id)
    }

    /// Retrieve data by ContentId. Returns a slice into the data region.
    pub fn retrieve(&self, id: &ContentId) -> Result<&[u8], CasError> {
        match self.find_entry(id) {
            Some(idx) => {
                let entry = unsafe { &*self.index.add(idx) };
                let offset = entry.offset as usize;
                let size = entry.size as usize;
                let slice =
                    unsafe { core::slice::from_raw_parts(self.data_region.add(offset), size) };
                Ok(slice)
            }
            None => Err(CasError::NotFound),
        }
    }

    /// Check if content exists
    pub fn exists(&self, id: &ContentId) -> bool {
        self.find_entry(id).is_some()
    }

    /// Increment reference count
    pub fn add_ref(&self, id: &ContentId) -> Result<(), CasError> {
        match self.find_entry(id) {
            Some(idx) => {
                let entry = unsafe { &mut *self.index.add(idx) };
                entry.ref_count = entry.ref_count.saturating_add(1);
                Ok(())
            }
            None => Err(CasError::NotFound),
        }
    }

    /// Decrement reference count, returns new count
    pub fn release(&self, id: &ContentId) -> Result<u32, CasError> {
        match self.find_entry(id) {
            Some(idx) => {
                let entry = unsafe { &mut *self.index.add(idx) };
                if entry.ref_count > 0 {
                    entry.ref_count -= 1;
                }
                Ok(entry.ref_count)
            }
            None => Err(CasError::NotFound),
        }
    }

    /// Number of stored entries
    pub fn count(&self) -> u32 {
        self.count.load(Ordering::Acquire)
    }

    /// Find entry by ContentId (linear scan)
    fn find_entry(&self, id: &ContentId) -> Option<usize> {
        let count = self.count.load(Ordering::Acquire) as usize;
        for i in 0..count {
            let entry = unsafe { &*self.index.add(i) };
            if entry.id == *id {
                return Some(i);
            }
        }
        None
    }
}
