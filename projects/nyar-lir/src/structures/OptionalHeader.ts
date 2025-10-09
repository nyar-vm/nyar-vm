import {PeWriter} from "@/pe-writer/PeAssembler";

export class OptionalHeader {
    public magic: bigint = 0x020Bn; // PE32+
    public major_linker_version: bigint = 14n;
    public minor_linker_version: bigint = 0n;
    public size_of_code: bigint = 0x1000n;
    public size_of_initialized_data: bigint = 0x1000n;
    public size_of_uninitialized_data: bigint = 0n;
    public address_of_entry_point: bigint = 0x1000n;
    public base_of_code: bigint = 0x1000n;
    public image_base: bigint = 0x140000000n; // Default for x64
    public section_alignment: bigint = 0x1000n;
    public file_alignment: bigint = 0x200n;
    public major_operating_system_version: bigint = 6n;
    public minor_operating_system_version: bigint = 0n;
    public major_image_version: bigint = 0n;
    public minor_image_version: bigint = 0n;
    public major_subsystem_version: bigint = 6n;
    public minor_subsystem_version: bigint = 0n;
    public win32_version_value: bigint = 0n;
    // Updated to match actual sections
    public size_of_image: bigint = 0x3000n;
    public size_of_headers: bigint = 0x400n;
    public checksum_optional: bigint = 0n;
    // WINDOWS_GUI
    public subsystem: bigint = 2n;
    // DYNAMIC_BASE | NX_COMPAT | TERMINAL_SERVER_AWARE
    public dll_characteristics: bigint = 0x0160n;
    public size_of_stack_reserve: bigint = 0x100000n;
    public size_of_stack_commit: bigint = 0x1000n;
    public size_of_heap_reserve: bigint = 0x100000n;
    public size_of_heap_commit: bigint = 0x1000n;
    public loader_flags: bigint = 0n;
    public bigint_of_rva_and_sizes: bigint = 16n;

    public write(writer: PeWriter) {
        throw Error("unimplement")
    }
}