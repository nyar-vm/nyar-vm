import { PeWriter } from './PeAssembler';

/**
 * PE section characteristics flags
 */
export enum SectionCharacteristics {
    IMAGE_SCN_CNT_CODE = 0x00000020,
    IMAGE_SCN_CNT_INITIALIZED_DATA = 0x00000040,
    IMAGE_SCN_CNT_UNINITIALIZED_DATA = 0x00000080,
    IMAGE_SCN_MEM_EXECUTE = 0x20000000,
    IMAGE_SCN_MEM_READ = 0x40000000,
    IMAGE_SCN_MEM_WRITE = 0x80000000,
}

/**
 * PE file section
 */
export class PeSection {
    public name: string;
    public virtual_size: number;
    public virtual_address: number;
    public size_of_raw_data: number;
    public pointer_to_raw_data: number;
    public pointer_to_relocations: number;
    public pointer_to_line_numbers: number;
    public number_of_relocations: number;
    public number_of_line_numbers: number;
    public characteristics: number;
    public data: Uint8Array;

    constructor(
        name: string,
        virtual_address: number,
        characteristics: number,
        data: Uint8Array = new Uint8Array(0)
    ) {
        this.name = name.padEnd(8, '\0').substring(0, 8);
        this.virtual_size = data.length;
        this.virtual_address = virtual_address;
        this.size_of_raw_data = this.align_to_file_alignment(data.length);
        this.pointer_to_raw_data = 0; // Will be set when writing
        this.pointer_to_relocations = 0;
        this.pointer_to_line_numbers = 0;
        this.number_of_relocations = 0;
        this.number_of_line_numbers = 0;
        this.characteristics = characteristics;
        this.data = data;
    }

    /**
     * Align size to file alignment (512 bytes)
     */
    private align_to_file_alignment(size: number): number {
        const alignment = 0x200; // 512 bytes
        return Math.ceil(size / alignment) * alignment;
    }

    /**
     * Write section header to binary writer
     */
    write_section_header(writer: PeWriter): void {
        // Section name (8 bytes)
        const name_bytes = new TextEncoder().encode(this.name);
        const padded_name = new Uint8Array(8);
        padded_name.set(name_bytes.slice(0, 8));
        writer.write_bytes(padded_name);

        writer.write_u32(this.virtual_size);
        writer.write_u32(this.virtual_address);
        writer.write_u32(this.size_of_raw_data);
        writer.write_u32(this.pointer_to_raw_data);
        writer.write_u32(this.pointer_to_relocations);
        writer.write_u32(this.pointer_to_line_numbers);
        writer.write_u16(this.number_of_relocations);
        writer.write_u16(this.number_of_line_numbers);
        writer.write_u32(this.characteristics);
    }

    /**
     * Write section data to binary writer with proper alignment
     */
    write_section_data(writer: PeWriter): void {
        writer.write_bytes(this.data);
        
        // Pad to file alignment
        const padding = this.size_of_raw_data - this.data.length;
        for (let i = 0; i < padding; i++) {
            writer.write_u8(0);
        }
    }

    /**
     * Set the file offset where this section's data will be written
     */
    set_raw_data_pointer(offset: number): void {
        this.pointer_to_raw_data = offset;
    }

    /**
     * Get the size of this section when written to file (aligned)
     */
    get_file_size(): number {
        return this.size_of_raw_data;
    }

    /**
     * Get the size of this section in memory
     */
    get_virtual_size(): number {
        return this.virtual_size;
    }

    /**
     * Create a code section (.text)
     */
    static create_text_section(code: Uint8Array): PeSection {
        return new PeSection(
            '.text',
            0x1000, // Virtual address
            SectionCharacteristics.IMAGE_SCN_CNT_CODE |
            SectionCharacteristics.IMAGE_SCN_MEM_EXECUTE |
            SectionCharacteristics.IMAGE_SCN_MEM_READ,
            code
        );
    }

    /**
     * Create a data section (.rdata)
     */
    static create_rdata_section(data: Uint8Array): PeSection {
        return new PeSection(
            '.rdata',
            0x2000, // Virtual address
            SectionCharacteristics.IMAGE_SCN_CNT_INITIALIZED_DATA |
            SectionCharacteristics.IMAGE_SCN_MEM_READ,
            data
        );
    }

    /**
     * Create an import data section (.idata)
     */
    static create_idata_section(data: Uint8Array): PeSection {
        return new PeSection(
            '.idata',
            0x3000, // Virtual address
            SectionCharacteristics.IMAGE_SCN_CNT_INITIALIZED_DATA |
            SectionCharacteristics.IMAGE_SCN_MEM_READ |
            SectionCharacteristics.IMAGE_SCN_MEM_WRITE,
            data
        );
    }
}