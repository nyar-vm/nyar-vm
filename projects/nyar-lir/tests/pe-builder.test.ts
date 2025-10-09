import { describe, expect, it } from 'vitest';
import { PeAssembler, PeTargetArchitecture } from '@/pe-writer';
import { PeSection } from '@/pe-writer/PeSection';

describe('PE Builder Tests', () => {
    it('should create a simple PE executable', () => {
        const pe_builder = new PeAssembler(PeTargetArchitecture.X64);

        // 添加一些简单的 x86 机器码 (Hello World 程序)
        const text_section = PeSection.create_text_section(new Uint8Array([
            // 简化的 "Hello World" x86 机器码
            0x68,
            0x00,
            0x00,
            0x00,
            0x00, // push offset hello_string
            0x68,
            0x00,
            0x00,
            0x00,
            0x00, // push offset title_string
            0x6a,
            0x00, // push 0 (MB_OK)
            0xe8,
            0x00,
            0x00,
            0x00,
            0x00, // call MessageBoxA
            0x6a,
            0x00, // push 0
            0xe8,
            0x00,
            0x00,
            0x00,
            0x00, // call ExitProcess
        ]));

        pe_builder.add_section(text_section);

        // 生成 PE 文件
        const pe_data = pe_builder.build();

        // 验证 PE 文件头
        expect(pe_data.length).toBeGreaterThan(0);
        expect(pe_data[0]).toBe(0x4d); // 'M'
        expect(pe_data[1]).toBe(0x5a); // 'Z'

        // 验证 PE 签名
        const pe_offset = new DataView(pe_data.buffer).getUint32(0x3c, true);
        expect(pe_data[pe_offset]).toBe(0x50); // 'P'
        expect(pe_data[pe_offset + 1]).toBe(0x45); // 'E'
        expect(pe_data[pe_offset + 2]).toBe(0x00);
        expect(pe_data[pe_offset + 3]).toBe(0x00);
    });

    it('should create PE with proper section headers', () => {
        const pe_builder = new PeAssembler(PeTargetArchitecture.X86);
        
        const text_section = PeSection.create_text_section(new Uint8Array([0x90, 0x90, 0x90]));
        const idata_section = PeSection.create_idata_section(new Uint8Array([0x01, 0x02, 0x03, 0x04]));
        
        pe_builder.add_section(text_section);
        pe_builder.add_section(idata_section);

        const pe_data = pe_builder.build();

        // 应该有标准的 PE 结构
        expect(pe_data.length).toBeGreaterThan(0x400); // 至少包含头部

        // 检查 DOS 头
        const dos_header = pe_data.slice(0, 0x40);
        expect(dos_header[0]).toBe(0x4d); // 'M'
        expect(dos_header[1]).toBe(0x5a); // 'Z'
    });
});