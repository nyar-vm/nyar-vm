const fs = require('fs');

// 创建改进的 PE 文件 - 修复架构和格式问题
function createImprovedPE() {
    console.log('Creating improved PE executable...');
    
    const buffer = new ArrayBuffer(0x600); // 1.5KB
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);
    
    let offset = 0;
    
    // DOS 头
    // MZ 签名
    view.setUint16(0, 0x5A4D, true); // 'MZ'
    
    // DOS 存根头部信息
    view.setUint16(2, 0x0090, true); // 最后页的字节数
    view.setUint16(4, 0x0003, true); // 页数
    view.setUint16(6, 0x0000, true); // 重定位项数
    view.setUint16(8, 0x0004, true); // 头部大小 (段落)
    view.setUint16(10, 0x0000, true); // 最小额外段落
    view.setUint16(12, 0xFFFF, true); // 最大额外段落
    view.setUint16(14, 0x0000, true); // 初始 SS
    view.setUint16(16, 0x00B8, true); // 初始 SP
    view.setUint16(18, 0x0000, true); // 校验和
    view.setUint16(20, 0x0000, true); // 初始 IP
    view.setUint16(22, 0x0000, true); // 初始 CS
    view.setUint16(24, 0x0040, true); // 重定位表偏移
    view.setUint16(26, 0x0000, true); // 覆盖号
    
    // DOS 存根代码 - 简化的 DOS 程序
    offset = 0x40;
    
    // 简化的 DOS 程序：打印消息并退出
    const dos_stub = [
        0xB4, 0x09,             // mov ah, 09h (显示字符串)
        0xBA, 0x0E, 0x01,       // mov dx, 010Eh (字符串偏移)
        0xCD, 0x21,             // int 21h
        0xB8, 0x01, 0x4C,       // mov ax, 4C01h (退出，返回码 1)
        0xCD, 0x21              // int 21h
    ];
    
    for (let i = 0; i < dos_stub.length; i++) {
        bytes[offset + i] = dos_stub[i];
    }
    
    // DOS 消息
    const dos_message = "This program cannot be run in DOS mode.\r\r\n$";
    for (let i = 0; i < dos_message.length; i++) {
        bytes[offset + dos_stub.length + i] = dos_message.charCodeAt(i);
    }
    
    // PE 头偏移
    view.setUint32(0x3C, 0x100, true); // PE 头在 0x100 处
    
    // PE 头开始 (文件对齐)
    offset = 0x100;
    
    // PE 签名
    view.setUint32(offset, 0x00004550, true); // 'PE\0\0'
    offset += 4;
    
    // COFF 头
    view.setUint16(offset, 0x014C, true); // Machine (x86)
    offset += 2;
    view.setUint16(offset, 0x0001, true); // Number of sections
    offset += 2;
    view.setUint32(offset, Math.floor(Date.now() / 1000), true); // Time date stamp
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Pointer to symbol table
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Number of symbols
    offset += 4;
    view.setUint16(offset, 0x00E0, true); // Size of optional header
    offset += 2;
    view.setUint16(offset, 0x010F, true); // Characteristics (Executable, 32-bit)
    offset += 2;
    
    // Optional 头
    view.setUint16(offset, 0x010B, true); // Magic (PE32)
    offset += 2;
    view.setUint8(offset, 0x0E); // Major linker version
    offset += 1;
    view.setUint8(offset, 0x0C); // Minor linker version
    offset += 1;
    view.setUint32(offset, 0x00000200, true); // Size of code
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Size of initialized data
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Size of uninitialized data
    offset += 4;
    view.setUint32(offset, 0x00001000, true); // Address of entry point
    offset += 4;
    view.setUint32(offset, 0x00001000, true); // Base of code
    offset += 4;
    view.setUint32(offset, 0x00002000, true); // Base of data
    offset += 4;
    view.setUint32(offset, 0x00400000, true); // Image base
    offset += 4;
    view.setUint32(offset, 0x00001000, true); // Section alignment
    offset += 4;
    view.setUint32(offset, 0x00000200, true); // File alignment
    offset += 4;
    view.setUint16(offset, 0x0004, true); // Major OS version
    offset += 2;
    view.setUint16(offset, 0x0000, true); // Minor OS version
    offset += 2;
    view.setUint16(offset, 0x0000, true); // Major image version
    offset += 2;
    view.setUint16(offset, 0x0000, true); // Minor image version
    offset += 2;
    view.setUint16(offset, 0x0004, true); // Major subsystem version
    offset += 2;
    view.setUint16(offset, 0x0000, true); // Minor subsystem version
    offset += 2;
    view.setUint32(offset, 0x00000000, true); // Reserved
    offset += 4;
    view.setUint32(offset, 0x00002000, true); // Size of image
    offset += 4;
    view.setUint32(offset, 0x00000200, true); // Size of headers
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Checksum
    offset += 4;
    view.setUint16(offset, 0x0002, true); // Subsystem (Windows GUI)
    offset += 2;
    view.setUint16(offset, 0x0000, true); // DLL characteristics
    offset += 2;
    view.setUint32(offset, 0x00100000, true); // Size of stack reserve
    offset += 4;
    view.setUint32(offset, 0x00001000, true); // Size of stack commit
    offset += 4;
    view.setUint32(offset, 0x00100000, true); // Size of heap reserve
    offset += 4;
    view.setUint32(offset, 0x00001000, true); // Size of heap commit
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Loader flags
    offset += 4;
    view.setUint32(offset, 0x00000010, true); // Number of RVA and sizes
    offset += 4;
    
    // 数据目录 (全为空)
    for (let i = 0; i < 16; i++) {
        view.setUint32(offset, 0x00000000, true); // RVA
        offset += 4;
        view.setUint32(offset, 0x00000000, true); // Size
        offset += 4;
    }
    
    // 节头 - .text 节
    // 节名 (.text)
    const section_name = '.text\0\0\0';
    for (let i = 0; i < 8; i++) {
        bytes[offset + i] = section_name.charCodeAt(i) || 0;
    }
    offset += 8;
    
    view.setUint32(offset, 0x00000008, true); // Virtual size
    offset += 4;
    view.setUint32(offset, 0x00001000, true); // Virtual address
    offset += 4;
    view.setUint32(offset, 0x00000200, true); // Size of raw data
    offset += 4;
    view.setUint32(offset, 0x00000200, true); // Pointer to raw data
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Pointer to relocations
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Pointer to line numbers
    offset += 4;
    view.setUint16(offset, 0x0000, true); // Number of relocations
    offset += 2;
    view.setUint16(offset, 0x0000, true); // Number of line numbers
    offset += 2;
    view.setUint32(offset, 0x60000020, true); // Characteristics (CODE, EXECUTE, READ)
    offset += 4;
    
    // 节数据 - .text 节
    const code_offset = 0x200; // 文件对齐后的位置
    
    // 简单的 x86 代码 - 返回 42
    const code = [
        0x55,             // push ebp
        0x8B, 0xEC,       // mov ebp, esp
        0xB8, 0x2A, 0x00, 0x00, 0x00, // mov eax, 42 (返回码 42)
        0x5D,             // pop ebp
        0xC3              // ret
    ];
    
    for (let i = 0; i < code.length; i++) {
        bytes[code_offset + i] = code[i];
    }
    
    // 返回实际使用的字节
    return bytes.slice(0, 0x600);
}

