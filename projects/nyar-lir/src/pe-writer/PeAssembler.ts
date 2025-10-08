import {BinaryWriter} from '../BinaryWriter';
import {PeSection} from './PeSection';
import {ImportTable} from './ImportTable';
import {PeTargetArchitecture} from './PeTargetArchitecture';
import {PeHeaders} from './PeHeaders';

export class PeAssembler {
    private pe_headers: PeHeaders;
    private sections: Map<string, PeSection>;
    private file_alignment: number;
    private section_alignment: number;
    private import_table: ImportTable;

    constructor(target_architecture = PeTargetArchitecture.X86) {
        this.sections = new Map();
        this.pe_headers = new PeHeaders(target_architecture);
        this.file_alignment = 0x200;
        this.section_alignment = 0x1000;
        this.init_standard_sections();
        this.import_table = new ImportTable();
    }

    private init_standard_sections(): void {
        // 标准PE节
        this.add_section('.text', 0x60000020); // 可执行、可读、代码
        this.add_section('.data', 0xc0000040); // 可读、可写、初始化数据
        this.add_section('.rdata', 0x40000040); // 只读数据
        this.add_section('.pdata', 0xc0000040); // 异常信息(x64)
        this.add_section('.reloc', 0x42000000); // 重定位
    }

    add_section(
        name: string,
        characteristics: number,
        raw_data: Uint8Array | null = null
    ): PeSection {
        const section = new PeSection(name, characteristics);
        if (raw_data) {
            section.set_raw_data(raw_data);
        }
        this.sections.set(name, section);
        return section;
    }

    get_sections_count(): number {
        return this.sections.size;
    }

    get_architecture(): PeTargetArchitecture {
        return this.pe_headers.architecture;
    }

    get_base_address(): number {
        return this.pe_headers.get_base_address();
    }

    get_section_alignment(): number {
        return this.section_alignment;
    }

    get_file_alignment(): number {
        return this.file_alignment;
    }

    add_import(library_name: string, function_name: string) {
        this.import_table.add_import(library_name, function_name);
    }

    get_iat_rva(library_name: string, function_name: string): number {
        return this.import_table.get_iat_rva(library_name, function_name);
    }

    get_exception_directory(): { rva: number; size: number } {
        return {rva: 0, size: 0};
    }

    private calculate_code_size(): number {
        let total_size = 0;
        for (const section of this.sections.values()) {
            if ((section.get_characteristics() & 0x00000020) === 0x00000020) {
                // IMAGE_SCN_CNT_CODE
                total_size += section.get_raw_data_size();
            }
        }
        return total_size;
    }

    private calculate_initialized_data_size(): number {
        let total_size = 0;
        for (const section of this.sections.values()) {
            if ((section.get_characteristics() & 0x00000040) === 0x00000040) {
                // IMAGE_SCN_CNT_INITIALIZED_DATA
                total_size += section.get_raw_data_size();
            }
        }
        return total_size;
    }

    private calculate_uninitialized_data_size(): number {
        let total_size = 0;
        for (const section of this.sections.values()) {
            if ((section.get_characteristics() & 0x00000080) === 0x00000080) {
                // IMAGE_SCN_CNT_UNINITIALIZED_DATA
                total_size += section.get_virtual_size();
            }
        }
        return total_size;
    }

    private calculate_image_size(): number {
        let image_size = 0;
        for (const section of this.sections.values()) {
            const end_address = section.get_virtual_address() + section.get_virtual_size();
            image_size = Math.max(image_size, end_address);
        }
        return this.align_to_section_alignment(image_size);
    }

    private calculate_headers_size(): number {
        let size = 64; // DOS Header
        size += this.generate_dos_stub().length; // DOS Stub
        size += 4; // PE Signature
        size += 20; // File Header
        size += this.pe_headers.optional_header_size; // Optional Header - use public field
        size += this.get_sections_count() * 40; // Section Headers (40 bytes per section)
        return this.align_to_file_alignment(size);
    }

    private align_to_section_alignment(size: number): number {
        return Math.ceil(size / this.section_alignment) * this.section_alignment;
    }

