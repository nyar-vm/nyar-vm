import { PeAssembler } from '../src/pe/PeAssembler';
import { writeFileSync } from 'fs';

console.log('=== Simple x86 PE Generator ===\n');

console.log('Creating simple x86 executable...');

const pe_builder = new PeAssembler('x86');

// 创建 .text 节
const text_section = pe_builder.add_section('.text', 0x60000020);

// 非常简单的 x86 代码 - 只是返回 0
const simple_code = new Uint8Array([
    0x55,                         // push ebp
    0x8B, 0xEC,                   // mov ebp, esp
    0x31, 0xC0,                   // xor eax, eax (return 0)
    0x5D,                         // pop ebp
    0xC3                          // ret
]);

text_section.set_raw_data(simple_code);

// 生成 PE 文件
const pe_data = pe_builder.build();

// 保存到文件
const filename = 'simple_x86_direct.exe';
writeFileSync(filename, pe_data);

console.log(`Simple x86 executable created: ${filename}`);
console.log(`File size: ${pe_data.length} bytes`);

console.log(`\nCreated: ${filename}`);
console.log('This should be a valid x86 PE executable that returns 0');