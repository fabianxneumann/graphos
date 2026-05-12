#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotSwapError {
    EdgeNotFound,
    CasFailed,
    CacheFull,
    RegionNotFound,
    InvalidRegion,
    InsufficientRights,
}
