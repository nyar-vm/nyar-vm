import { writeFileSync } from 'fs';

function createX64PE() {
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
    peData.push(...dosStub);
    
    // PE Header at offset 0x80
    // PE Signature (4 bytes)
    peData.push(0x50, 0x45, 0x00, 0x00); // "PE\0\0"
    
    // COFF Header (20 bytes) - 64-bit version
    // Machine type: x64 (0x8664)
    peData.push(0x64, 0x86, 0x00, 0x00);
    // Number of sections: 1 (.text)
    peData.push(0x01, 0x00);
    // Time date stamp
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Pointer to symbol table
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Number of symbols
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Size of optional header
    peData.push(0xF0, 0x00); // 240 bytes for PE32+
    // Characteristics: executable, large address aware
    peData.push(0x22, 0x00);
    
    // Optional Header (240 bytes) - PE32+ format
    // Magic number: PE32+ (0x20B)
    peData.push(0x0B, 0x02);
    // Major linker version
    peData.push(0x02);
    // Minor linker version
    peData.push(0x19);
    // Size of code
    peData.push(0x00, 0x10, 0x00, 0x00);
    // Size of initialized data
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Size of uninitialized data
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Address of entry point
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // Base of code
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    
    // Image base (8 bytes for PE32+)
    peData.push(0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00); // 0x400000
    
    // Section alignment
    peData.push(0x00, 0x10, 0x00, 0x00); // 0x1000
    // File alignment
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Major OS version
    peData.push(0x06, 0x00);
    // Minor OS version
    peData.push(0x00, 0x00);
    // Major image version
    peData.push(0x00, 0x00);
    // Minor image version
    peData.push(0x00, 0x00);
    // Major subsystem version
    peData.push(0x06, 0x00);
    // Minor subsystem version
    peData.push(0x00, 0x00);
    // Reserved
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Size of image
    peData.push(0x00, 0x20, 0x00, 0x00); // 0x2000
    // Size of headers
    peData.push(0x00, 0x02, 0x00, 0x00); // 0x200
    // Checksum
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Subsystem: Windows console (0x03)
    peData.push(0x03, 0x00);
    // DLL characteristics
    peData.push(0x00, 0x00);
    
    // Size of stack reserve (8 bytes)
    peData.push(0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Size of stack commit (8 bytes)
    peData.push(0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Size of heap reserve (8 bytes)
    peData.push(0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00);
    // Size of heap commit (8 bytes)
    peData.push(0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00);
    
    // Loader flags
    peData.push(0x00, 0x00, 0x00, 0x00);
    // Number of data directories
    peData.push(0x10, 0x00, 0x00, 0x00);
    
    // Data directories (16 entries × 8 bytes = 128 bytes)
    // All directories empty for now
    for (let i = 0; i < 16; i++) {
        peData.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);
    }
    
    // Section header start at offset 0x108
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
    
    // Padding to reach file alignment (0x200)
    while (peData.length < 0x200) {
        peData.push(0x00);
    }
    
    // .text section data at offset 0x200
    // Simple x64 assembly that exits with code 0
    // This is a minimal x64 program that just returns 0
    const textData = [
        0x48, 0x31, 0xC0,       // xor rax, rax (return 0)
        0xC3                    // ret
    ];
    
    // Add .text section data
    peData.push(...textData);
    
    // Create final buffer
    const peBuffer = Buffer.from(peData);
    
    return peBuffer;
}

function main() {
    try {
        console.log('=== x64 PE Generator ===');
        
        const peData = createX64PE();
        const filename = 'x64_hello.exe';
        
        writeFileSync(filename, peData);
        
        console.log(`x64 PE executable created: ${filename}`);
        console.log(`File size: ${peData.length} bytes`);
        console.log('\nInstructions:');
        console.log('1. The file "x64_hello.exe" has been created');
        console.log('2. This is a 64-bit PE executable for Windows x64');
        console.log('3. Try running it to see if it executes without errors');
        console.log('4. It should exit cleanly with code 0');
        
    } catch (error) {
        console.error('Error creating x64 PE:', error.message);
        process.exit(1);
    }
}

main();