    build(): Uint8Array {
        // 预先计算所有节的虚拟地址和原始数据偏移量
        let current_virtual_address = this.section_alignment; // 从第一个节的对齐地址开始
        let current_raw_offset = this.align_to_file_alignment(this.calculate_headers_size());

        // 临时存储节，以便按顺序处理
        const ordered_sections: PeSection[] = [];

        // 处理标准节
        for (const section of this.sections.values()) {
            section.set_virtual_address(current_virtual_address);
            section.set_raw_offset(current_raw_offset);
            ordered_sections.push(section);

            current_virtual_address += this.align_to_section_alignment(section.get_virtual_size());
            current_raw_offset += this.align_to_file_alignment(section.get_raw_data_size());
        }

        // 处理 .idata 节 (如果存在导入表)
        if (this.import_table.libraries.size > 0) {
            const idata_section_rva = current_virtual_address; // idata 节的 RVA
            const import_table_size = this.import_table.layout(idata_section_rva);

            const idata_section = new PeSection('.idata', 0xc0000040); // 可读、可写、初始化数据
            idata_section.set_virtual_address(idata_section_rva);
            idata_section.set_virtual_size(import_table_size);
            idata_section.set_raw_offset(current_raw_offset);
            this.sections.set('.idata', idata_section); // 添加到 sections 映射
            ordered_sections.push(idata_section);

            // 生成 .idata 节的原始数据
            const idata_raw_data = this.import_table.generate_raw_data(
                idata_section.get_virtual_address()
            );
            idata_section.set_raw_data(idata_raw_data);

            current_virtual_address += this.align_to_section_alignment(
                idata_section.get_virtual_size()
            );
            current_raw_offset += this.align_to_file_alignment(idata_section.get_raw_data_size());

            // 更新导入表节信息
            this.pe_headers.update_import_section(idata_section);
        }

        const writer = new BinaryWriter();
        // 生成DOS头
        this.pe_headers.write_dos_header(writer);
        // 设置 PeHeaders 的公共字段
        this.pe_headers.number_of_sections = this.sections.size;
        this.pe_headers.entry_point_rva = this.sections.get('.text')?.get_virtual_address() ?? 0x1000;
        this.pe_headers.section_alignment = this.section_alignment;
        this.pe_headers.file_alignment = this.file_alignment;
        this.pe_headers.size_of_image = this.calculate_image_size();
        this.pe_headers.size_of_headers = this.calculate_headers_size();
        this.pe_headers.code_size = this.calculate_code_size();
        this.pe_headers.initialized_data_size = this.calculate_initialized_data_size();
        this.pe_headers.uninitialized_data_size = this.calculate_uninitialized_data_size();
        this.pe_headers.code_base_rva = this.sections.get('.text')?.get_virtual_address() || 0;
        this.pe_headers.data_base_rva = this.sections.get('.data')?.get_virtual_address() || 0;
        this.pe_headers.checksum = 0;
        this.pe_headers.subsystem = 3;
        this.pe_headers.dll_characteristics = 0x0060; // NX_COMPAT (0x0040) | NO_SEH (0x0020)

        // 生成NT头
        this.pe_headers.write_nt_headers(writer);

        this.write_sections(writer, ordered_sections);

        return writer.get_bytes();
    }

    private generate_dos_stub(): Uint8Array {
        // 简化的DOS存根程序
        const stub = new Uint8Array([
            0x0e, 0x1f, 0xba, 0x0e, 0x00, 0xb4, 0x09, 0xcd, 0x21, 0xb8, 0x01, 0x4c, 0xcd, 0x21,
            0x54, 0x68, 0x69, 0x73, 0x20, 0x70, 0x72, 0x6f, 0x67, 0x72, 0x61, 0x6d, 0x20, 0x63,
            0x61, 0x6e, 0x6e, 0x6f, 0x74, 0x20, 0x62, 0x65, 0x20, 0x72, 0x75, 0x6e, 0x20, 0x69,
            0x6e, 0x20, 0x44, 0x4f, 0x53, 0x20, 0x6d, 0x6f, 0x64, 0x65, 0x2e, 0x0d, 0x0d, 0x0a,
            0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        return stub;
    }

    private write_sections(writer: BinaryWriter, sections_to_write: PeSection[]) {
        // 生成节头 (使用 ordered_sections)
        for (const section of sections_to_write) {
            // 节名 (8字节)
            const name_bytes = new TextEncoder().encode(section.get_name());
            writer.write_bytes(name_bytes);
            writer.write_bytes(new Uint8Array(8 - name_bytes.length));
            // 虚拟大小
            writer.write_u32(section.get_virtual_size());

            // 虚拟地址
            writer.write_u32(section.get_virtual_address());

            // 原始数据大小
            writer.write_u32(section.get_raw_data_size());

            // 原始数据指针
            writer.write_u32(section.get_raw_offset());

            // 重定位信息
            writer.write_u32(0); // 重定位指针
            writer.write_u32(0); // 行号指针
            writer.write_u16(0); // 重定位数量
            writer.write_u16(0); // 行号数量

            // 特征
            writer.write_u32(section.get_characteristics());
        }
        // 写入节数据 (使用 ordered_sections)
        for (const section of sections_to_write) {
            writer.write_bytes(section.get_raw_data());
        }
    }

    private align_to_file_alignment(size: number): number {
        return Math.ceil(size / this.file_alignment) * this.file_alignment;
    }
}