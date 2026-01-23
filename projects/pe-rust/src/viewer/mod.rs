use crate::exports::nyar::pe_assembly::{
    headers::{OptionalHeader, PeHeader},
    program::PeProgram,
    sections::PeSection,
    types::{SubsystemType, TargetArch},
};

/// High-level, intuitive view of PE file structure
/// Provides reorganized and simplified access to PE information
#[derive(Debug, Clone)]
pub struct PeView {
    /// Basic file information
    pub file_info: FileInfo,
    /// PE headers summary
    pub headers: HeaderSummary,
    /// Section overview
    pub sections: Vec<SectionView>,
    /// Import summary
    pub imports: ImportSummary,
    /// Export summary  
    pub exports: ExportSummary,
    /// Security and characteristics
    pub security: SecurityInfo,
}

/// Basic file information
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File size in bytes
    pub size: usize,
    /// Target architecture
    pub architecture: String,
    /// Application type (GUI, Console, etc.)
    pub application_type: String,
    /// Entry point address
    pub entry_point: String,
    /// Preferred load address
    pub image_base: String,
    /// Time stamp
    pub timestamp: String,
}

/// Simplified header information
#[derive(Debug, Clone)]
pub struct HeaderSummary {
    /// DOS header signature check
    pub dos_signature_valid: bool,
    /// PE header signature check
    pub pe_signature_valid: bool,
    /// Number of sections
    pub section_count: u16,
    /// Image size in memory
    pub image_size: u32,
    /// Header size
    pub header_size: u32,
    /// Linker version
    pub linker_version: String,
}

/// Simplified section view
#[derive(Debug, Clone)]
pub struct SectionView {
    /// Section name
    pub name: String,
    /// Section purpose (Code, Data, Resource, etc.)
    pub purpose: String,
    /// Virtual address in memory
    pub virtual_address: String,
    /// Virtual size in memory
    pub virtual_size: u32,
    /// Raw size on disk
    pub raw_size: u32,
    /// Memory protection flags
    pub protection: Vec<String>,
    /// Entropy indicator (for packed detection)
    pub entropy: f32,
}

/// Import information summary
#[derive(Debug, Clone)]
pub struct ImportSummary {
    /// Total number of imported DLLs
    pub dll_count: usize,
    /// Total number of imported functions
    pub function_count: usize,
    /// List of imported DLLs with function counts
    pub dlls: Vec<DllImport>,
    /// Common system DLLs detected
    pub system_dlls: Vec<String>,
    /// Suspicious imports (if any)
    pub suspicious_imports: Vec<String>,
}

/// DLL import details
#[derive(Debug, Clone)]
pub struct DllImport {
    /// DLL name
    pub name: String,
    /// Number of functions imported
    pub function_count: usize,
    /// Function names (first few)
    pub functions: Vec<String>,
}

/// Export information summary
#[derive(Debug, Clone)]
pub struct ExportSummary {
    /// Total number of exported functions
    pub function_count: usize,
    /// Exported function names
    pub functions: Vec<String>,
    /// Has exports
    pub has_exports: bool,
}

/// Security-related information
#[derive(Debug, Clone)]
pub struct SecurityInfo {
    /// File characteristics (ASLR, DEP, etc.)
    pub characteristics: Vec<String>,
    /// DLL characteristics
    pub dll_characteristics: Vec<String>,
    /// Packed/packed detection indicators
    pub packing_indicators: Vec<String>,
    /// Digital signature presence
    pub has_signature: bool,
}

impl PeView {
    /// Create a new PeView from a PeProgram
    pub fn from_program(program: &PeProgram, file_size: usize) -> Self {
        let file_info = Self::extract_file_info(program, file_size);
        let headers = Self::extract_header_summary(&program.header, file_size);
        let sections = Self::extract_sections(&program.sections);
        let imports = Self::extract_imports(&program.import_tables);
        let exports = Self::extract_exports(&program.export_tables);
        let security = Self::extract_security_info(&program.header.optional);

        Self { file_info, headers, sections, imports, exports, security }
    }

    /// Extract basic file information
    fn extract_file_info(program: &PeProgram, file_size: usize) -> FileInfo {
        FileInfo {
            size: file_size,
            architecture: Self::format_architecture(program.arch),
            application_type: Self::format_subsystem(program.subsystem),
            entry_point: format!("0x{:08X}", program.entry_point),
            image_base: format!("0x{:X}", program.header.optional.image_base),
            timestamp: Self::format_timestamp(program.header.coff.time_date_stamp),
        }
    }

