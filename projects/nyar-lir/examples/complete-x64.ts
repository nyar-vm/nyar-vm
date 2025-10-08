import { PeAssembler } from '../src/pe/PeAssembler';
import { writeFileSync } from 'fs';

console.log('=== Complete x64 PE Generator ===\n');

console.log('Creating complete x64 executable...');

const pe_builder = new PeAssembler('x64');

// 创建 .text 节
const text_section = pe_builder.add_section('.text', 0x60000020);

// 简单的 x64 代码 - 直接返回 0
const code = new Uint8Array([
    0x48, 0x31, 0xC0,       // xor rax, rax (return 0)
    0xC3                    // ret
]);

text_section.set_raw_data(code);

// 生成 PE 文件
const pe_data = pe_builder.build();

// 保存到文件
const filename = 'complete_x64.exe';
writeFileSync(filename, pe_data);

console.log(`Complete x64 executable created: ${filename}`);
console.log(`File size: ${pe_data.length} bytes`);

console.log(`\nCreated: ${filename}`);
console.log('This is a simple x64 PE executable that returns 0');