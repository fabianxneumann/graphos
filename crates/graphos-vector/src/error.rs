#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorError {
    IndexOutOfBounds,
    SpaceFull,
    InvalidDimension,
    NoResults,
}
