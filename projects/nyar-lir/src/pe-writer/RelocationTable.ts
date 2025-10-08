import { BinaryWriter } from '@/BinaryWriter';

export class RelocationTable {
    generate_raw_data(): Uint8Array {
        // For an empty relocation table, we need at least one BASE_RELOCATION_BLOCK
        // with VirtualAddress = 0 and SizeOfBlock = 8 (header only).
        // This indicates no relocations are present.
        const writer = new BinaryWriter();
        writer.write_u32(0); // VirtualAddress
        writer.write_u32(8); // SizeOfBlock (header only)
        return writer.get_bytes();
    }
}