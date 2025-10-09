import * as fs from 'fs';

/**
 * Simple PE file validator to check basic structure
 */
class PeValidator {
    private buffer: Buffer;
    
    constructor(filePath: string) {
        this.buffer = fs.readFileSync(filePath);
    }
    
    validate(): void {
        console.log('=== PE File Validation ===');
        console.log(`File size: ${this.buffer.length} bytes`);
        
        // Check DOS header
        this.validateDosHeader();
        
        // Check NT headers
        this.validateNtHeaders();
        
        // Check sections
        this.validateSections();
    }
    
    private validateDosHeader(): void {
        console.log('\n--- DOS Header ---');
        
        // Check MZ signature
        const mzSignature = this.buffer.readUInt16LE(0);
        console.log(`MZ signature: 0x${mzSignature.toString(16)} (${mzSignature === 0x5A4D ? 'OK' : 'INVALID'})`);
        
        // Get e_lfanew (offset to NT headers)
        const e_lfanew = this.buffer.readUInt32LE(0x3C);
        console.log(`e_lfanew: 0x${e_lfanew.toString(16)} (${e_lfanew})`);
        
        if (e_lfanew >= this.buffer.length) {
            console.log('ERROR: e_lfanew points beyond file end');
            return;
        }
    }
    
    private validateNtHeaders(): void {
        console.log('\n--- NT Headers ---');
        
        const e_lfanew = this.buffer.readUInt32LE(0x3C);
        
        // Check PE signature
        const peSignature = this.buffer.readUInt32LE(e_lfanew);
        console.log(`PE signature: 0x${peSignature.toString(16)} (${peSignature === 0x00004550 ? 'OK' : 'INVALID'})`);
        
        // Check machine type
        const machine = this.buffer.readUInt16LE(e_lfanew + 4);
        console.log(`Machine: 0x${machine.toString(16)} (${machine === 0x8664 ? 'x64' : 'other'})`);
        
        // Check number of sections
        const numberOfSections = this.buffer.readUInt16LE(e_lfanew + 6);
        console.log(`Number of sections: ${numberOfSections}`);
        
        // Check optional header size
        const sizeOfOptionalHeader = this.buffer.readUInt16LE(e_lfanew + 20);
        console.log(`Size of optional header: ${sizeOfOptionalHeader} (expected: 240 for PE32+)`);
        
        // Check characteristics
        const characteristics = this.buffer.readUInt16LE(e_lfanew + 22);
        console.log(`Characteristics: 0x${characteristics.toString(16)}`);
        
        // Check optional header magic
        const magic = this.buffer.readUInt16LE(e_lfanew + 24);
        console.log(`Magic: 0x${magic.toString(16)} (${magic === 0x020B ? 'PE32+' : 'other'})`);
        
        // Check entry point
        const entryPoint = this.buffer.readUInt32LE(e_lfanew + 40);
        console.log(`Entry point RVA: 0x${entryPoint.toString(16)}`);
        
        // Check image base
        const imageBase = this.buffer.readBigUInt64LE(e_lfanew + 48);
        console.log(`Image base: 0x${imageBase.toString(16)}`);
        
        // Check section alignment
        const sectionAlignment = this.buffer.readUInt32LE(e_lfanew + 56);
        console.log(`Section alignment: 0x${sectionAlignment.toString(16)}`);
        
        // Check file alignment
        const fileAlignment = this.buffer.readUInt32LE(e_lfanew + 60);
        console.log(`File alignment: 0x${fileAlignment.toString(16)}`);
        
        // Check size of image
        const sizeOfImage = this.buffer.readUInt32LE(e_lfanew + 80);
        console.log(`Size of image: 0x${sizeOfImage.toString(16)}`);
        
        // Check size of headers
        const sizeOfHeaders = this.buffer.readUInt32LE(e_lfanew + 84);
        console.log(`Size of headers: 0x${sizeOfHeaders.toString(16)}`);
        
        // Check subsystem
        const subsystem = this.buffer.readUInt16LE(e_lfanew + 92);
        console.log(`Subsystem: ${subsystem} (${subsystem === 3 ? 'CONSOLE' : 'other'})`);
        
        // Check DLL characteristics
        const dllCharacteristics = this.buffer.readUInt16LE(e_lfanew + 94);
        console.log(`DLL characteristics: 0x${dllCharacteristics.toString(16)}`);
    }
    
    private validateSections(): void {
        console.log('\n--- Sections ---');
        
        const e_lfanew = this.buffer.readUInt32LE(0x3C);
        const numberOfSections = this.buffer.readUInt16LE(e_lfanew + 6);
        const sizeOfOptionalHeader = this.buffer.readUInt16LE(e_lfanew + 20);
        
        // Section headers start after NT headers + optional header
        const sectionHeadersOffset = e_lfanew + 24 + sizeOfOptionalHeader;
        
        for (let i = 0; i < numberOfSections; i++) {
            const sectionOffset = sectionHeadersOffset + (i * 40);
            
            // Read section name (8 bytes)
            const nameBytes = this.buffer.subarray(sectionOffset, sectionOffset + 8);
            const name = nameBytes.toString('ascii').replace(/\0/g, '');
            
            const virtualSize = this.buffer.readUInt32LE(sectionOffset + 8);
            const virtualAddress = this.buffer.readUInt32LE(sectionOffset + 12);
            const sizeOfRawData = this.buffer.readUInt32LE(sectionOffset + 16);
            const pointerToRawData = this.buffer.readUInt32LE(sectionOffset + 20);
            const characteristics = this.buffer.readUInt32LE(sectionOffset + 36);
            
            console.log(`Section ${i + 1}: ${name}`);
            console.log(`  Virtual size: 0x${virtualSize.toString(16)}`);
            console.log(`  Virtual address: 0x${virtualAddress.toString(16)}`);
            console.log(`  Size of raw data: 0x${sizeOfRawData.toString(16)}`);
            console.log(`  Pointer to raw data: 0x${pointerToRawData.toString(16)}`);
            console.log(`  Characteristics: 0x${characteristics.toString(16)}`);
            
            // Validate section data exists
            if (pointerToRawData + sizeOfRawData > this.buffer.length) {
                console.log(`  ERROR: Section data extends beyond file end`);
            }
        }
    }
}

// Validate the minimal.exe file
const validator = new PeValidator('minimal.exe');
validator.validate();