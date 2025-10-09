import { PeAssembler } from '../pe-writer/PeAssembler';
import { PeTargetArchitecture } from '../pe-writer/PeTargetArchitecture';
import { ImportTable } from '../pe-writer/ImportTable';
import * as fs from 'fs';

/**
 * Generate x64 machine code for Hello World program
 * This code uses MessageBoxA to display "Hello, World!" and then ExitProcess
 * Much simpler than console output and easier to debug
 */
function generate_hello_world_code(): Uint8Array {
    // x64 assembly equivalent:
    // sub rsp, 40          ; Reserve stack space (shadow space + alignment)
    // mov rcx, 0           ; hWnd = NULL
    // lea rdx, [hello_msg] ; lpText = "Hello, World!"
    // lea r8, [title_msg]  ; lpCaption = "Hello"
    // mov r9, 0            ; uType = MB_OK
    // call MessageBoxA     ; Show message box
    // mov rcx, 0           ; Exit code = 0
    // call ExitProcess     ; Exit

    const code = new Uint8Array([
        // sub rsp, 40 (0x28) - Reserve shadow space
        0x48, 0x83, 0xEC, 0x28,
        
        // mov rcx, 0 (hWnd = NULL)
        0x48, 0x31, 0xC9,
        
        // lea rdx, [hello_msg] (message pointer - relative to current position)
        0x48, 0x8D, 0x15, 0x1C, 0x00, 0x00, 0x00, // lea rdx, [rip+0x1C]
        
        // lea r8, [title_msg] (title pointer)
        0x4C, 0x8D, 0x05, 0x20, 0x00, 0x00, 0x00, // lea r8, [rip+0x20]
        
        // mov r9, 0 (uType = MB_OK)
        0x49, 0x31, 0xC9,
        
        // call MessageBoxA - use absolute address from import table
        // IAT is at import_section_rva + 60, MessageBoxA is first entry (user32.dll)
        0xFF, 0x15, 0xCA, 0x1F, 0x00, 0x00, // call [0x203C] - MessageBoxA import
        
        // mov rcx, 0 (exit code)
        0x48, 0x31, 0xC9,
        
        // call ExitProcess - use absolute address from import table  
        // ExitProcess is second entry in IAT (kernel32.dll)
        0xFF, 0x15, 0xD4, 0x1F, 0x00, 0x00, // call [0x2044] - ExitProcess import
        
        // String data (aligned to current position)
        // "Hello, World!" message
        0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x2C, 0x20, 0x57,
        0x6F, 0x72, 0x6C, 0x64, 0x21, 0x00, // "Hello, World!\0"
        
        // "Hello" title
        0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x00, // "Hello\0"
        
        // Padding for alignment
        0x00, 0x00
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
    import_table.add_kernel32_imports(); // ExitProcess
    import_table.add_user32_imports();   // MessageBoxA
    
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