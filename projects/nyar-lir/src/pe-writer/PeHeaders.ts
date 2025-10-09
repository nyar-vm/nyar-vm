import { PeWriter } from './PeAssembler';

/**
 * PE file headers structure
 */
export class PeHeaders {
    // DOS Header
    public dos_signature: number = 0x5A4D; // "MZ"
    public bytes_on_last_page: number = 0x90;
    public pages_in_file: number = 0x03;
    public relocations: number = 0x00;
    public size_of_header: number = 0x04;
    public minimum_extra_paragraphs: number = 0x00;
    public maximum_extra_paragraphs: number = 0xFFFF;
    public initial_relative_ss: number = 0x00;
    public initial_sp: number = 0xB8;
    public checksum: number = 0x00;
    public initial_ip: number = 0x00;
    public initial_relative_cs: number = 0x00;
    public addr_relocation_table: number = 0x40;
    public overlay_number: number = 0x00;
    public e_lfanew: number = 0x80; // Offset to NT headers

    // NT Headers
    public nt_signature: number = 0x00004550; // "PE\0\0"
    
    // File Header (COFF Header)
    public machine: number = 0x8664; // x64
    public number_of_sections: number = 2; // .text and .idata sections only
    public time_date_stamp: number = Math.floor(Date.now() / 1000);
    public pointer_to_symbol_table: number = 0;
    public number_of_symbols: number = 0;
    public size_of_optional_header: number = 0xF0; // 240 bytes for PE32+
    public characteristics: number = 0x0022; // EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE

    // Optional Header (PE32+)
    public magic: number = 0x020B; // PE32+
    public major_linker_version: number = 14;
    public minor_linker_version: number = 0;
    public size_of_code: number = 0x1000;
    public size_of_initialized_data: number = 0x1000;
    public size_of_uninitialized_data: number = 0;
    public address_of_entry_point: number = 0x1000;
    public base_of_code: number = 0x1000;
    public image_base: bigint = 0x140000000n; // Default for x64
    public section_alignment: number = 0x1000;
    public file_alignment: number = 0x200;
    public major_operating_system_version: number = 6;
    public minor_operating_system_version: number = 0;
    public major_image_version: number = 0;
    public minor_image_version: number = 0;
    public major_subsystem_version: number = 6;
    public minor_subsystem_version: number = 0;
    public win32_version_value: number = 0;
    public size_of_image: number = 0x3000; // Updated to match actual sections
    public size_of_headers: number = 0x400;
    public checksum_optional: number = 0;
    public subsystem: number = 3; // CONSOLE
    public dll_characteristics: number = 0x0160; // DYNAMIC_BASE | NX_COMPAT | TERMINAL_SERVER_AWARE
    public size_of_stack_reserve: bigint = 0x100000n;
    public size_of_stack_commit: bigint = 0x1000n;
    public size_of_heap_reserve: bigint = 0x100000n;
    public size_of_heap_commit: bigint = 0x1000n;
    public loader_flags: number = 0;
    public number_of_rva_and_sizes: number = 16;

    // Data Directories
    public export_table_rva: number = 0;
    public export_table_size: number = 0;
    public import_table_rva: number = 0x2000;
    public import_table_size: number = 0x100;
    public resource_table_rva: number = 0;
    public resource_table_size: number = 0;
    public exception_table_rva: number = 0;
    public exception_table_size: number = 0;
    public certificate_table_rva: number = 0;
    public certificate_table_size: number = 0;
    public base_relocation_table_rva: number = 0;
    public base_relocation_table_size: number = 0;
    public debug_rva: number = 0;
    public debug_size: number = 0;
    public architecture_rva: number = 0;
    public architecture_size: number = 0;
    public global_ptr_rva: number = 0;
    public global_ptr_size: number = 0;
    public tls_table_rva: number = 0;
    public tls_table_size: number = 0;
    public load_config_table_rva: number = 0;
    public load_config_table_size: number = 0;
    public bound_import_rva: number = 0;
    public bound_import_size: number = 0;
    public iat_rva: number = 0x2000;
    public iat_size: number = 0x100;
    public delay_import_descriptor_rva: number = 0;
    public delay_import_descriptor_size: number = 0;
    public com_runtime_descriptor_rva: number = 0;
    public com_runtime_descriptor_size: number = 0;

    /**
     * Write DOS header to binary writer
     */
    write_dos_header(writer: PeWriter): void {
        writer.write_u16(this.dos_signature);
        writer.write_u16(this.bytes_on_last_page);
        writer.write_u16(this.pages_in_file);
        writer.write_u16(this.relocations);
        writer.write_u16(this.size_of_header);
        writer.write_u16(this.minimum_extra_paragraphs);
        writer.write_u16(this.maximum_extra_paragraphs);
        writer.write_u16(this.initial_relative_ss);
        writer.write_u16(this.initial_sp);
        writer.write_u16(this.checksum);
        writer.write_u16(this.initial_ip);
        writer.write_u16(this.initial_relative_cs);
        writer.write_u16(this.addr_relocation_table);
        writer.write_u16(this.overlay_number);
        
        // Reserved fields
        for (let i = 0; i < 4; i++) {
            writer.write_u16(0);
        }
        
        // OEM fields
        writer.write_u16(0);
        writer.write_u16(0);
        
        // Reserved fields
        for (let i = 0; i < 10; i++) {
            writer.write_u16(0);
        }
        
        writer.write_u32(this.e_lfanew);
    }

