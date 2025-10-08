// 直接导入 TypeScript 源文件进行测试
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { writeFileSync } from 'fs';

// 直接导入我们的 TypeScript 源文件
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// 动态导入 TypeScript 文件
const srcPath = join(__dirname, '../src');

// 创建一个简单的 PE 文件生成器
async function createSimplePE() {
    try {
        console.log('Creating simple PE executable...');
        
        // 手动创建简单的 PE 文件结构
        const pe_data = createMinimalPE();
        
        // 保存到文件
        const filename = 'minimal_hello.exe';
        writeFileSync(filename, pe_data);
        
        console.log(`Minimal PE executable created: ${filename}`);
        console.log(`File size: ${pe_data.length} bytes`);
        
        // 验证 PE 结构
        console.log('\nPE Structure Verification:');
        console.log(`DOS Header: ${pe_data[0].toString(16).toUpperCase()}${pe_data[1].toString(16).toUpperCase()}`);
        
        const pe_offset = new DataView(pe_data.buffer).getUint32(0x3C, true);
        console.log(`PE Header offset: 0x${pe_offset.toString(16).toUpperCase()}`);
        console.log(`PE Signature: ${pe_data[pe_offset].toString(16).toUpperCase()}${pe_data[pe_offset + 1].toString(16).toUpperCase()}`);
        
        return filename;
        
    } catch (error) {
        console.error('Error creating PE executable:', error);
        process.exit(1);
    }
}

// 创建最小的 PE 文件
function createMinimalPE() {
    const buffer = new ArrayBuffer(0x400); // 1KB 应该足够
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);
    
    let offset = 0;
    
    // DOS 头
    // MZ 签名
    view.setUint16(0, 0x5A4D, true); // 'MZ'
    
    // DOS 存根 - 简化的 DOS 程序
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
    
    // DOS 存根代码
    offset = 0x40;
    
    // 简化的 DOS 程序：打印消息并退出
    const dos_stub = [
        0xB8, 0x01, 0x00,       // mov ax, 1
        0xCD, 0x21,             // int 21h
        0xB8, 0x4C, 0x00,       // mov ax, 4C00h
        0xCD, 0x21              // int 21h
    ];
    
    for (let i = 0; i < dos_stub.length; i++) {
        bytes[offset + i] = dos_stub[i];
    }
    
    // PE 头偏移 (通常在 0x80 或 0xE0)
    view.setUint32(0x3C, 0x80, true);
    
    // PE 头开始
    offset = 0x80;
    
    // PE 签名
    view.setUint32(offset, 0x00004550, true); // 'PE\0\0'
    offset += 4;
    
    // COFF 头
    view.setUint16(offset, 0x014C, true); // Machine (x86)
    offset += 2;
    view.setUint16(offset, 0x0001, true); // Number of sections
    offset += 2;
    view.setUint32(offset, 0x00000000, true); // Time date stamp
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Pointer to symbol table
    offset += 4;
    view.setUint32(offset, 0x00000000, true); // Number of symbols
    offset += 4;
    view.setUint16(offset, 0x00E0, true); // Size of optional header
    offset += 2;
    view.setUint16(offset, 0x0103, true); // Characteristics
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
    view.setUint32(offset, 0x00000200, true); // Size of image
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
    
    // 节头
    // .text 节
    const section_offset = offset;
    
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
    view.setUint32(offset, 0x60000020, true); // Characteristics
    offset += 4;
    
    // 节数据
    const code_offset = 0x200; // 文件对齐后的位置
    
    // 简单的 x86 代码
    const code = [
        0x55,             // push ebp
        0x8B, 0xEC,       // mov ebp, esp
        0x31, 0xC0,       // xor eax, eax (返回码 0)
        0x5D,             // pop ebp
        0xC3              // ret
    ];
    
    for (let i = 0; i < code.length; i++) {
        bytes[code_offset + i] = code[i];
    }
    
    // 返回实际使用的字节
    return bytes.slice(0, 0x400);
}

// 运行测试
createSimplePE();