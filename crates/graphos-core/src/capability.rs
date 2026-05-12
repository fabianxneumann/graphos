bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CapRights: u16 {
        const READ      = 0b0000_0000_0001;
        const WRITE     = 0b0000_0000_0010;
        const EXECUTE   = 0b0000_0000_0100;
        const TRAVERSE  = 0b0000_0000_1000;
        const CREATE    = 0b0000_0001_0000;
        const DELETE    = 0b0000_0010_0000;
        const DELEGATE  = 0b0000_0100_0000;
        const REVOKE    = 0b0000_1000_0000;
        const GRANT     = 0b0001_0000_0000;
        const KERNEL    = 0b1000_0000_0000;
    }
}

/// 8-byte capability token — inspired by seL4.
/// Encodes: rights (what operations), scope (which node types), badge (who).
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct CapabilityToken {
    pub rights: CapRights,
    pub scope: u16,
    pub badge: u32,
}

const _: () = assert!(core::mem::size_of::<CapabilityToken>() == 8);

impl CapabilityToken {
    pub const OPEN: Self = Self {
        rights: CapRights::all(),
        scope: 0xFFFF,
        badge: 0,
    };

    pub const ROOT: Self = Self {
        rights: CapRights::all(),
        scope: 0xFFFF,
        badge: 1,
    };

    pub fn derive(&self, mask: CapRights, new_scope: u16) -> Option<Self> {
        if !self.rights.contains(CapRights::DELEGATE) {
            return None;
        }
        let restricted_rights = self.rights & mask;
        Some(Self {
            rights: restricted_rights,
            scope: new_scope,
            badge: self.badge,
        })
    }

    pub fn satisfies(&self, required: &Self) -> bool {
        self.rights.contains(required.rights)
            && (self.scope == 0xFFFF || self.scope == required.scope)
    }

    #[inline]
    pub fn can_read(&self) -> bool { self.rights.contains(CapRights::READ) }
    #[inline]
    pub fn can_write(&self) -> bool { self.rights.contains(CapRights::WRITE) }
    #[inline]
    pub fn can_traverse(&self) -> bool { self.rights.contains(CapRights::TRAVERSE) }
}

impl core::fmt::Debug for CapabilityToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Cap(rights={:?}, scope={:#06x}, badge={})",
            self.rights, self.scope, self.badge)
    }
}
