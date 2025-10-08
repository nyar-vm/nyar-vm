import { writeFileSync } from 'fs';

function createHelloWorldPE() {
    const peData = [];
    
    // DOS Header (64 bytes)
    const dosHeader = new Uint8Array(64);
    // Magic number "MZ"
    dosHeader[0] = 0x4D; // M
    dosHeader[1] = 0x5A; // Z
    // Pointer to PE header (at offset 0x80)
    dosHeader[60] = 0x80;
    dosHeader[61] = 0x00;
    peData.push(...dosHeader);
    
    // DOS Stub (56 bytes to reach offset 0x80)
    const dosStub = new Uint8Array(56);
    // Simple DOS program that just exits
    // This is a minimal DOS program that does nothing
    peData.push(...dosStub);
    
    // PE Header at offset 0x80
    // PE Signature (4 bytes)
    peData.push(0x50, 0x45, 0x00, 0x00); // "PE\0\0"
    
    // COFF Header (20 bytes)
    // Machine type: x86 (0x14C)
    peData.push(0x4C, 0x01, 0x00, 0x00);
    // Number of sections: 2 (.text and .data)
    peData.push(0x02, 0x00);
    // Time date stamp
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Pointer to symbol table
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Number of symbols
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Size of optional header
    peData.push(0xE0, 0x00); // 224 bytes
    // Characteristics: executable, 32-bit
    peData.push(0x02, 0x01);
    
    // Optional Header (224 bytes)
    // Magic number: PE32 (0x10B)
    peData.push(0x0B, 0x01);
    // Major linker version
    peData.push(0x02);
    // Minor linker version
    peData.push(0x19);
    // Size of code
    peData.push(0x00, 0x10, 0x00, 0x00);
    // Size of initialized data
    peData.push(0x00, 0x10, 0x00, 0x00);
    // Size of uninitialized data
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Address of entry point
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // Base of code
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // Base of data
    peData.push(0x00, 0x20, 0x00, 0x00); // 0x2000
    // Image base
    peData.push(0x00, 0x00, 0x40, 0x00); // 0x400000
    // Section alignment
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // File alignment
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Major OS version
    peData.push(0x04, 0x00);
    // Minor OS version
    peData.push(0x00, 0x00);
    // Major image version
    peData.push(0x00, 0x00);
    // Minor image version
    peData.push(0x00, 0x00);
    // Major subsystem version
    peData.push(0x04, 0x00);
    // Minor subsystem version
    peData.push(0x00, 0x00);
    // Reserved
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Size of image
    peData.push(0x00, 0x30, 0x00, 0x00); // 0x3000
    // Size of headers
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Checksum
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Subsystem: Windows console (0x03)
    peData.push(0x03, 0x00);
    // DLL characteristics
    peData.push(0x00, 0x00);
    // Size of stack reserve
    peData.push(0x00, 0x00, 0x10, 0x00);
    // Size of stack commit
    peData.push(0x00, 0x10, 0x00, 0x00);
    // Size of heap reserve
    peData.push(0x00, 0x00, 0x10, 0x00);
    // Size of heap commit
    peData.push(0x00, 0x10, 0x00, 0x00);
    // Loader flags
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Number of data directories
    peData.push(0x10, 0x00, 0x00, 0x00);
    
    // Data directories (16 entries × 8 bytes = 128 bytes)
    // Export table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Import table
    peData.push(0x00, 0x20, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00); // RVA 0x2000, size 0x20
    // Resource table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Exception table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Certificate table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Base relocation table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Debug data
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Architecture-specific data
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Global pointer register
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Thread local storage table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Load configuration table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Bound import table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Import address table
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Delay import descriptor
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // COM+ runtime header
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Reserved
    peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    
    // Section headers start at offset 0x178
    // .text section header (40 bytes)
    // Section name: ".text\0\0\0\0"
    peData.push(0x2E, 0x74, 0x65, 0x78, 0x74, 0x00, 0x00, 0x00);
    // Virtual size
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // Virtual address
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // Size of raw data
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Pointer to raw data
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Pointer to relocations
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Pointer to line numbers
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Number of relocations
    peData.push(0x00, 0x00);
    // Number of line numbers
    peData.push(0x00, 0x00);
    // Characteristics: code, executable, readable
    peData.push(0x20, 0x00, 0x00, 0x60);
    
    // .data section header (40 bytes)
    // Section name: ".data\0\0\0\0"
    peData.push(0x2E, 0x64, 0x61, 0x74, 0x61, 0x00, 0x00, 0x00);
    // Virtual size
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // Virtual address
    peData.push(0x00, 0x20, 0x00, 0x00); // 0x2000
    // Size of raw data
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Pointer to raw data
    peData.push(0x00, 0x04, 0x00, 0x00); // 0x400
    // Pointer to relocations
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Pointer to line numbers
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Number of relocations
    peData.push(0x00, 0x00);
    // Number of line numbers
    peData.push(0x00, 0x00);
    // Characteristics: initialized data, readable, writable
    peData.push(0x40, 0x00, 0x00, 0xC0);
    
    // Padding to reach file alignment
    while (peData.length < 0x200) {
        peData.push(0x00);
    }
    
    // .text section data at offset 0x200
    // Simple x86 assembly that calls ExitProcess(0)
    const textData = [
        0x6A, 0x00,                         // push 0 (exit code)
        0xFF, 0x15, 0x00, 0x20, 0x00, 0x00, // call [0x2000] (ExitProcess)
        0xC3                                // ret
    ];
    
    // Add .text section data
    peData.push(...textData);
    
    // Padding to reach next section alignment
    while (peData.length < 0x400) {
        peData.push(0x00);
    }
    
    // .data section data at offset 0x400
    // Import address table and string data
    const dataData = [
        // IAT entry for ExitProcess (8 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // Import table (20 bytes)
        0x00, 0x00, 0x00, 0x00, // OriginalFirstThunk
        0x00, 0x00, 0x00, 0x00, // TimeDateStamp
        0x00, 0x00, 0x00, 0x00, // ForwarderChain
        0x10, 0x20, 0x00, 0x00, // Name (RVA 0x2010)
        0x00, 0x20, 0x00, 0x00, // FirstThunk (RVA 0x2000)
        // DLL name "kernel32.dll\0"
        0x6B, 0x65, 0x72, 0x6E, 0x65, 0x6C, 0x33, 0x32,
        0x2E, 0x64, 0x6C, 0x6C, 0x00
    ];
    
    // Add .data section data
    peData.push(...dataData);
    
    // Create final buffer
    const peBuffer = Buffer.from(peData);
    
    return peBuffer;
}

function main() {
    try {
        console.log('=== Hello World PE Generator ===');
        
        const peData = createHelloWorldPE();
        const filename = 'hello_world.exe';
        
        writeFileSync(filename, peData);
        
        console.log(`Hello World PE executable created: ${filename}`);
        console.log(`File size: ${peData.length} bytes`);
        console.log('\nInstructions:');
        console.log('1. The file "hello_world.exe" has been created');
        console.log('2. This should be a valid PE executable for x86 Windows');
        console.log('3. Try running it to see if it executes without errors');
        console.log('4. Note: This version exits cleanly but doesn\'t print "Hello World" yet');
        console.log('5. To print text, we would need to import and call Windows API functions');
        
    } catch (error) {
        console.error('Error creating Hello World PE:', error.message);
        process.exit(1);
    }
}

main();