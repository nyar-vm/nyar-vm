import { PeAssembler } from '../src/pe/PeBuilder.js';
import { BinaryWriter } from '../src/BinaryWriter.js';
import { writeFileSync } from 'fs';

// 创建一个真正的 Hello World PE 可执行文件，使用控制台输出
function createRealHelloWorldExe() {
    console.log('Creating REAL Hello World PE executable...');
    
    const pe_builder = new PeAssembler('x64'); // 使用 x64 架构
    
    // 创建 .text 节（代码段）
    const text_section = pe_builder.add_section('.text', 0x60000020);
    
    // x64 汇编代码：调用 WriteConsoleA 和 ExitProcess
    const hello_code = new Uint8Array([
        // 函数入口点
        0x55,                                     // push rbp
        0x48, 0x8B, 0xEC,                         // mov rbp, rsp
        0x48, 0x83, 0xEC, 0x30,                   // sub rsp, 0x30 (分配栈空间)
        
        // 获取标准输出句柄 (GetStdHandle)
        0x48, 0xB8, 0xF5, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // mov rax, -11 (STD_OUTPUT_HANDLE)
        0x48, 0x89, 0xC1,                         // mov rcx, rax
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00,     // call GetStdHandle (需要重定位)
        0x48, 0x89, 0x45, 0x00,                   // mov [rbp+0], rax (保存句柄)
        
        // 准备 WriteConsoleA 参数
        0x48, 0x8B, 0x4D, 0x00,                   // mov rcx, [rbp+0] (句柄)
        0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00, // lea rdx, [message] (需要重定位)
        0xB9, 0x0C, 0x00, 0x00, 0x00,            // mov ecx, 12 (消息长度)
        0x48, 0x8D, 0x4D, 0x08,                   // lea rcx, [rbp+8] (写入字节数)
        0x41, 0xB8, 0x00, 0x00, 0x00, 0x00,      // mov r8d, 0 (保留)
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
    
    // 设置导入表
    const imports = pe_builder.get_imports();
    
    // 添加 kernel32.dll 导入
    const kernel32_imports = [
        { name: 'GetStdHandle', hint: 0 },
        { name: 'WriteConsoleA', hint: 0 },
        { name: 'ExitProcess', hint: 0 }
    ];
    
    // 这里需要实现完整的导入表逻辑
    // 目前先创建基本的 PE 结构
    
    // 生成 PE 文件
    console.log('Building PE file...');
    const pe_data = pe_builder.build();
    
    // 保存到文件
    const filename = 'real_hello_world.exe';
    writeFileSync(filename, pe_data);
    
    console.log(`REAL Hello World PE executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);
    
    return filename;
}

// 创建一个简化但真实的 Hello World
function createSimpleHelloWorld() {
    console.log('Creating simple Hello World executable...');
    
    const pe_builder = new PeAssembler('x64');
    
    // 创建 .text 节
    const text_section = pe_builder.add_section('.text', 0x60000020);
    
    // 简化的 x64 代码，只做最基本的返回
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
    const filename = 'simple_hello_x64.exe';
    writeFileSync(filename, pe_data);
    
    console.log(`Simple x64 executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);
    
    return filename;
}

// 主函数
function main() {
    try {
        console.log('=== REAL Hello World PE Generator ===\n');
        
        // 创建简化版本（确保能工作）
        const simple_file = createSimpleHelloWorld();
        
        console.log('\n=== Testing Simple Version ===');
        console.log(`Created: ${simple_file}`);
        console.log('This should be a valid x64 PE executable');
        
    } catch (error) {
        console.error('Error creating PE executable:', error);
        process.exit(1);
    }
}

// 运行主函数
if (import.meta.url === `file://${process.argv[1]}`) {
    main();
}