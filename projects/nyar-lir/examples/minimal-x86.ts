import { PeAssembler } from '../src/./pe-writer/PeAssembler';
import { writeFileSync } from 'fs';

console.log('=== Minimal x86 PE Generator ===\n');

console.log('Creating minimal x86 executable...');

const pe_builder = new PeAssembler('x86');

// 创建 .text 节
const text_section = pe_builder.add_section('.text', 0x60000020);

// 最简单的 x86 代码 - 直接调用 ExitProcess(0)
const simple_code = new Uint8Array([
    0xB8, 0x00, 0x00, 0x00, 0x00,   // mov eax, 0
    0xC3                          // ret
]);

text_section.set_raw_data(simple_code);

// 生成 PE 文件
const pe_data = pe_builder.build();

// 保存到文件
const filename = 'minimal_x86.exe';
writeFileSync(filename, pe_data);

console.log(`Minimal x86 executable created: ${filename}`);
console.log(`File size: ${pe_data.length} bytes`);

console.log(`\nCreated: ${filename}`);
console.log('This should be a minimal x86 PE executable that returns 0');