    /// Extract header summary
    fn extract_header_summary(header: &PeHeader, _file_size: usize) -> HeaderSummary {
        HeaderSummary {
            dos_signature_valid: header.dos.e_magic == 0x5A4D,     // "MZ"
            pe_signature_valid: header.nt.signature == 0x00004550, // "PE\0\0"
            section_count: header.coff.number_of_sections,
            image_size: header.optional.size_of_image,
            header_size: header.optional.size_of_headers,
            linker_version: format!("{}.{}", header.optional.major_linker_version, header.optional.minor_linker_version),
        }
    }

    /// Extract section information
    fn extract_sections(sections: &[PeSection]) -> Vec<SectionView> {
        sections
            .iter()
            .map(|section| {
                let purpose = Self::determine_section_purpose(&section.name);
                let protection = Self::extract_protection_flags(section.characteristics);
                let entropy = Self::calculate_entropy(&section.data);

                SectionView {
                    name: section.name.trim_end_matches('\0').to_string(),
                    purpose,
                    virtual_address: format!("0x{:08X}", section.virtual_address),
                    virtual_size: section.virtual_size,
                    raw_size: section.size_of_raw_data,
                    protection,
                    entropy,
                }
            })
            .collect()
    }

    /// Extract import information
    fn extract_imports(import_tables: &[crate::exports::nyar::pe_assembly::imports::ImportTable]) -> ImportSummary {
        let dll_count = import_tables.len();
        let function_count: usize = import_tables.iter().map(|table| table.imports.len()).sum();

        let mut dlls: Vec<DllImport> = import_tables
            .iter()
            .map(|table| {
                let functions: Vec<String> = table
                    .imports
                    .iter()
                    .take(5) // Only show first 5 functions
                    .filter_map(|import| {
                        if import.function_name.is_empty() {
                            Some(format!("Ordinal:{}", import.ordinal.unwrap_or(0)))
                        }
                        else {
                            Some(import.function_name.clone())
                        }
                    })
                    .collect();

                DllImport { name: table.dll_name.clone(), function_count: table.imports.len(), functions }
            })
            .collect();

        // Sort by function count (most imports first)
        dlls.sort_by(|a, b| b.function_count.cmp(&a.function_count));

        let system_dlls = Self::identify_system_dlls(&dlls);
        let suspicious_imports = Self::identify_suspicious_imports(&dlls);

        ImportSummary { dll_count, function_count, dlls, system_dlls, suspicious_imports }
    }

    /// Extract export information
    fn extract_exports(export_tables: &[crate::exports::nyar::pe_assembly::exports::ExportTable]) -> ExportSummary {
        let function_count: usize = export_tables.iter().map(|table| table.exports.len()).sum();
        let functions: Vec<String> = export_tables
            .iter()
            .flat_map(|table| {
                table.exports.iter().map(|export| {
                    if export.function_name.is_empty() {
                        format!("Ordinal:{}", export.ordinal)
                    }
                    else {
                        export.function_name.clone()
                    }
                })
            })
            .collect();

        ExportSummary { function_count, functions, has_exports: !export_tables.is_empty() }
    }

    /// Extract security information
    fn extract_security_info(optional_header: &OptionalHeader) -> SecurityInfo {
        let characteristics = Self::extract_file_characteristics(optional_header.dll_characteristics);
        let dll_characteristics = Self::extract_dll_characteristics(optional_header.dll_characteristics);
        let packing_indicators = Vec::new(); // TODO: Implement packing detection
        let has_signature = false; // TODO: Implement signature detection

        SecurityInfo { characteristics, dll_characteristics, packing_indicators, has_signature }
    }

    // Helper methods

    fn format_architecture(arch: TargetArch) -> String {
        match arch {
            TargetArch::X86 => "x86 (32-bit)",
            TargetArch::X64 => "x64 (64-bit)",
        }
        .to_string()
    }

    fn format_subsystem(subsystem: SubsystemType) -> String {
        match subsystem {
            SubsystemType::WindowsGui => "Windows GUI Application",
            SubsystemType::WindowsCui => "Windows Console Application",
            SubsystemType::Native => "Native Application",
            SubsystemType::PosixCui => "POSIX Console Application",
            SubsystemType::Os2Cui => "OS/2 Console Application",
            SubsystemType::Unknown => "Unknown",
            SubsystemType::WindowsCeGui => "Windows CE GUI Application",
            SubsystemType::EfiApplication => "EFI Application",
            SubsystemType::EfiBootServiceDriver => "EFI Boot Service Driver",
            SubsystemType::EfiRuntimeDriver => "EFI Runtime Driver",
            SubsystemType::EfiRom => "EFI ROM",
            SubsystemType::Xbox => "Xbox",
            SubsystemType::WindowsBootApplication => "Windows Boot Application",
        }
        .to_string()
    }

