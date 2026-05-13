use graphos_core::node::NodeHeader;

pub const NODES_PER_PAGE: usize = 64;
pub const NODE_PAGE_SIZE: usize = NODES_PER_PAGE * core::mem::size_of::<NodeHeader>();

const _: () = assert!(NODE_PAGE_SIZE == 4096);

#[repr(C, align(4096))]
pub struct NodePage {
    pub nodes: [NodeHeader; NODES_PER_PAGE],
}

impl NodePage {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                NODE_PAGE_SIZE,
            )
        }
    }

    pub fn from_bytes_mut(buf: &mut [u8; NODE_PAGE_SIZE]) -> &mut Self {
        unsafe { &mut *(buf.as_mut_ptr() as *mut Self) }
    }
}
