import { describe, expect, it } from 'vitest';
import { PeReader } from '@/reader/PeReader';
import { PeAssembler, PeTargetArchitecture, PeSection } from '@/pe-writer';
import fs from 'fs';

describe('PE Reader Tests', () => {
    it('should read and parse a PE file', () => {
        // 创建一个简单的 PE 文件用于测试
        const pe_builder = new PeAssembler(PeTargetArchitecture.X64);
        
        // 添加一个简单的文本节
        const text_section = PeSection.create_text_section(new Uint8Array([
            0xB8, 0x01, 0x00, 0x00, 0x00, // mov eax, 1
            0xC3                          // ret
        ]));
        
        pe_builder.add_section(text_section);
        
        // 生成 PE 文件
        const pe_data = pe_builder.build();
        
        // 使用 PeReader 读取 PE 文件
        const pe_reader = new PeReader(pe_data);
        const pe_assembler = pe_reader.read();
        
        // 验证解析结果
        expect(pe_assembler).toBeInstanceOf(PeAssembler);
        
        // 验证 DOS 头
        const dos_header = pe_reader.get_dos_header();
        expect(dos_header).not.toBeNull();
        expect(dos_header!.e_magic).toBe(0x5A4D); // 'MZ'
        expect(dos_header!.e_lfanew).toBeGreaterThan(0);
        
        // 验证文件头
        const file_header = pe_reader.get_file_header();
        expect(file_header).not.toBeNull();
        expect(file_header!.machine).toBe(0x8664); // AMD64
        expect(file_header!.number_of_sections).toBeGreaterThan(0);
        
        // 验证可选头
        const optional_header = pe_reader.get_optional_header();
        expect(optional_header).not.toBeNull();
        expect(optional_header!.magic).toBe(0x20B); // PE32+
        
        // 验证节头
        const section_headers = pe_reader.get_section_headers();
        expect(section_headers.length).toBeGreaterThan(0);
        expect(section_headers.some(section => section.name === '.text')).toBe(true);
    });
    
    it('should throw error for invalid PE file', () => {
        // 创建一个无效的 PE 文件
        const invalid_data = new Uint8Array([0x00, 0x01, 0x02, 0x03]);
        
        expect(() => {
            const pe_reader = new PeReader(invalid_data);
            pe_reader.read();
        }).toThrow('Invalid DOS header signature');
    });
    
    it('should throw error for file too small', () => {
        // 创建一个太小的文件
        const small_data = new Uint8Array([0x4D, 0x5A]); // 只有 MZ 标识
        
        expect(() => {
            const pe_reader = new PeReader(small_data);
            pe_reader.read();
        }).toThrow('File too small to be a valid PE file');
    });
    
    it('should detect x86 architecture correctly', () => {
        // 创建一个 x86 PE 文件
        const pe_builder = new PeAssembler(PeTargetArchitecture.X86);
        
        // 添加一个简单的文本节
        const text_section = PeSection.create_text_section(new Uint8Array([
            0xB8, 0x01, 0x00, 0x00, 0x00, // mov eax, 1
            0xC3                          // ret
        ]));
        
        pe_builder.add_section(text_section);
        
        // 生成 PE 文件
        const pe_data = pe_builder.build();
        
        // 使用 PeReader 读取 PE 文件
        const pe_reader = new PeReader(pe_data);
        const pe_assembler = pe_reader.read();
        
        // 验证解析结果
        expect(pe_assembler).toBeInstanceOf(PeAssembler);
        
        // 验证文件头
        const file_header = pe_reader.get_file_header();
        expect(file_header).not.toBeNull();
        expect(file_header!.machine).toBe(0x014C); // I386
    });
    
    it('should detect x64 architecture correctly', () => {
        // 创建一个 x64 PE 文件
        const pe_builder = new PeAssembler(PeTargetArchitecture.X64);
        
        // 添加一个简单的文本节
        const text_section = pe_builder.add_section('.text', 0x60000020);
        
        // 简单的 x86 汇编代码
        const hello_code = new Uint8Array([
            0xB8, 0x01, 0x00, 0x00, 0x00, // mov eax, 1
            0xC3                          // ret
        ]);
        
        text_section.set_data(hello_code);
        
        // 生成 PE 文件
        const pe_data = pe_builder.build();
        
        // 使用 PeReader 读取 PE 文件
        const pe_reader = new PeReader(pe_data);
        const pe_assembler = pe_reader.read();
        
        // 验证解析结果
        expect(pe_assembler).toBeInstanceOf(PeAssembler);
        
        // 验证文件头
        const file_header = pe_reader.get_file_header();
        expect(file_header).not.toBeNull();
        expect(file_header!.machine).toBe(0x8664); // AMD64
    });
    
    it('should parse section headers correctly', () => {
        // 创建一个包含多个节的 PE 文件
        const pe_builder = new PeAssembler(PeTargetArchitecture.X64);
        
        // 添加多个节
        const text_section = pe_builder.add_section('.text', 0x60000020);
        const data_section = pe_builder.add_section('.data', 0xC0000040);
        const rdata_section = pe_builder.add_section('.rdata', 0x40000040);
        
        // 设置节数据
        text_section.set_data(new Uint8Array([0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3]));
        data_section.set_data(new Uint8Array([0x01, 0x02, 0x03, 0x04]));
        rdata_section.set_data(new Uint8Array([0x68, 0x65, 0x6C, 0x6C, 0x6F])); // "hello"
        
        // 生成 PE 文件
        const pe_data = pe_builder.build();
        
        // 使用 PeReader 读取 PE 文件
        const pe_reader = new PeReader(pe_data);
        const pe_assembler = pe_reader.read();
        
        // 验证解析结果
        expect(pe_assembler).toBeInstanceOf(PeAssembler);
        
        // 验证节头
        const section_headers = pe_reader.get_section_headers();
        expect(section_headers.length).toBe(3);
        
        // 验证节名称
        const section_names = section_headers.map(section => section.name);
        expect(section_names).toContain('.text');
        expect(section_names).toContain('.data');
        expect(section_names).toContain('.rdata');
        
        // 验证节特征
        const text_section_header = section_headers.find(section => section.name === '.text');
        expect(text_section_header).toBeDefined();
        expect(text_section_header!.characteristics & 0x00000020).toBe(0x00000020); // IMAGE_SCN_CNT_CODE
        expect(text_section_header!.characteristics & 0x20000000).toBe(0x20000000); // IMAGE_SCN_MEM_EXECUTE
        
        const data_section_header = section_headers.find(section => section.name === '.data');
        expect(data_section_header).toBeDefined();
        expect(data_section_header!.characteristics & 0x00000040).toBe(0x00000040); // IMAGE_SCN_CNT_INITIALIZED_DATA
        expect(data_section_header!.characteristics & 0x80000000).toBe(0x80000000); // IMAGE_SCN_MEM_WRITE
        
        const rdata_section_header = section_headers.find(section => section.name === '.rdata');
        expect(rdata_section_header).toBeDefined();
        expect(rdata_section_header!.characteristics & 0x00000040).toBe(0x00000040); // IMAGE_SCN_CNT_INITIALIZED_DATA
        expect(rdata_section_header!.characteristics & 0x40000000).toBe(0x40000000); // IMAGE_SCN_MEM_READ
    });
});