    fn format_timestamp(timestamp: u32) -> String {
        if timestamp == 0 {
            "Not set".to_string()
        }
        else {
            // Simple timestamp formatting (could be enhanced)
            format!("{} (Unix timestamp)", timestamp)
        }
    }

    fn determine_section_purpose(name: &str) -> String {
        let name_lower = name.to_lowercase();
        match name_lower.trim_end_matches('\0') {
            ".text" => "Executable Code",
            ".data" => "Initialized Data",
            ".rdata" => "Read-only Data",
            ".bss" => "Uninitialized Data",
            ".rsrc" => "Resources",
            ".reloc" => "Relocations",
            ".idata" => "Import Data",
            ".edata" => "Export Data",
            ".pdata" => "Exception Data",
            ".xdata" => "Exception Data",
            ".tls" => "Thread Local Storage",
            ".crt" => "C Runtime Data",
            ".debug" => "Debug Information",
            name if name.starts_with(".debug") => "Debug Information",
            name if name.starts_with(".text") => "Code Section",
            name if name.starts_with(".data") => "Data Section",
            _ => "Unknown",
        }
        .to_string()
    }

    fn extract_protection_flags(characteristics: u32) -> Vec<String> {
        let mut flags = Vec::new();

        if characteristics & 0x20000000 != 0 {
            flags.push("Execute".to_string());
        }
        if characteristics & 0x40000000 != 0 {
            flags.push("Read".to_string());
        }
        if characteristics & 0x80000000 != 0 {
            flags.push("Write".to_string());
        }
        if characteristics & 0x02000000 != 0 {
            flags.push("Discardable".to_string());
        }
        if characteristics & 0x08000000 != 0 {
            flags.push("Not Cached".to_string());
        }

        if flags.is_empty() {
            flags.push("No Access".to_string());
        }

        flags
    }

    fn calculate_entropy(data: &[u8]) -> f32 {
        if data.is_empty() {
            return 0.0;
        }

        let mut frequency = [0u32; 256];
        for &byte in data {
            frequency[byte as usize] += 1;
        }

        let len = data.len() as f32;
        let mut entropy = 0.0;

        for &count in &frequency {
            if count > 0 {
                let probability = count as f32 / len;
                entropy -= probability * probability.log2();
            }
        }

        entropy
    }

    fn identify_system_dlls(dlls: &[DllImport]) -> Vec<String> {
        let system_dlls = vec![
            "kernel32.dll",
            "user32.dll",
            "advapi32.dll",
            "gdi32.dll",
            "shell32.dll",
            "msvcrt.dll",
            "ntdll.dll",
            "ws2_32.dll",
            "winmm.dll",
            "comctl32.dll",
            "comdlg32.dll",
            "ole32.dll",
            "oleaut32.dll",
            "uuid.dll",
            "shlwapi.dll",
            "version.dll",
        ];

        dlls.iter()
            .filter(|dll| {
                let name_lower = dll.name.to_lowercase();
                system_dlls.contains(&name_lower.as_str())
            })
            .map(|dll| dll.name.clone())
            .collect()
    }

    fn identify_suspicious_imports(dlls: &[DllImport]) -> Vec<String> {
        let suspicious_functions = vec![
            "createprocess",
            "createremotethread",
            "virtualalloc",
            "virtualprotect",
            "writeprocessmemory",
            "readprocessmemory",
            "openprocess",
            "terminateprocess",
            "getprocaddress",
            "loadlibrary",
            "freelibrary",
            "getmodulehandle",
            "internetopen",
            "internetopenurl",
            "internetreadfile",
            "httpopenrequest",
            "regopenkey",
            "regsetvalue",
            "regcreatekey",
            "regdeletekey",
        ];

        let mut suspicious = Vec::new();

        for dll in dlls {
            for function in &dll.functions {
                let func_lower = function.to_lowercase();
                if suspicious_functions.iter().any(|&sus| func_lower.contains(sus)) {
                    suspicious.push(format!("{}:{}", dll.name, function));
                }
            }
        }

        suspicious
    }

    fn extract_file_characteristics(_dll_characteristics: u16) -> Vec<String> {
        // TODO: Implement file characteristics extraction
        Vec::new()
    }

