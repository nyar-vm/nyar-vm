import {PeAssembler} from '../src/pe/PeAssembler';
import {writeFileSync} from 'fs';

// 创建一个简单的 x64 可执行文件
function createSimpleX64Exe() {
    console.log('Creating simple x64 executable...');

    const pe_builder = new PeAssembler('x64');

    // 创建 .text 节
    const text_section = pe_builder.add_section('.text', 0x60000020);

    // 非常简单的 x64 代码 - 只是返回 0
    const simple_code = new Uint8Array([
        0x55,                         // push rbp
        0x48, 0x8B, 0xEC,             // mov rbp, rsp
        0x48, 0x31, 0xC0,             // xor rax, rax (return 0)
        0x5D,                         // pop rbp
        0xC3                          // ret
    ]);

    text_section.set_raw_data(simple_code);

    // 生成 PE 文件
    const pe_data = pe_builder.build();

    // 保存到文件
    const filename = 'simple_x64.exe';
    writeFileSync(filename, pe_data);

    console.log(`Simple x64 executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);

    return filename;
}

console.log('=== Simple x64 PE Generator ===\n');

const simple_file = createSimpleX64Exe();

console.log(`\nCreated: ${simple_file}`);
console.log('This should be a valid x64 PE executable that returns 0');
