import { PeAssembler } from '../pe-writer/PeAssembler';
import { PeTargetArchitecture } from '../pe-writer/PeTargetArchitecture';
import { ImportTable } from '../pe-writer/ImportTable';
import { PeSection } from '../pe-writer/PeSection';
import * as fs from 'fs';

/**
 * 生成最简单的PE文件，只包含一个ret指令
 */
function generate_minimal_code(): Uint8Array {
    // 只有一个ret指令
    return new Uint8Array([0xC3]); // ret
}

/**
 * 创建最简单的PE文件
 */
function create_minimal_pe(): Uint8Array {
    const assembler = new PeAssembler(PeTargetArchitecture.X64);
    
    // 不添加导入表，只添加代码段
    const code = generate_minimal_code();
    const text_section = PeSection.create_text_section(code);
    assembler.add_section(text_section);
    
    return assembler.build();
}

/**
 * 十六进制转储函数
 */
function hexDump(data: Uint8Array, maxBytes: number = 512): string {
    let result = '';
    for (let i = 0; i < Math.min(data.length, maxBytes); i += 16) {
        // 地址
        result += i.toString(16).padStart(8, '0') + ': ';
        
        // 十六进制字节
        for (let j = 0; j < 16; j++) {
            if (i + j < data.length) {
                result += data[i + j].toString(16).padStart(2, '0') + ' ';
            } else {
                result += '   ';
            }
        }
        
        // ASCII表示
        result += ' |';
        for (let j = 0; j < 16 && i + j < data.length; j++) {
            const byte = data[i + j];
            result += (byte >= 32 && byte <= 126) ? String.fromCharCode(byte) : '.';
        }
        result += '|\n';
    }
    return result;
}

/**
 * 分析PE文件结构
 */
function analyzePeStructure(data: Uint8Array): void {
    console.log('=== PE文件结构分析 ===');
    console.log(`文件大小: ${data.length} 字节`);
    
    // DOS头
    if (data.length >= 64) {
        const dosSignature = String.fromCharCode(data[0], data[1]);
        console.log(`DOS签名: ${dosSignature} (应该是 'MZ')`);
        
        // e_lfanew (PE头偏移)
        const e_lfanew = data[60] | (data[61] << 8) | (data[62] << 16) | (data[63] << 24);
        console.log(`PE头偏移 (e_lfanew): 0x${e_lfanew.toString(16)}`);
        
        // PE签名
        if (data.length > e_lfanew + 4) {
            const peSignature = String.fromCharCode(data[e_lfanew], data[e_lfanew + 1], data[e_lfanew + 2], data[e_lfanew + 3]);
            console.log(`PE签名: ${peSignature} (应该是 'PE\\0\\0')`);
        }
        
        // COFF头
        if (data.length > e_lfanew + 24) {
            const machine = data[e_lfanew + 4] | (data[e_lfanew + 5] << 8);
            const numberOfSections = data[e_lfanew + 6] | (data[e_lfanew + 7] << 8);
            console.log(`机器类型: 0x${machine.toString(16)} (0x8664 = x64)`);
            console.log(`节数量: ${numberOfSections}`);
        }
        
        // Optional Header
        if (data.length > e_lfanew + 24 + 20) {
            const optionalHeaderOffset = e_lfanew + 24;
            const magic = data[optionalHeaderOffset] | (data[optionalHeaderOffset + 1] << 8);
            console.log(`Optional Header Magic: 0x${magic.toString(16)} (0x20B = PE32+)`);
            
            // Entry Point
            const entryPointOffset = optionalHeaderOffset + 16;
            if (data.length > entryPointOffset + 4) {
                const entryPoint = data[entryPointOffset] | (data[entryPointOffset + 1] << 8) | 
                                 (data[entryPointOffset + 2] << 16) | (data[entryPointOffset + 3] << 24);
                console.log(`入口点RVA: 0x${entryPoint.toString(16)}`);
            }
            
            // Image Base
            const imageBaseOffset = optionalHeaderOffset + 24;
            if (data.length > imageBaseOffset + 8) {
                let imageBase = 0n;
                for (let i = 0; i < 8; i++) {
                    imageBase |= BigInt(data[imageBaseOffset + i]) << BigInt(i * 8);
                }
                console.log(`镜像基址: 0x${imageBase.toString(16)}`);
            }
        }
    }
    
    console.log('\n=== 十六进制转储 (前512字节) ===');
    console.log(hexDump(data, 512));
}

/**
 * 主函数
 */
function main() {
    try {
        console.log('生成最简单的PE文件进行调试...');
        
        // 创建最简单的PE文件
        const pe_data = create_minimal_pe();
        
        // 分析结构
        analyzePeStructure(pe_data);
        
        // 写入文件
        const output_path = 'debug-minimal.exe';
        fs.writeFileSync(output_path, pe_data);
        
        console.log(`\n调试文件已生成: ${output_path}`);
        console.log('可以尝试运行此文件来测试基本的PE结构');
        
    } catch (error) {
        console.error('生成调试PE文件时出错:', error);
        process.exit(1);
    }
}

// 如果直接执行此文件则运行
if (require.main === module) {
    main();
}

export { create_minimal_pe, generate_minimal_code, analyzePeStructure };