    fn extract_dll_characteristics(characteristics: u16) -> Vec<String> {
        let mut flags = Vec::new();

        if characteristics & 0x0040 != 0 {
            flags.push("ASLR Enabled".to_string());
        }
        if characteristics & 0x0100 != 0 {
            flags.push("DEP/NX Enabled".to_string());
        }
        if characteristics & 0x0800 != 0 {
            flags.push("SEH Enabled".to_string());
        }
        if characteristics & 0x4000 != 0 {
            flags.push("Terminal Server Aware".to_string());
        }

        flags
    }

    /// Generate a formatted summary string
    pub fn format_summary(&self) -> String {
        let mut output = String::new();

        output.push_str("=== PE File Analysis Summary ===\n\n");

        // File Info
        output.push_str("File Information:\n");
        output.push_str(&format!("  Size: {} bytes\n", self.file_info.size));
        output.push_str(&format!("  Architecture: {}\n", self.file_info.architecture));
        output.push_str(&format!("  Type: {}\n", self.file_info.application_type));
        output.push_str(&format!("  Entry Point: {}\n", self.file_info.entry_point));
        output.push_str(&format!("  Image Base: {}\n", self.file_info.image_base));
        output.push_str(&format!("  Timestamp: {}\n\n", self.file_info.timestamp));

        // Headers
        output.push_str("Header Summary:\n");
        output.push_str(&format!("  DOS Signature: {}\n", if self.headers.dos_signature_valid { "Valid" } else { "Invalid" }));
        output.push_str(&format!("  PE Signature: {}\n", if self.headers.pe_signature_valid { "Valid" } else { "Invalid" }));
        output.push_str(&format!("  Sections: {}\n", self.headers.section_count));
        output.push_str(&format!("  Image Size: {} bytes\n", self.headers.image_size));
        output.push_str(&format!("  Linker: {}\n\n", self.headers.linker_version));

        // Sections
        output.push_str(&format!("Sections ({}):\n", self.sections.len()));
        for (i, section) in self.sections.iter().enumerate() {
            output.push_str(&format!("  [{}] {} ({})\n", i, section.name, section.purpose));
            output.push_str(&format!("       VA: {} ({} bytes)\n", section.virtual_address, section.virtual_size));
            output.push_str(&format!("       Raw: {} bytes\n", section.raw_size));
            output.push_str(&format!("       Protection: {}\n", section.protection.join(", ")));
            output.push_str(&format!("       Entropy: {:.2}\n", section.entropy));
        }
        output.push('\n');

        // Imports
        output.push_str(&format!("Import Summary:\n"));
        output.push_str(&format!("  Total DLLs: {}\n", self.imports.dll_count));
        output.push_str(&format!("  Total Functions: {}\n", self.imports.function_count));
        output.push_str(&format!("  System DLLs: {}\n", self.imports.system_dlls.join(", ")));

        if !self.imports.suspicious_imports.is_empty() {
            output.push_str(&format!("  Suspicious Imports: {}\n", self.imports.suspicious_imports.join(", ")));
        }

        if !self.imports.dlls.is_empty() {
            output.push_str("  Top Imports:\n");
            for dll in self.imports.dlls.iter().take(3) {
                output.push_str(&format!("    {} ({} functions)\n", dll.name, dll.function_count));
                if !dll.functions.is_empty() {
                    output.push_str(&format!("      Functions: {}\n", dll.functions.join(", ")));
                }
            }
        }
        output.push('\n');

        // Exports
        if self.exports.has_exports {
            output.push_str(&format!("Export Summary:\n"));
            output.push_str(&format!("  Total Functions: {}\n", self.exports.function_count));
            if !self.exports.functions.is_empty() {
                output.push_str(&format!("  Functions: {}\n", self.exports.functions.join(", ")));
            }
            output.push('\n');
        }

        // Security
        output.push_str("Security Information:\n");
        if !self.security.characteristics.is_empty() {
            output.push_str(&format!("  Characteristics: {}\n", self.security.characteristics.join(", ")));
        }
        if !self.security.dll_characteristics.is_empty() {
            output.push_str(&format!("  DLL Security: {}\n", self.security.dll_characteristics.join(", ")));
        }
        if !self.security.packing_indicators.is_empty() {
            output.push_str(&format!("  Packing Indicators: {}\n", self.security.packing_indicators.join(", ")));
        }
        output
            .push_str(&format!("  Digital Signature: {}\n", if self.security.has_signature { "Present" } else { "Not Found" }));

        output
    }
}
