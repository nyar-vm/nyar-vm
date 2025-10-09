import { PeAssembler } from '../pe-writer/PeAssembler';
import { PeTargetArchitecture } from '../pe-writer/PeTargetArchitecture';
import { ImportTable } from '../pe-writer/ImportTable';
import { PeSection } from '../pe-writer/PeSection';
import * as fs from 'fs';
import { RelocationTable } from '../pe-writer/RelocationTable';

/**
 * Generate x64 machine code for Hello World program
 * This code uses MessageBoxA to display "Hello, World!" and then ExitProcess
 * Much simpler than console output and easier to debug
 */
function generate_hello_world_code(): { code: Uint8Array, relocations: RelocationTable, symbols: Map<string, number> } {
    // x64 assembly equivalent:
    // sub rsp, 40          ; Reserve stack space (shadow space + alignment)
    // mov rcx, 0           ; hWnd = NULL
    // lea rdx, [hello_msg] ; lpText = "Hello, World!"
    // lea r8, [title_msg]  ; lpCaption = "Hello"
    // mov r9, 0            ; uType = MB_OK
    // call MessageBoxA     ; Show message box
    // mov rcx, 0           ; Exit code = 0
    // call ExitProcess     ; Exit

    const code_section_bytes = [
        // sub rsp, 40 (0x28)
        0x48, 0x83, 0xEC, 0x28,
        // mov rcx, 0
        0x48, 0x31, 0xC9,
        // lea rdx, [rip+hello_msg]
        0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00,
        // lea r8, [rip+title_msg]
        0x4C, 0x8D, 0x05, 0x00, 0x00, 0x00, 0x00,
        // mov r9, 0
        0x41, 0xB9, 0x00, 0x00, 0x00, 0x00, // mov r9d, 0
        // call MessageBoxA
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00,
        // mov rcx, 0
        0x48, 0x31, 0xC9,
        // call ExitProcess
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00,
    ];

    const hello_msg = "Hello, World!\0".split('').map(c => c.charCodeAt(0));
    const title_msg = "Hello\0".split('').map(c => c.charCodeAt(0));

    const hello_msg_offset = code_section_bytes.length;
    const title_msg_offset = hello_msg_offset + hello_msg.length;

    const final_code = new Uint8Array([
        ...code_section_bytes,
        ...hello_msg,
        ...title_msg,
    ]);

    const relocations = [
        { offset: 10, type: 'rip_relative', symbol: 'hello_msg' },
        { offset: 17, type: 'rip_relative', symbol: 'title_msg' },
        { offset: 26, type: 'rip_relative_import', symbol: 'MessageBoxA' },
        { offset: 35, type: 'rip_relative_import', symbol: 'ExitProcess' },
    ];

    const symbols = new Map<string, number>([
        ['hello_msg', hello_msg_offset],
        ['title_msg', title_msg_offset],
    ]);

    return { code: final_code, relocations, symbols };
}

/**
 * Create and build a Hello World PE executable
 */
function create_hello_world_pe(): Uint8Array {
    const assembler = new PeAssembler(PeTargetArchitecture.X64);
    
    const import_table = new ImportTable();
    import_table.add_import("kernel32.dll", ["ExitProcess"]);
    import_table.add_import("user32.dll", ["MessageBoxA"]);
    
    assembler.set_import_table(import_table);
    
    const { code, relocations, symbols } = generate_hello_world_code();
    const text_section = PeSection.create_text_section(code);
    assembler.add_section(text_section);

    assembler.add_relocations(text_section, relocations, symbols);
    
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