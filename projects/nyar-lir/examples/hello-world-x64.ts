import { writeFileSync } from 'fs';
import { PeAssembler } from "../src/pe-writer/PeAssembler";

// 创建一个简单的 Hello World PE 可执行文件 (x64)
function createHelloWorldExe() {
    console.log('Creating Hello World PE executable for x64...');
    
    const pe_builder = new PeAssembler('x64');

    // 导入所需的函数
    pe_builder.add_import('user32.dll', 'MessageBoxA');
    pe_builder.add_import('kernel32.dll', 'ExitProcess');
    
    // 创建 .text 节（代码段）
    const text_section = pe_builder.add_section('.text', 0x60000020);

    const message_string = new TextEncoder().encode("Hello World!\0");
    const title_string = new TextEncoder().encode("My App\0");

    // 计算字符串和导入函数的偏移量
    const code_length = 38; // 汇编代码的长度
    const offset_to_message = code_length; // 消息字符串在 combined_code 中的偏移量
    const offset_to_title = code_length + message_string.length; // 标题字符串在 combined_code 中的偏移量

    // 简化的 x64 汇编代码，调用 Windows API
    const hello_code = new Uint8Array([
        // 简化的程序入口点
        0x48, 0x83, 0xEC, 0x28,       // sub rsp, 40 (shadow space + alignment)
        0x48, 0x31, 0xC9,             // xor rcx, rcx (MB_OK)
        0x48, 0x8D, 0x15, 0x00, 0x00, 0x00, 0x00, // lea rdx, [rip + offset_to_title]
        0x48, 0x8D, 0x1D, 0x00, 0x00, 0x00, 0x00, // lea r8, [rip + offset_to_message]
        0x48, 0x31, 0xC0,             // xor rax, rax
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, // call [rip + offset_to_MessageBoxA]
        0x48, 0x31, 0xC9,             // xor rcx, rcx (exit code 0)
        0xFF, 0x15, 0x00, 0x00, 0x00, 0x00, // call [rip + offset_to_ExitProcess]
    ]);

    // 填充占位符
    const fill_offset = (arr: Uint8Array, index: number, value: number) => {
        arr[index] = value & 0xFF;
        arr[index + 1] = (value >> 8) & 0xFF;
        arr[index + 2] = (value >> 16) & 0xFF;
        arr[index + 3] = (value >> 24) & 0xFF;
    };

    // 计算 MessageBoxA 和 ExitProcess 的 IAT RVA
    const message_box_a_iat_rva = pe_builder.get_iat_rva('user32.dll', 'MessageBoxA');
    const exit_process_iat_rva = pe_builder.get_iat_rva('kernel32.dll', 'ExitProcess');

    // 计算 rip 相对偏移量
    // lea rdx, [rip + offset_to_title]
    // 当前指令的下一条指令的地址是 hello_code[10] + 7 = 17
    // 目标地址是 .text 节的基址 + offset_to_title
    // rip 相对偏移 = (目标地址 - 当前指令的下一条指令的地址)
    const rip_offset_to_title = (text_section.get_virtual_address() + offset_to_title) - (text_section.get_virtual_address() + 10 + 7);
    fill_offset(hello_code, 10, rip_offset_to_title);

    // lea r8, [rip + offset_to_message]
    // 当前指令的下一条指令的地址是 hello_code[17] + 7 = 24
    const rip_offset_to_message = (text_section.get_virtual_address() + offset_to_message) - (text_section.get_virtual_address() + 17 + 7);
    fill_offset(hello_code, 17, rip_offset_to_message);

    // call [rip + offset_to_MessageBoxA]
    // 当前指令的下一条指令的地址是 hello_code[24] + 6 = 30
    const rip_offset_to_message_box_a_iat = message_box_a_iat_rva - (text_section.get_virtual_address() + 24 + 6);
    fill_offset(hello_code, 26, rip_offset_to_message_box_a_iat);

    // call [rip + offset_to_ExitProcess]
    // 当前指令的下一条指令的地址是 hello_code[30] + 6 = 36
    const rip_offset_to_exit_process_iat = exit_process_iat_rva - (text_section.get_virtual_address() + 30 + 6);
    fill_offset(hello_code, 34, rip_offset_to_exit_process_iat);

    // 将字符串数据附加到代码段
    const combined_code = new Uint8Array(hello_code.length + message_string.length + title_string.length);
    combined_code.set(hello_code, 0);
    combined_code.set(message_string, hello_code.length);
    combined_code.set(title_string, hello_code.length + message_string.length);

    text_section.set_raw_data(combined_code);

    // 创建 .data 节（数据段）
    const data_section = pe_builder.add_section('.data', 0xc0000040);
    const data = new Uint8Array([
        0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x57, 0x6F, 0x72, 0x6C, 0x64, 0x21, 0x00, // "Hello World!\0"
        0x4D, 0x79, 0x20, 0x41, 0x70, 0x70, 0x00 // "My App\0"
    ]);
    data_section.set_raw_data(data);

    // 生成 PE 文件
    console.log('Building PE file...');
    const pe_data = pe_builder.build();

    // 保存到文件
    const filename = 'e:\\RustroverProjects\\nyar-framework\\nyar-vm\\projects\\nyar-lir\\examples\\hello_world_x64.exe';
    writeFileSync(filename, pe_data);

    console.log(`PE executable created: ${filename}`);
    console.log(`File size: ${pe_data.length} bytes`);

    return filename;
}

// 主函数
function main() {
    try {
        console.log('=== PE Hello World Generator (x64) ===\n');
        const filename = createHelloWorldExe();
        console.log(`\nCreated: ${filename}`);
    } catch (error) {
        console.error('Error creating PE executable:', error);
        process.exit(1);
    }
}

// 运行主函数
main();