use bitflags::bitflags;

#[rustfmt::skip]
bitflags! {
    /// ## Access control character
    /// | Scopes    | curr module | sub module | curr package | other package |
    /// | :-------- | :---------: | :--------: | :----------: | :-----------: |
    /// | public     |      √     |     √      |      √       |       √       |
    /// | internal   |      √     |     √      |      √       |       ×       |
    /// | private    |      √     |     √      |      ×       |       ×       |
    /// | sealed     |      √     |     ×      |      ×       |       ×       |
    ///
    #[allow(non_upper_case_globals)]
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
        const Sealed = Self::SelfRead.bits | Self::SelfWrite.bits;
        ///
        const Private = Self::ModuleRead.bits | Self::ModuleWrite.bits | Self::Sealed.bits;
        /// inside
        const Internal = Self::PackageRead.bits | Self::PackageWrite.bits | Self::Private.bits;
        ///
        const Public = Self::GlobalRead.bits | Self::GlobalWrite.bits | Self::Internal.bits;
    }
}