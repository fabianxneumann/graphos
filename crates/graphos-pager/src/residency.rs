#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidencyState {
    Resident = 0,
    Swapped = 1,
    Loading = 2,
}

impl ResidencyState {
    pub fn from_raw(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Resident,
            1 => Self::Swapped,
            2 => Self::Loading,
            _ => Self::Resident,
        }
    }
}
