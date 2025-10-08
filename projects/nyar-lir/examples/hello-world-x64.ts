import { PeAssembler } from '../src/pe/PeBuilder.js';
import { BinaryWriter } from '../src/BinaryWriter.js';
import { writeFileSync } from 'fs';

// 创建一个真正能在 x64 Windows 上运行的 Hello World
function createWorkingHelloWorldX64() {
    console.log('Creating working x64 Hello World PE executable...');
    
    const pe_builder = new PeAssembler('x64'); // 使用 x64 架构
    
    // 创建 .text 节（代码段）
    const text_section = pe_builder.add_section('.text', 0x60000020);
    
    // x64 汇编代码：调用 WriteConsoleA 和 ExitProcess
    // 这是一个简化的版本，实际需要导入表和重定位
    const hello_code = new Uint8Array([
        // 函数入口点
        0x55,                                     // push rbp
        0x48, 0x8B, 0xEC,                         // mov rbp, rsp
        0x48, 0x83, 0xEC, 0x20,                   // sub rsp, 0x20 (分配栈空间)
        
        // 获取标准输出句柄 (GetStdHandle)
        0x48, 0xC7, 0xC1, 0xF5, 0xFF, 0xFF, 0xFF, // mov rcx, -11 (STD_OUTPUT_HANDLE)
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00,     // call GetStdHandle (需要重定位)
        
        // 准备 WriteConsoleA 参数
        0x48, 0x89, 0xC1,                         // mov rcx, rax (句柄)
        0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00, // lea rdx, [message] (需要重定位)
        0xB8, 0x0C, 0x00, 0x00, 0x00,            // mov eax, 12 (消息长度)
        0x49, 0x89, 0xC0,                         // mov r8, rax (长度)
        0x48, 0x8D, 0x4D, 0x00,                   // lea rcx, [rbp+0] (写入字节数)
        0x41, 0xB9, 0x00, 0x00, 0x00, 0x00,      // mov r9d, 0 (保留)
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00,     // call WriteConsoleA (需要重定位)
        
        // 调用 ExitProcess
        0x33, 0xC9,                               // xor ecx, ecx
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00,     // call ExitProcess (需要重定位)
        
        0x5D,                                     // pop rbp
        0xC3                                      // ret
    ]);
    
    text_section.set_raw_data(hello_code);
    
    // 创建 .data 节（数据段）
    const data_section = pe_builder.add_section('.data', 0xc0000040);
    const hello_message = new Uint8Array([
        0x48, 0x65, 0x6C, 0x6C, 0x6F, // "Hello"
        0x2C, 0x20,                     // ", "
        0x57, 0x6F, 0x72, 0x6C, 0x64, // "World"
        0x21, 0x0A,                     // "!\n"
        0x00                            // null terminator
    ]);
    data_section.set_raw_data(hello_message);
    
    // 生成 PE 文件
    console.log('Building PE file...');
    const pe_data = pe_builder.build();
    
    // 保存到文件
    const filename = 'hello_world_x64.exe';
    writeFileSync(filename, pe_data);
    
    console.log(`x64 Hello World PE executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);
    
    return filename;
}

// 创建一个简单的 x64 可执行文件（确保能工作）
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

// 测试现有的可执行文件
function testExistingExe(filename: string) {
    console.log(`\n=== Testing ${filename} ===`);
    
    try {
        const { execSync } = require('child_process');
        const result = execSync(`cmd /c "${filename} & echo Exit code: %errorlevel%"`, { encoding: 'utf8' });
        console.log('Execution result:', result);
    } catch (error) {
        console.log('Execution failed:', error.message);
    }
}

// 主函数
function main() {
    try {
        console.log('=== x64 Hello World PE Generator ===\n');
        
        // 创建简单的 x64 可执行文件
        const simple_file = createSimpleX64Exe();
        
        console.log('\n=== Testing Simple x64 Version ===');
        testExistingExe(simple_file);
        
        console.log('\n=== Creating Hello World Version ===');
        const hello_file = createWorkingHelloWorldX64();
        
        console.log('\n=== Testing Hello World Version ===');
        testExistingExe(hello_file);
        
        console.log('\n=== Summary ===');
        console.log(`Created: ${simple_file} (simple return)`);
        console.log(`Created: ${hello_file} (with Hello World message)`);
        console.log('\nNote: The Hello World version needs proper import table implementation');
        console.log('to actually display the message. The simple version should work.');
        
    } catch (error) {
        console.error('Error creating PE executable:', error);
        process.exit(1);
    }
}

// 运行主函数
if (import.meta.url === `file://${process.argv[1]}`) {
    main();
}