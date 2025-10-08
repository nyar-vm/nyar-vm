import { writeFileSync } from 'fs';
import {PeAssembler} from "../src/pe/PeBuilder";

// 创建一个简单的 Hello World PE 可执行文件
function createHelloWorldExe() {
    console.log('Creating Hello World PE executable...');
    
    const pe_builder = new PeAssembler('x86');
    
    // 创建 .text 节（代码段）
    const text_section = pe_builder.add_section('.text', 0x60000020);
    
    // 简化的 x86 汇编代码，调用 Windows API
    // 这是一个非常简化的示例，实际使用时需要完整的导入表和重定位
    const hello_code = new Uint8Array([
        // 简化的程序入口点
        0x55,                         // push ebp
        0x8B, 0xEC,                   // mov ebp, esp
        0x6A, 0x00,                   // push 0 (MB_OK)
        0x68, 0x00, 0x00, 0x00, 0x00, // push offset title (需要重定位)
        0x68, 0x00, 0x00, 0x00, 0x00, // push offset message (需要重定位)
        0xE8, 0x00, 0x00, 0x00, 0x00, // call MessageBoxA (需要重定位)
        0x6A, 0x00,                   // push 0
        0xE8, 0x00, 0x00, 0x00, 0x00, // call ExitProcess (需要重定位)
        0x5D,                         // pop ebp
        0xC3,                         // ret
        
        // 字符串数据
        0x48, 0x65, 0x6C, 0x6C, 0x6F, // "Hello"
        0x20, 0x57, 0x6F, 0x72, 0x6C, // " Wor"
        0x64, 0x21, 0x00,             // "d!\0"
        0x4D, 0x79, 0x20, 0x41, 0x70, // "My Ap"
        0x70, 0x00                    // "p\0"
    ]);
    
    text_section.set_raw_data(hello_code);
    
    // 创建 .data 节（数据段）
    const data_section = pe_builder.add_section('.data', 0xc0000040);
    const data = new Uint8Array([
        0x48, 0x65, 0x6C, 0x6C, 0x6F, // "Hello"
        0x20, 0x57, 0x6F, 0x72, 0x6C, // " Wor"
        0x64, 0x21, 0x00,             // "d!\0"
        0x00, 0x00, 0x00, 0x00,       // padding
        0x4D, 0x79, 0x20, 0x41, 0x70, // "My Ap"
        0x70, 0x00                    // "p\0"
    ]);
    data_section.set_raw_data(data);
    
    // 生成 PE 文件
    console.log('Building PE file...');
    const pe_data = pe_builder.build();
    
    // 保存到文件
    const filename = 'hello_world.exe';
    writeFileSync(filename, pe_data);
    
    console.log(`PE executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);
    
    // 验证 PE 结构
    console.log('\nPE Structure Verification:');
    console.log(`DOS Header: ${pe_data[0].toString(16).toUpperCase()}${pe_data[1].toString(16).toUpperCase()}`);
    
    const pe_offset = new DataView(pe_data.buffer).getUint32(0x3C, true);
    console.log(`PE Header offset: 0x${pe_offset.toString(16).toUpperCase()}`);
    console.log(`PE Signature: ${pe_data[pe_offset].toString(16).toUpperCase()}${pe_data[pe_offset + 1].toString(16).toUpperCase()}`);
    
    return filename;
}

// 创建一个更简单的 "Hello World" 示例
function createSimpleHelloWorld() {
    console.log('Creating simple Hello World PE executable...');
    
    const pe_builder = new PeAssembler('x86');
    
    // 创建 .text 节（代码段）
    const text_section = pe_builder.add_section('.text', 0x60000020);
    
    // 非常简化的 x86 代码 - 只是一个返回指令
    // 注意：这个版本不会显示消息框，但会生成有效的 PE 文件
    const simple_code = new Uint8Array([
        0x55,             // push ebp
        0x8B, 0xEC,       // mov ebp, esp
        0x31, 0xC0,       // xor eax, eax (返回码 0)
        0x5D,             // pop ebp
        0xC3              // ret
    ]);
    
    text_section.set_raw_data(simple_code);
    
    // 生成 PE 文件
    const pe_data = pe_builder.build();
    
    // 保存到文件
    const filename = 'simple_hello.exe';
    writeFileSync(filename, pe_data);
    
    console.log(`Simple PE executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);
    
    return filename;
}

// 主函数
function main() {
    try {
        console.log('=== PE Hello World Generator ===\n');
        
        // 创建简单的可执行文件
        const simple_file = createSimpleHelloWorld();
        
        console.log('\n=== Instructions ===');
        console.log(`1. The file '${simple_file}' has been created`);
        console.log('2. This is a valid PE executable for x86 Windows');
        console.log('3. You can try running it, but it will just exit immediately');
        console.log('4. For a full Hello World with GUI, we need to implement:');
        console.log('   - Import table for Windows API functions');
        console.log('   - Proper relocation table');
        console.log('   - Resource section for strings');
        
    } catch (error) {
        console.error('Error creating PE executable:', error);
        process.exit(1);
    }
}

// 运行主函数
if (import.meta.url === `file://${process.argv[1]}`) {
    main();
}