import {PeSection} from './PeSection';
import {ImportTable} from './ImportTable';
import {PeTargetArchitecture} from './PeTargetArchitecture';
import {PeHeaders} from './PeHeaders';

export class PeAssembler {
    private architecture: PeTargetArchitecture;
    private pe_headers: PeHeaders;
    private sections: Map<string, PeSection>;
    private import_table: ImportTable;

    constructor(architecture: PeTargetArchitecture) {
        this.architecture = architecture;
        this.pe_headers = new PeHeaders();
        this.sections = new Map();
        this.import_table = new ImportTable();
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


export class PeReader {
    public read(input: Uint8Array | ArrayBuffer): PeAssembler {
        throw new Error('Not implemented');
    }
    private read_u8(view: DataView, offset: number): number {
        return view.getUint8(offset);
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

    public write(data: Uint8Array | File, pe: PeAssembler) {
       throw new Error('Not implemented');
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
