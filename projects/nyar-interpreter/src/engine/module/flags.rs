use super::*;
use bitflags::bitflags;

#[rustfmt::skip]
bitflags! {
    /// ## Access control character
    /// | Scopes    | curr module | sub module | curr package | other package |
    /// | :-------- | :---------: | :--------: | :----------: | :-----------: |
    /// | public     |      √     |     √      |      √       |       √       |
    /// | internal   |      √     |     √      |      √       |       ×       |
    /// | private    |      √     |     √      |      ×       |       ×       |
    /// | restricted |      √     |     ×      |      ×       |       ×       |
    ///
    pub struct NyarReadWrite: u8 {
        const SelfRead      = 0b00000001;
        const SelfWrite     = 0b00000010;
        const ModuleRead    = 0b00000100;
        const ModuleWrite   = 0b00001000;
        const PackageRead   = 0b00010000;
        const PackageWrite  = 0b00100000;
        const GlobalRead    = 0b01000000;
        const GlobalWrite   = 0b10000000;
        /// self modify
        const Restricted = Self::SelfRead.bits | Self::SelfWrite.bits;
        ///
        const Private = Self::ModuleRead.bits | Self::ModuleWrite.bits | Self::Restricted.bits;
        /// inside
        const Internal = Self::PackageRead.bits | Self::PackageWrite.bits | Self::Private.bits;
        ///
        const Public = Self::GlobalRead.bits | Self::GlobalWrite.bits | Self::Internal.bits;
    }
}

impl Default for NyarReadWrite {
    fn default() -> Self {
        Self::Public
    }
}

unsafe impl GcSafe for NyarReadWrite {}

unsafe impl GcDrop for NyarReadWrite {}

unsafe impl Scan for NyarReadWrite {
    fn scan(&self, scanner: &mut Scanner<'_>) {
        scanner.scan(&self.bits);
        check_gc_drop(&self.bits);
    }
}
