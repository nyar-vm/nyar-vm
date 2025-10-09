import {PeSection} from './PeSection';
import {ImportTable} from './ImportTable';
import {PeTargetArchitecture} from './PeTargetArchitecture';
import {PeHeaders} from '../structures/PeHeaders';
import { Relocation, RelocationTable } from './RelocationTable';

export class PeAssembler {
    private architecture: PeTargetArchitecture;
    private pe_headers: PeHeaders;
    private sections: Map<string, PeSection>;
    private import_table: ImportTable;
    private relocations: RelocationTable[] = [];

    constructor(architecture: PeTargetArchitecture) {
        this.architecture = architecture;
        this.pe_headers = new PeHeaders();
        this.sections = new Map();
        this.import_table = new ImportTable();
    }

    /**
     * Add a section to the PE file
     */
    add_section(section: PeSection): void {
        this.sections.set(section.name, section);
    }

    add_relocations(section: PeSection, table: Relocation[], symbols: Map<string, number>): void {
        this.relocations.push(new RelocationTable(section, table, symbols));
    }

    /**
     * Add code section with machine code
     */
    add_code_section(code: Uint8Array): void {
        const text_section = PeSection.create_text_section(code);
        this.add_section(text_section);
    }

    /**
     * Add import table
     */
    set_import_table(import_table: ImportTable): void {
        this.import_table = import_table;
    }

    private apply_relocations() {
        const import_section = this.sections.get('.idata'.padEnd(8, '\0'));

        for (const reloc_table of this.relocations) {
            const section = reloc_table.section;
            const symbols = reloc_table.symbols;

            for (const reloc of reloc_table.table) {
                const offset = reloc.offset;
                const symbol = reloc.symbol;
                const type = reloc.type;

                if (type === 'rip_relative_import') {
                    if (!import_section) {
                        throw new Error('Cannot apply import relocations before .idata section is added.');
                    }
                    const import_rva = this.import_table.get_import_rva(symbol);
                    if (import_rva === undefined) {
                        throw new Error(`Import symbol not found: ${symbol}`);
                    }
                    const target_rva = import_section.virtual_address + import_rva;
                    const rip = section.virtual_address + offset + 4;
                    const relative_offset = target_rva - rip;
                    section.data.set(new Uint32Array([relative_offset]), offset);
                } else if (type === 'rip_relative') {
                    const symbol_offset = symbols.get(symbol);
                    if (symbol_offset === undefined) {
                        throw new Error(`Symbol not found: ${symbol}`);
                    }
                    const target_rva = section.virtual_address + symbol_offset;
                    const rip = section.virtual_address + offset + 4;
                    const relative_offset = target_rva - rip;
                    section.data.set(new Uint32Array([relative_offset]), offset);
                }
            }
        }
    }

    /**
     * Build the complete PE file
     */
    build(): Uint8Array {
        const writer = new PeWriter();

        // Add import table if it has imports
        if (this.import_table.has_imports()) {
            const import_section = PeSection.create_idata_section(new Uint8Array(0)); // Placeholder
            this.add_section(import_section);
        }

        // Update headers with section count
        this.pe_headers.number_of_sections = this.sections.size;
        const sections_array = Array.from(this.sections.values());

        // Generate import data before calculating layout
        if (this.import_table.has_imports()) {
            const import_section = sections_array.find(s => s.name.startsWith('.idata'));
            if (import_section) {
                // Tentatively set a virtual address to generate the table
                import_section.virtual_address = 0; 
                const import_data = this.import_table.generate_import_table(0);
                import_section.set_data(import_data);
            }
        }

        // Calculate section virtual addresses and file offsets
        let current_rva = 0x1000; // Start at 4KB boundary
        let current_file_offset = 0x400; // Start after headers

        for (const section of sections_array) {
            section.virtual_address = current_rva;
            section.set_raw_data_pointer(current_file_offset);

            const aligned_virtual_size = Math.ceil(section.get_virtual_size() / 0x1000) * 0x1000;
            current_rva += aligned_virtual_size;

            const aligned_file_size = Math.ceil(section.get_file_size() / this.pe_headers.file_alignment) * this.pe_headers.file_alignment;
            current_file_offset += aligned_file_size;
        }

        // Update import table RVA and data now that we have the final virtual address
        if (this.import_table.has_imports()) {
            const import_section = sections_array.find(s => s.name.startsWith('.idata'));
            if (import_section) {
                const final_import_data = this.import_table.generate_import_table(import_section.virtual_address);
                import_section.set_data(final_import_data);

                this.pe_headers.import_table_rva = import_section.virtual_address;
                this.pe_headers.import_table_size = import_section.get_virtual_size();
                this.pe_headers.iat_rva = import_section.virtual_address + this.import_table.get_iat_offset();
                this.pe_headers.iat_size = this.import_table.get_iat_size();
            }
        }

        // Apply relocations now that all sections have their virtual addresses
        this.apply_relocations();

        // Update size of image and headers
        this.pe_headers.size_of_image = current_rva;
        this.pe_headers.size_of_headers = 0x400;

        // Write DOS header and stub
        this.pe_headers.write_dos_header(writer);
        this.pe_headers.write_dos_stub(writer);

        // Align to NT headers offset
        writer.align(this.pe_headers.e_lfanew);

        // Write NT headers
        this.pe_headers.write_nt_headers(writer);

        // Write section headers
        for (const section of sections_array) {
            section.write_section_header(writer);
        }

        // Write section data
        for (const section of sections_array) {
            writer.align(section.pointer_to_raw_data);
            section.write_section_data(writer);
        }

        return writer.get_bytes();
    }
}


