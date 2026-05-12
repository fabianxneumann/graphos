#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasError {
    StoreFull,
    NotFound,
    DataTooLarge,
    HashMismatch,
    DiskError,
}
