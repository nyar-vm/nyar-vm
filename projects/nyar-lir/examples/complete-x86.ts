import { PeAssembler } from '../src/pe/PeBuilder';
import { writeFileSync } from 'fs';

console.log('=== Complete x86 PE Generator ===\n');

console.log('Creating complete x86 executable...');

const pe_builder = new PeAssembler('x86');

// 创建 .text 节
const text_section = pe_builder.add_section('.text', 0x60000020);

// 简单的 x86 代码 - 直接返回 0
const code = new Uint8Array([
    0x31, 0xC0,             // xor eax, eax (return 0)
    0xC3                    // ret
]);

text_section.set_raw_data(code);

// 生成 PE 文件
const pe_data = pe_builder.build();

// 保存到文件
const filename = 'complete_x86.exe';
writeFileSync(filename, pe_data);

console.log(`Complete x86 executable created: ${filename}`);
console.log(`File size: ${pe_data.length} bytes`);

console.log(`\nCreated: ${filename}`);
console.log('This is a simple x86 PE executable that returns 0');