export class X86Assembler extends PeAssembler {
    constructor() {
        super(PeTargetArchitecture.X86);
    }
}

export class X64Assembler extends PeAssembler {
    constructor() {
        super(PeTargetArchitecture.X64);
    }
}


/**
 * use little_endian
 */
export class PeWriter {
    private buffer: Uint8Array;
    private position: number;

    constructor(initial_capacity: number = 1024, little_endian: boolean = true) {
        this.buffer = new Uint8Array(initial_capacity);
        this.position = 0;
    }

    public write(pe: PeAssembler): Uint8Array {
        return pe.build();
    }

    private ensure_capacity(additional_bytes: number) {
        const required_capacity = this.position + additional_bytes;
        if (required_capacity > this.buffer.length) {
            const new_capacity = Math.max(this.buffer.length * 2, required_capacity);
            const new_buffer = new Uint8Array(new_capacity);
            new_buffer.set(this.buffer);
            this.buffer = new_buffer;
        }
    }

    write_u8(value: number): this {
        this.ensure_capacity(1);
        this.buffer[this.position++] = value & 0xff;
        return this;
    }

    write_u16(value: number): this {
        this.ensure_capacity(2);
        this.buffer[this.position++] = value & 0xff;
        this.buffer[this.position++] = (value >> 8) & 0xff;
        return this;
    }

    write_u32(value: number): this {
        this.ensure_capacity(4);
        this.buffer[this.position++] = value & 0xff;
        this.buffer[this.position++] = (value >> 8) & 0xff;
        this.buffer[this.position++] = (value >> 16) & 0xff;
        this.buffer[this.position++] = (value >> 24) & 0xff;
        return this;
    }

    write_u64(value: bigint): this {
        this.ensure_capacity(8);
        const low = Number(value & 0xffffffffn);
        const high = Number((value >> 32n) & 0xffffffffn);
        this.write_u32(low);
        this.write_u32(high);
        return this;
    }

    write_i8(value: number): this {
        return this.write_u8(value < 0 ? value + 256 : value);
    }

    write_i16(value: number): this {
        return this.write_u16(value < 0 ? value + 65536 : value);
    }

    write_i32(value: number): this {
        return this.write_u32(value >>> 0);
    }

    write_i64(value: bigint): this {
        return this.write_u64(value);
    }

    write_bytes(bytes: Uint8Array | number[]): this {
        this.ensure_capacity(bytes.length);
        if (bytes instanceof Uint8Array) {
            this.buffer.set(bytes, this.position);
        } else {
            for (let i = 0; i < bytes.length; i++) {
                this.buffer[this.position + i] = bytes[i] & 0xff;
            }
        }
        this.position += bytes.length;
        return this;
    }

    write_cstring(str: string): this {
        const encoder = new TextEncoder();
        const bytes = encoder.encode(str);
        this.write_bytes(bytes);
        this.write_u8(0); // null terminator
        return this;
    }

    write_string(str: string, length: number): this {
        const encoder = new TextEncoder();
        const bytes = encoder.encode(str);

        if (bytes.length > length) {
            this.write_bytes(bytes.slice(0, length));
        } else {
            this.write_bytes(bytes);
            // Pad with zeros
            for (let i = bytes.length; i < length; i++) {
                this.write_u8(0);
            }
        }
        return this;
    }

    align(alignment: number, padding_byte: number = 0): this {
        const aligned_position = Math.ceil(this.position / alignment) * alignment;
        while (this.position < aligned_position) {
            this.write_u8(padding_byte);
        }
        return this;
    }

    get_position(): number {
        return this.position;
    }

    set_position(position: number): this {
        if (position < 0 || position > this.buffer.length) {
            throw new Error(`Invalid position: ${position}`);
        }
        this.position = position;
        return this;
    }

    get_bytes(): Uint8Array {
        return this.buffer.slice(0, this.position);
    }

    get_bytes_at(offset: number, length: number): Uint8Array {
        if (offset < 0 || offset + length > this.position) {
            throw new Error(`Invalid offset or length: offset=${offset}, length=${length}`);
        }
        return this.buffer.slice(offset, offset + length);
    }

    clear(): this {
        this.position = 0;
        return this;
    }
}