    /**
     * Write DOS stub to binary writer
     */
    write_dos_stub(writer: PeWriter): void {
        // Simple DOS stub that prints "This program cannot be run in DOS mode."
        const dos_stub = [
            0x0E, 0x1F, 0xBA, 0x0E, 0x00, 0xB4, 0x09, 0xCD,
            0x21, 0xB8, 0x01, 0x4C, 0xCD, 0x21, 0x54, 0x68,
            0x69, 0x73, 0x20, 0x70, 0x72, 0x6F, 0x67, 0x72,
            0x61, 0x6D, 0x20, 0x63, 0x61, 0x6E, 0x6E, 0x6F,
            0x74, 0x20, 0x62, 0x65, 0x20, 0x72, 0x75, 0x6E,
            0x20, 0x69, 0x6E, 0x20, 0x44, 0x4F, 0x53, 0x20,
            0x6D, 0x6F, 0x64, 0x65, 0x2E, 0x0D, 0x0D, 0x0A,
            0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        ];
        writer.write_bytes(dos_stub);
    }

    /**
     * Write NT headers to binary writer
     */
    write_nt_headers(writer: PeWriter): void {
        // NT Signature
        writer.write_u32(this.nt_signature);
        
        // File Header (COFF Header)
        writer.write_u16(this.machine);
        writer.write_u16(this.number_of_sections);
        writer.write_u32(this.time_date_stamp);
        writer.write_u32(this.pointer_to_symbol_table);
        writer.write_u32(this.number_of_symbols);
        writer.write_u16(this.size_of_optional_header);
        writer.write_u16(this.characteristics);
        
        // Optional Header
        writer.write_u16(this.magic);
        writer.write_u8(this.major_linker_version);
        writer.write_u8(this.minor_linker_version);
        writer.write_u32(this.size_of_code);
        writer.write_u32(this.size_of_initialized_data);
        writer.write_u32(this.size_of_uninitialized_data);
        writer.write_u32(this.address_of_entry_point);
        writer.write_u32(this.base_of_code);
        writer.write_u64(this.image_base);
        writer.write_u32(this.section_alignment);
        writer.write_u32(this.file_alignment);
        writer.write_u16(this.major_operating_system_version);
        writer.write_u16(this.minor_operating_system_version);
        writer.write_u16(this.major_image_version);
        writer.write_u16(this.minor_image_version);
        writer.write_u16(this.major_subsystem_version);
        writer.write_u16(this.minor_subsystem_version);
        writer.write_u32(this.win32_version_value);
        writer.write_u32(this.size_of_image);
        writer.write_u32(this.size_of_headers);
        writer.write_u32(this.checksum_optional);
        writer.write_u16(this.subsystem);
        writer.write_u16(this.dll_characteristics);
        writer.write_u64(this.size_of_stack_reserve);
        writer.write_u64(this.size_of_stack_commit);
        writer.write_u64(this.size_of_heap_reserve);
        writer.write_u64(this.size_of_heap_commit);
        writer.write_u32(this.loader_flags);
        writer.write_u32(this.number_of_rva_and_sizes);
        
        // Data Directories
        writer.write_u32(this.export_table_rva);
        writer.write_u32(this.export_table_size);
        writer.write_u32(this.import_table_rva);
        writer.write_u32(this.import_table_size);
        writer.write_u32(this.resource_table_rva);
        writer.write_u32(this.resource_table_size);
        writer.write_u32(this.exception_table_rva);
        writer.write_u32(this.exception_table_size);
        writer.write_u32(this.certificate_table_rva);
        writer.write_u32(this.certificate_table_size);
        writer.write_u32(this.base_relocation_table_rva);
        writer.write_u32(this.base_relocation_table_size);
        writer.write_u32(this.debug_rva);
        writer.write_u32(this.debug_size);
        writer.write_u32(this.architecture_rva);
        writer.write_u32(this.architecture_size);
        writer.write_u32(this.global_ptr_rva);
        writer.write_u32(this.global_ptr_size);
        writer.write_u32(this.tls_table_rva);
        writer.write_u32(this.tls_table_size);
        writer.write_u32(this.load_config_table_rva);
        writer.write_u32(this.load_config_table_size);
        writer.write_u32(this.bound_import_rva);
        writer.write_u32(this.bound_import_size);
        writer.write_u32(this.iat_rva);
        writer.write_u32(this.iat_size);
        writer.write_u32(this.delay_import_descriptor_rva);
        writer.write_u32(this.delay_import_descriptor_size);
        writer.write_u32(this.com_runtime_descriptor_rva);
        writer.write_u32(this.com_runtime_descriptor_size);
        // Reserved
        writer.write_u32(0);
        writer.write_u32(0);
    }
}