use crate::PagerError;

const MAX_SWAP_PAGES: usize = 1024;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwapDescriptor {
    pub disk_sector: u32,
    pub page_id: u16,
    pub slot_in_page: u8,
    pub flags: u8,
}

impl SwapDescriptor {
    pub const EMPTY: Self = Self {
        disk_sector: 0,
        page_id: 0,
        slot_in_page: 0,
        flags: 0,
    };

    pub fn to_u64(self) -> u64 {
        unsafe { core::mem::transmute::<Self, u64>(self) }
    }

    pub fn from_u64(val: u64) -> Self {
        unsafe { core::mem::transmute::<u64, Self>(val) }
    }
}

const _: () = assert!(core::mem::size_of::<SwapDescriptor>() == 8);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwapEntry {
    pub page_id: u16,
    pub disk_sector: u32,
    pub resident_mask: u64,
    pub dirty_mask: u64,
    _pad: [u8; 2],
}

pub struct SwapTable {
    entries: [SwapEntry; MAX_SWAP_PAGES],
    count: u32,
    next_free_sector: u32,
}

impl SwapTable {
    pub fn new() -> Self {
        Self {
            entries: [SwapEntry {
                page_id: 0,
                disk_sector: 0,
                resident_mask: 0,
                dirty_mask: 0,
                _pad: [0; 2],
            }; MAX_SWAP_PAGES],
            count: 0,
            next_free_sector: 0,
        }
    }

    pub fn alloc_page(&mut self) -> Result<(u16, u32), PagerError> {
        if self.count as usize >= MAX_SWAP_PAGES {
            return Err(PagerError::SwapTableFull);
        }
        let page_id = self.count as u16;
        let sector = self.next_free_sector;
        self.entries[page_id as usize] = SwapEntry {
            page_id,
            disk_sector: sector,
            resident_mask: 0,
            dirty_mask: 0,
            _pad: [0; 2],
        };
        self.count += 1;
        self.next_free_sector += 8; // 8 sectors per NodePage (4096 / 512)
        Ok((page_id, sector))
    }

    pub fn get(&self, page_id: u16) -> Option<&SwapEntry> {
        if (page_id as u32) < self.count {
            Some(&self.entries[page_id as usize])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, page_id: u16) -> Option<&mut SwapEntry> {
        if (page_id as u32) < self.count {
            Some(&mut self.entries[page_id as usize])
        } else {
            None
        }
    }
}
