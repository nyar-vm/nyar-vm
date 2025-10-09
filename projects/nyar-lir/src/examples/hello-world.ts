import { PeAssembler } from '../pe-writer/PeAssembler';
import { PeTargetArchitecture } from '../pe-writer/PeTargetArchitecture';
import { ImportTable } from '../pe-writer/ImportTable';
import * as fs from 'fs';

/**
 * Generate x64 machine code for Hello World program
 * This code calls WriteConsoleA to output "Hello, World!" and then ExitProcess
 */
function generate_hello_world_code(): Uint8Array {
    // x64 assembly equivalent:
    // sub rsp, 40          ; Reserve stack space (shadow space + alignment)
    // mov rcx, -11         ; STD_OUTPUT_HANDLE
    // call GetStdHandle    ; Get stdout handle
    // mov rcx, rax         ; Console handle
    // lea rdx, [hello_msg] ; Message pointer
    // mov r8, 13           ; Message length
    // lea r9, [bytes_written] ; Bytes written pointer
    // mov qword ptr [rsp+32], 0 ; Reserved parameter
    // call WriteConsoleA   ; Write to console
    // mov rcx, 0           ; Exit code
    // call ExitProcess     ; Exit

    const code = new Uint8Array([
        // sub rsp, 40 (0x28)
        0x48, 0x83, 0xEC, 0x28,
        
        // mov rcx, -11 (STD_OUTPUT_HANDLE)
        0x48, 0xC7, 0xC1, 0xF5, 0xFF, 0xFF, 0xFF,
        
        // call GetStdHandle (placeholder - will be resolved by import table)
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, // call [GetStdHandle]
        
        // mov rcx, rax (console handle)
        0x48, 0x89, 0xC1,
        
        // lea rdx, [hello_msg] (message pointer - relative to RIP)
        0x48, 0x8D, 0x15, 0x20, 0x00, 0x00, 0x00, // lea rdx, [rip+0x20]
        
        // mov r8, 13 (message length)
        0x49, 0xC7, 0xC0, 0x0D, 0x00, 0x00, 0x00,
        
        // lea r9, [bytes_written] (bytes written pointer)
        0x4C, 0x8D, 0x0D, 0x30, 0x00, 0x00, 0x00, // lea r9, [rip+0x30]
        
        // mov qword ptr [rsp+32], 0 (reserved parameter)
        0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00,
        
        // call WriteConsoleA (placeholder - will be resolved by import table)
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, // call [WriteConsoleA]
        
        // mov rcx, 0 (exit code)
        0x48, 0x31, 0xC9,
        
        // call ExitProcess (placeholder - will be resolved by import table)
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, // call [ExitProcess]
        
        // Data section starts here (aligned)
        // "Hello, World!" string
        0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x2C, 0x20, 0x57,
        0x6F, 0x72, 0x6C, 0x64, 0x21, 0x00, // "Hello, World!\0"
        
        // Padding to align
        0x00, 0x00,
        
        // bytes_written variable (4 bytes)
        0x00, 0x00, 0x00, 0x00
    ]);

    return code;
}

/**
 * Create and build a Hello World PE executable
 */
function create_hello_world_pe(): Uint8Array {
    // Create PE assembler for x64 architecture
    const assembler = new PeAssembler(PeTargetArchitecture.X64);
    
    // Create import table with required Windows API functions
    const import_table = new ImportTable();
    import_table.add_kernel32_imports(); // This adds GetStdHandle, WriteConsoleA, ExitProcess
    
    // Set the import table
    assembler.set_import_table(import_table);
    
    // Generate and add the machine code
    const code = generate_hello_world_code();
    assembler.add_code_section(code);
    
    // Build the PE file
    return assembler.build();
}

/**
 * Main function to generate Hello World executable
 */
function main() {
    try {
        console.log('Generating Hello World PE executable...');
        
        // Create the PE file
        const pe_data = create_hello_world_pe();
        
        // Write to file
        const output_path = 'hello-world.exe';
        fs.writeFileSync(output_path, pe_data);
        
        console.log(`Hello World executable generated: ${output_path}`);
        console.log(`File size: ${pe_data.length} bytes`);
        
        // Display some basic info
        console.log('\nTo run the executable:');
        console.log(`  ./${output_path}`);
        console.log('\nExpected output:');
        console.log('  Hello, World!');
        
    } catch (error) {
        console.error('Error generating PE executable:', error);
        process.exit(1);
    }
}

// Run if this file is executed directly
if (require.main === module) {
    main();
}

export { create_hello_world_pe, generate_hello_world_code };