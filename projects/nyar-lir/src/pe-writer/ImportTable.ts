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
            // 'GetStdHandle',
            // 'WriteConsoleA',
        ]);
    }

    /**
     * Add user32.dll imports for MessageBox
     */
    add_user32_imports(): void {
        this.add_import('user32.dll', [
            'MessageBoxA'
        ]);
    }

    /**
     * Generate import table data with correct RVA calculations
     */
    generate_import_table(base_rva: number = 0): Uint8Array {
        const writer = new PeWriter();
        
        // Import table structure:
        // 1. Import descriptors (20 bytes each + null terminator)
        // 2. Import Address Table (IAT) - 8 bytes per function + null terminator per DLL
        // 3. DLL names (null-terminated strings)
        // 4. Function names with hints (2-byte hint + null-terminated string)
        
        const descriptor_count = this.imports.length;
        const descriptor_size = (descriptor_count + 1) * 20; // +1 for null terminator
        
        // Calculate IAT offset (right after descriptors)
        let iat_offset = descriptor_size;
        
        // Calculate total IAT size
        let total_iat_size = 0;
        for (const imp of this.imports) {
            total_iat_size += (imp.functions.length + 1) * 8; // +1 for null terminator
        }
        
        // Calculate string table offset (after IAT)
        let string_table_offset = iat_offset + total_iat_size;
        
        // Calculate function names offset (after DLL names)
        let function_names_offset = string_table_offset;
        for (const imp of this.imports) {
            function_names_offset += imp.dll_name.length + 1;
        }
        
        // Write import descriptors
        let current_iat_offset = iat_offset;
        let current_string_offset = string_table_offset;
        
        for (const imp of this.imports) {
            // Import Lookup Table RVA (same as IAT for simplicity)
            writer.write_u32(base_rva + current_iat_offset);
            // TimeDateStamp
            writer.write_u32(0);
            // ForwarderChain
            writer.write_u32(0);
            // Name RVA
            writer.write_u32(base_rva + current_string_offset);
            // Import Address Table RVA
            writer.write_u32(base_rva + current_iat_offset);
            
            current_string_offset += imp.dll_name.length + 1;
            current_iat_offset += (imp.functions.length + 1) * 8;
        }
        
        // Null terminator descriptor
        for (let i = 0; i < 20; i++) {
            writer.write_u8(0);
        }
        
        // Write Import Address Table (IAT)
        let current_function_name_offset = function_names_offset;
        
        for (const imp of this.imports) {
            for (const func of imp.functions) {
                // Write RVA to function name (with hint)
                writer.write_u64(BigInt(base_rva + current_function_name_offset));
                current_function_name_offset += 2 + func.length + 1; // 2 bytes hint + name + null
            }
            // Null terminator for this DLL
            writer.write_u64(0n);
        }
        
        // Write DLL names
        for (const imp of this.imports) {
            writer.write_cstring(imp.dll_name);
        }
        
        // Write function names with hints
        for (const imp of this.imports) {
            for (let i = 0; i < imp.functions.length; i++) {
                writer.write_u16(i); // Hint (ordinal)
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

    /**
     * Get the RVA of a function's entry in the Import Address Table (IAT)
     * relative to the start of the import table data.
     * @param function_name The name of the function to find.
     * @returns The relative RVA, or undefined if not found.
     */
    get_import_rva(function_name: string): number | undefined {
        const num_descriptors = this.imports.length + 1; // +1 for null terminator
        const iat_start_rva = num_descriptors * 20; // RVA relative to start of .idata section data
        let iat_entry_index = 0;

        for (const descriptor of this.imports) {
            for (const func of descriptor.functions) {
                if (func === function_name) {
                    // RVA is start of IAT + index * size_of_entry
                    return iat_start_rva + (iat_entry_index * 8);
                }
                iat_entry_index++;
            }
        }

        return undefined;
    }
}