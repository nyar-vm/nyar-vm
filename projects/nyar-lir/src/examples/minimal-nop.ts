import { PeAssembler } from '../pe-writer/PeAssembler';
import { PeSection } from '../pe-writer/PeSection';
import { PeTargetArchitecture } from '../pe-writer/PeTargetArchitecture';
import { ImportTable } from '../pe-writer/ImportTable';
import { writeFileSync } from 'fs';

// Create PE assembler
const pe = new PeAssembler(PeTargetArchitecture.X64);

// Create Import Table
const import_table = new ImportTable();
import_table.add_import("kernel32.dll", ["ExitProcess"]);
pe.set_import_table(import_table);

// Minimal machine code - call ExitProcess(0)
const machine_code = new Uint8Array([
    0x48, 0x31, 0xC9,           // xor rcx, rcx (exit code = 0)
    // call qword ptr [rip + 0xFF7]
    // rip will be entry_point(0x1000) + instruction_offset(3) + instruction_length(6) = 0x1009
    // The address of the IAT entry for ExitProcess is at import_table_rva(0x2000)
    // So, rip + 0xFF7 = 0x1009 + 0xFF7 = 0x2000
    0xFF, 0x15, 0xF7, 0x0F, 0x00, 0x00,  // call [rip + 0xFF7]
]);

// Add .text section
const text_section = PeSection.create_text_section(machine_code);
pe.add_section(text_section);

// Add .idata section for the import table
const idata_section = PeSection.create_idata_section(new Uint8Array(0));
pe.add_section(idata_section);

// Build PE file
const pe_data = pe.build();

// Write to file
writeFileSync('minimal-nop.exe', pe_data);

console.log('Generated minimal-nop.exe');
console.log('PE file size:', pe_data.length);

// Hex dump first 512 bytes
const hex_dump = Array.from(pe_data.slice(0, 512))
    .map((b: number) => b.toString(16).padStart(2, '0'))
    .join(' ');
console.log('Hex dump (first 512 bytes):');
console.log(hex_dump);