// 分析 PE 文件结构
function analyzePE(data) {
    console.log('\n=== PE File Analysis ===');
    
    // 检查 DOS 头
    const dos_signature = new DataView(data.buffer).getUint16(0, true);
    console.log(`DOS Signature: 0x${dos_signature.toString(16).toUpperCase()} (${String.fromCharCode(data[0])}${String.fromCharCode(data[1])})`);
    
    // 检查 PE 偏移
    const pe_offset = new DataView(data.buffer).getUint32(0x3C, true);
    console.log(`PE Header offset: 0x${pe_offset.toString(16).toUpperCase()}`);
    
    // 检查 PE 签名
    if (pe_offset < data.length - 4) {
        const pe_signature = new DataView(data.buffer).getUint32(pe_offset, true);
        console.log(`PE Signature: 0x${pe_signature.toString(16).toUpperCase()}`);
        
        // 检查机器类型
        const machine_type = new DataView(data.buffer).getUint16(pe_offset + 4, true);
        console.log(`Machine Type: 0x${machine_type.toString(16).toUpperCase()}`);
        
        // 检查节的数量
        const num_sections = new DataView(data.buffer).getUint16(pe_offset + 6, true);
        console.log(`Number of sections: ${num_sections}`);
        
        // 检查入口点
        const entry_point = new DataView(data.buffer).getUint32(pe_offset + 40, true);
        console.log(`Entry Point: 0x${entry_point.toString(16).toUpperCase()}`);
        
        // 检查可选头的大小
        const opt_header_size = new DataView(data.buffer).getUint16(pe_offset + 20, true);
        console.log(`Optional Header Size: ${opt_header_size} bytes`);
        
        // 检查节头
        const section_header_offset = pe_offset + 24 + opt_header_size;
        console.log(`Section Header offset: 0x${section_header_offset.toString(16).toUpperCase()}`);
        
        if (section_header_offset < data.length - 40) {
            // 读取第一个节的名称
            let section_name = '';
            for (let i = 0; i < 8; i++) {
                const char = data[section_header_offset + i];
                if (char === 0) break;
                section_name += String.fromCharCode(char);
            }
            console.log(`First Section: ${section_name}`);
            
            // 读取节的虚拟地址和大小
            const virtual_size = new DataView(data.buffer).getUint32(section_header_offset + 8, true);
            const virtual_address = new DataView(data.buffer).getUint32(section_header_offset + 12, true);
            const raw_data_size = new DataView(data.buffer).getUint32(section_header_offset + 16, true);
            const raw_data_ptr = new DataView(data.buffer).getUint32(section_header_offset + 20, true);
            
            console.log(`  Virtual Size: 0x${virtual_size.toString(16).toUpperCase()}`);
            console.log(`  Virtual Address: 0x${virtual_address.toString(16).toUpperCase()}`);
            console.log(`  Raw Data Size: 0x${raw_data_size.toString(16).toUpperCase()}`);
            console.log(`  Raw Data Pointer: 0x${raw_data_ptr.toString(16).toUpperCase()}`);
        }
    }
}

// 主函数
function main() {
    try {
        console.log('=== Improved PE Hello World Generator ===\n');
        
        // 创建改进的 PE 文件
        const pe_data = createImprovedPE();
        
        // 保存到文件
        const filename = 'improved_hello.exe';
        fs.writeFileSync(filename, pe_data);
        
        console.log(`Improved PE executable created: ${filename}`);
        console.log(`File size: ${pe_data.length} bytes`);
        
        // 分析 PE 文件
        analyzePE(pe_data);
        
        console.log('\n=== Instructions ===');
        console.log(`1. The file '${filename}' has been created`);
        console.log('2. This should be a valid PE executable for x86 Windows');
        console.log('3. Try running it to see if it returns error code 42');
        console.log('4. If it still fails, we may need to add proper imports');
        
    } catch (error) {
        console.error('Error creating PE executable:', error);
        process.exit(1);
    }
}

// 运行主函数
if (require.main === module) {
    main();
}