use crate::{
    PeContext,
    exports::nyar::pe_assembly::{
        headers::{CoffHeader, DosHeader, NtHeader, OptionalHeader, PeHeader},
        program::PeProgram as WitPeProgram,
        reader::{Guest, PeError, PeInfo, PeProgram, ReadConfig, PeView},
        sections::PeSection,
        types::{PeError as WitPeError, SubsystemType, TargetArch},
    },
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};

/// PE file parser implementation
pub struct PeParser {
    cursor: Cursor<Vec<u8>>,
}

impl PeParser {
    /// Create a new PE parser from raw data
    pub fn new(data: Vec<u8>) -> Self {
        let cursor = Cursor::new(data.clone());
        Self { cursor }
    }

    /// Reset the cursor position to the beginning
    fn reset(&mut self) {
        self.cursor.set_position(0);
    }

    /// Seek to a specific position
    fn seek(&mut self, pos: u64) -> Result<(), WitPeError> {
        self.cursor.seek(SeekFrom::Start(pos)).map_err(|_| WitPeError::InvalidHeader)?;
        Ok(())
    }

    /// Read DOS header
    fn read_dos_header(&mut self) -> Result<DosHeader, WitPeError> {
        self.seek(0)?;

        let e_magic = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        if e_magic != 0x5A4D {
            // "MZ"
            return Err(WitPeError::InvalidHeader);
        }

        let mut dos_header = DosHeader {
            e_magic,
            e_cblp: 0,
            e_cp: 0,
            e_crlc: 0,
            e_cparhdr: 0,
            e_minalloc: 0,
            e_maxalloc: 0,
            e_ss: 0,
            e_sp: 0,
            e_csum: 0,
            e_ip: 0,
            e_cs: 0,
            e_lfarlc: 0,
            e_ovno: 0,
            e_res: vec![],
            e_oemid: 0,
            e_oeminfo: 0,
            e_res2: vec![],
            e_lfanew: 0,
        };

        // Read remaining DOS header fields
        dos_header.e_cblp = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_cp = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_crlc = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_cparhdr = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_minalloc = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_maxalloc = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_ss = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_sp = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_csum = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_ip = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_cs = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_lfarlc = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_ovno = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        // Read reserved words (8 bytes = 4 u16 values)
        dos_header.e_res = vec![];
        for _ in 0..4 {
            let val = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
            dos_header.e_res.push(val);
        }

        dos_header.e_oemid = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        dos_header.e_oeminfo = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        // Read more reserved words (20 bytes = 10 u16 values)
        dos_header.e_res2 = vec![];
        for _ in 0..10 {
            let val = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
            dos_header.e_res2.push(val);
        }

        dos_header.e_lfanew = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        Ok(dos_header)
    }

    /// Read NT header
    fn read_nt_header(&mut self, dos_header: &DosHeader) -> Result<NtHeader, WitPeError> {
        let nt_offset = dos_header.e_lfanew as u64;
        self.seek(nt_offset)?;

        let signature = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        if signature != 0x00004550 {
            // "PE\0\0"
            return Err(WitPeError::InvalidHeader);
        }

        Ok(NtHeader { signature })
    }

    /// Read COFF header
    fn read_coff_header(&mut self) -> Result<CoffHeader, WitPeError> {
        let machine = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let number_of_sections = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let time_date_stamp = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let pointer_to_symbol_table = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let number_of_symbols = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_optional_header = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let characteristics = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        Ok(CoffHeader {
            machine,
            number_of_sections,
            time_date_stamp,
            pointer_to_symbol_table,
            number_of_symbols,
            size_of_optional_header,
            characteristics,
        })
    }

    /// Read Optional header
    fn read_optional_header(&mut self) -> Result<OptionalHeader, WitPeError> {
        let magic = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let major_linker_version = self.cursor.read_u8().map_err(|_| WitPeError::InvalidHeader)?;
        let minor_linker_version = self.cursor.read_u8().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_code = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_initialized_data = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_uninitialized_data = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let address_of_entry_point = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let base_of_code = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        let image_base = if magic == 0x020b {
            // PE32+
            self.cursor.read_u64::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?
        }
        else {
            // PE32
            self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)? as u64
        };

