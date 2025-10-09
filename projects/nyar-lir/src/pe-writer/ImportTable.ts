import { PeWriter } from './PeAssembler';

/**
 * Import descriptor structure
 */
export interface ImportDescriptor {
    dll_name: string;
    functions: string[];
}

/**
 * PE Import Table for managing DLL imports
 */
export class ImportTable {
    private imports: ImportDescriptor[] = [];

    /**
     * Add a DLL import with its functions
     */
    add_import(dll_name: string, functions: string[]): void {
        this.imports.push({ dll_name, functions });
    }

    /**
     * Add kernel32.dll imports for basic Windows API
     */
    add_kernel32_imports(): void {
        this.add_import('kernel32.dll', [
            'ExitProcess',
            'GetStdHandle',
            'WriteConsoleA'
        ]);
    }

    /**
     * Generate import table data
     */
    generate_import_table(): Uint8Array {
        const writer = new PeWriter();
        
        // Calculate string table offset
        const descriptor_size = (this.imports.length + 1) * 20; // 20 bytes per descriptor + null terminator
        let string_table_offset = descriptor_size;
        
        // Calculate import lookup table offset
        let lookup_table_offset = string_table_offset;
        for (const imp of this.imports) {
            lookup_table_offset += imp.dll_name.length + 1; // +1 for null terminator
        }
        
        // Align to 8 bytes
        lookup_table_offset = Math.ceil(lookup_table_offset / 8) * 8;
        
        // Write import descriptors
        let current_lookup_offset = lookup_table_offset;
        let current_string_offset = string_table_offset;
        
        for (const imp of this.imports) {
            // Import Lookup Table RVA
            writer.write_u32(0x2000 + current_lookup_offset);
            // TimeDateStamp
            writer.write_u32(0);
            // ForwarderChain
            writer.write_u32(0);
            // Name RVA
            writer.write_u32(0x2000 + current_string_offset);
            // Import Address Table RVA (same as lookup table)
            writer.write_u32(0x2000 + current_lookup_offset);
            
            current_string_offset += imp.dll_name.length + 1;
            current_lookup_offset += (imp.functions.length + 1) * 8; // 8 bytes per entry + null terminator
        }
        
        // Null terminator descriptor
        for (let i = 0; i < 20; i++) {
            writer.write_u8(0);
        }
        
        // Write DLL names
        for (const imp of this.imports) {
            writer.write_cstring(imp.dll_name);
        }
        
        // Align to lookup table offset
        while (writer.get_position() < lookup_table_offset) {
            writer.write_u8(0);
        }
        
        // Write import lookup tables and function names
        let function_name_offset = current_lookup_offset;
        
        for (const imp of this.imports) {
            // Write function name RVAs
            for (const func of imp.functions) {
                writer.write_u64(BigInt(0x2000 + function_name_offset));
                function_name_offset += 2 + func.length + 1; // 2 bytes hint + name + null terminator
            }
            // Null terminator
            writer.write_u64(0n);
        }
        
        // Write function names with hints
        for (const imp of this.imports) {
            for (let i = 0; i < imp.functions.length; i++) {
                writer.write_u16(i); // Hint
                writer.write_cstring(imp.functions[i]);
            }
        }
        
        return writer.get_bytes();
    }

    /**
     * Get the size of the import table
     */
    get_size(): number {
        return this.generate_import_table().length;
    }

    /**
     * Check if there are any imports
     */
    has_imports(): boolean {
        return this.imports.length > 0;
    }
}