        let section_alignment = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let file_alignment = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let major_operating_system_version = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let minor_operating_system_version = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let major_image_version = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let minor_image_version = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let major_subsystem_version = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let minor_subsystem_version = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let win32_version_value = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_image = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_headers = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let checksum = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let subsystem = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let dll_characteristics = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_stack_reserve = self.cursor.read_u64::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_stack_commit = self.cursor.read_u64::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_heap_reserve = self.cursor.read_u64::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let size_of_heap_commit = self.cursor.read_u64::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let loader_flags = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;
        let number_of_rva_and_sizes = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidHeader)?;

        Ok(OptionalHeader {
            magic,
            major_linker_version,
            minor_linker_version,
            size_of_code,
            size_of_initialized_data,
            size_of_uninitialized_data,
            address_of_entry_point,
            base_of_code,
            image_base,
            section_alignment,
            file_alignment,
            major_operating_system_version,
            minor_operating_system_version,
            major_image_version,
            minor_image_version,
            major_subsystem_version,
            minor_subsystem_version,
            win32_version_value,
            size_of_image,
            size_of_headers,
            checksum,
            subsystem,
            dll_characteristics,
            size_of_stack_reserve,
            size_of_stack_commit,
            size_of_heap_reserve,
            size_of_heap_commit,
            loader_flags,
            number_of_rva_and_sizes,
        })
    }

    /// Read sections
    fn read_sections(&mut self, coff_header: &CoffHeader, sections_offset: u64) -> Result<Vec<PeSection>, WitPeError> {
        let mut sections = Vec::new();

        self.seek(sections_offset)?;

        for _ in 0..coff_header.number_of_sections {
            let mut name_bytes = vec![0u8; 8];
            self.cursor.read_exact(&mut name_bytes).map_err(|_| WitPeError::InvalidSection)?;

            // Convert null-terminated name to string
            let name = String::from_utf8_lossy(&name_bytes).trim_end_matches('\0').to_string();

            let virtual_size = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let virtual_address = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let size_of_raw_data = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let pointer_to_raw_data = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let pointer_to_relocations = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let pointer_to_line_numbers = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let number_of_relocations = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let number_of_line_numbers = self.cursor.read_u16::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;
            let characteristics = self.cursor.read_u32::<LittleEndian>().map_err(|_| WitPeError::InvalidSection)?;

            // Read section data if available
            let mut data = Vec::new();
            if size_of_raw_data > 0 && pointer_to_raw_data > 0 {
                let current_pos = self.cursor.position();
                self.seek(pointer_to_raw_data as u64)?;

                data = vec![0u8; size_of_raw_data as usize];
                self.cursor.read_exact(&mut data).map_err(|_| WitPeError::InvalidSection)?;

                self.seek(current_pos)?;
            }

            sections.push(PeSection {
                name,
                virtual_size,
                virtual_address,
                size_of_raw_data,
                pointer_to_raw_data,
                pointer_to_relocations,
                pointer_to_line_numbers,
                number_of_relocations,
                number_of_line_numbers,
                characteristics,
                data,
            });
        }

        Ok(sections)
    }

    /// Parse the entire PE file
    pub fn parse(&mut self, config: &ReadConfig) -> Result<WitPeProgram, WitPeError> {
        // Read DOS header
        let dos_header = self.read_dos_header()?;

        // Read NT header
        let nt_header = self.read_nt_header(&dos_header)?;

        // Read COFF header
        let coff_header = self.read_coff_header()?;

        // Read Optional header
        let optional_header = self.read_optional_header()?;

        // Determine architecture
        let arch = match coff_header.machine {
            0x014c => TargetArch::X86, // IMAGE_FILE_MACHINE_I386
            0x8664 => TargetArch::X64, // IMAGE_FILE_MACHINE_AMD64
            _ => return Err(WitPeError::UnsupportedArchitecture),
        };

        // Determine subsystem
        let subsystem = match optional_header.subsystem {
            1 => SubsystemType::Native,
            2 => SubsystemType::WindowsGui,
            3 => SubsystemType::WindowsCui,
            5 => SubsystemType::Os2Cui,
            7 => SubsystemType::PosixCui,
            9 => SubsystemType::WindowsCeGui,
            10 => SubsystemType::EfiApplication,
            11 => SubsystemType::EfiBootServiceDriver,
            12 => SubsystemType::EfiRuntimeDriver,
            13 => SubsystemType::EfiRom,
            14 => SubsystemType::Xbox,
            16 => SubsystemType::WindowsBootApplication,
            _ => SubsystemType::Unknown,
        };

        // Calculate sections offset
        let sections_offset = dos_header.e_lfanew as u64 + 4 + 20 + coff_header.size_of_optional_header as u64;

        // Read sections
        let sections = if config.parse_sections { self.read_sections(&coff_header, sections_offset)? } else { vec![] };

        let header = PeHeader { dos: dos_header, nt: nt_header, coff: coff_header, optional: optional_header };

        Ok(WitPeProgram {
            arch,
            subsystem,
            header,
            sections,
            import_tables: vec![], // TODO: Implement import parsing
            export_tables: vec![], // TODO: Implement export parsing
            debug_info: None,      // TODO: Implement debug info parsing
            entry_point: optional_header.address_of_entry_point,
        })
    }

    /// Validate PE file structure
    pub fn validate(&mut self) -> Result<bool, WitPeError> {
        // Basic validation: check DOS header
        let dos_header = self.read_dos_header()?;

        // Check NT header
        let nt_header = self.read_nt_header(&dos_header)?;

        // Check if we can read COFF header
        let _coff_header = self.read_coff_header()?;

        Ok(true)
    }

    /// Get basic PE information
    pub fn get_info(&mut self) -> Result<PeInfo, WitPeError> {
        let dos_header = self.read_dos_header()?;
        let nt_header = self.read_nt_header(&dos_header)?;
        let coff_header = self.read_coff_header()?;
        let optional_header = self.read_optional_header()?;

        let arch = match coff_header.machine {
            0x014c => TargetArch::X86,
            0x8664 => TargetArch::X64,
            _ => return Err(WitPeError::UnsupportedArchitecture),
        };

        let subsystem = match optional_header.subsystem {
            1 => SubsystemType::Native,
            2 => SubsystemType::WindowsGui,
            3 => SubsystemType::WindowsCui,
            5 => SubsystemType::Os2Cui,
            7 => SubsystemType::PosixCui,
            9 => SubsystemType::WindowsCeGui,
            10 => SubsystemType::EfiApplication,
            11 => SubsystemType::EfiBootServiceDriver,
            12 => SubsystemType::EfiRuntimeDriver,
            13 => SubsystemType::EfiRom,
            14 => SubsystemType::Xbox,
            16 => SubsystemType::WindowsBootApplication,
            _ => SubsystemType::Unknown,
        };

        Ok(PeInfo {
            arch,
            subsystem,
            entry_point: optional_header.address_of_entry_point,
            image_base: optional_header.image_base,
            size_of_image: optional_header.size_of_image,
            number_of_sections: coff_header.number_of_sections,
            date_time_stamp: coff_header.time_date_stamp,
        })
    }
}

impl Guest for PeContext {
    fn read(pe_data: Vec<u8>, config: ReadConfig) -> Result<PeProgram, PeError> {
        let mut parser = PeParser::new(pe_data);
        match parser.parse(&config) {
            Ok(program) => Ok(program),
            Err(e) => Err(e),
        }
    }

    fn validate(pe_data: Vec<u8>) -> Result<bool, PeError> {
        let mut parser = PeParser::new(pe_data);
        parser.validate()
    }

    fn get_info(pe_data: Vec<u8>) -> Result<PeInfo, PeError> {
        let mut parser = PeParser::new(pe_data);
        parser.get_info()
    }

    fn read_view(pe_data: Vec<u8>) -> Result<PeView, PeError> {
        // TODO: Implement read_view functionality
        // For now, return a placeholder error
        Err(PeError::InvalidHeader)
    }

    fn validate_view(view: PeView) -> Result<PeView, PeError> {
        // TODO: Implement validate_view functionality
        // For now, return the view as-is with placeholder validation
        Ok(view)
    }

    fn view_to_program(view: PeView, config: ReadConfig) -> Result<PeProgram, PeError> {
        // TODO: Implement view_to_program functionality
        // For now, return a placeholder error
        Err(PeError::InvalidHeader)
    }
}