import {BinaryWriter} from '../BinaryWriter';
import {PeSection} from "./PeSection";
import {ImportTable} from "./ImportTable";
import {PeTargetArchitecture} from "./PeTargetArchitecture";
import {PeHeaders} from "./PeHeaders";

export class PeAssembler {
    private sections: Map<string, PeSection>;
    private architecture: PeTargetArchitecture;
    private base_address: number;
    private file_alignment: number;
    private section_alignment: number;
    private import_table: ImportTable;
    private pe_headers: PeHeaders;

    constructor(target_architecture = PeTargetArchitecture.X86) {
        this.sections = new Map();

        this.architecture = target_architecture;
        this.base_address = target_architecture === PeTargetArchitecture.X64 ? 0x140000000 : 0x400000;
        this.file_alignment = 0x200;
        this.section_alignment = 0x1000;

        this.init_standard_sections();
        this.import_table = new ImportTable();
        this.pe_headers = new PeHeaders();
    }

    private init_standard_sections(): void {
        // 标准PE节
        this.add_section('.text', 0x60000020); // 可执行、可读、代码
        this.add_section('.data', 0xc0000040); // 可读、可写、初始化数据
        this.add_section('.rdata', 0x40000040); // 只读数据
        this.add_section('.pdata', 0xC0000040); // 异常信息(x64)
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

    get_code_size(): number {
        let total_size = 0;
        for (const section of this.sections.values()) {
            if ((section.get_characteristics() & 0x00000020) === 0x00000020) { // IMAGE_SCN_CNT_CODE
                total_size += section.get_raw_data_size();
            }
        }
        return total_size;
    }

    get_initialized_data_size(): number {
        let total_size = 0;
        for (const section of this.sections.values()) {
            if ((section.get_characteristics() & 0x00000040) === 0x00000040) { // IMAGE_SCN_CNT_INITIALIZED_DATA
                total_size += section.get_raw_data_size();
            }
        }
        return total_size;
    }

    get_uninitialized_data_size(): number {
        let total_size = 0;
        for (const section of this.sections.values()) {
            if ((section.get_characteristics() & 0x00000080) === 0x00000080) { // IMAGE_SCN_CNT_UNINITIALIZED_DATA
                total_size += section.get_virtual_size();
            }
        }
        return total_size;
    }

    get_entry_point_rva(): number {
        // For now, hardcode entry point RVA to match hello-world-x64.js
        return 0x1000;
    }

    get_code_base_rva(): number {
        const text_section = this.sections.get('.text');
        return text_section ? text_section.get_virtual_address() : 0;
    }

    get_data_base_rva(): number {
        const data_section = this.sections.get('.data');
        return data_section ? data_section.get_virtual_address() : 0;
    }

    get_sections_count(): number {
        return this.sections.size;
    }

    get_architecture(): PeTargetArchitecture {
        return this.architecture;
    }

    get_base_address(): number {
        return this.base_address;
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

    get_image_size(): number {
        let image_size = 0;
        for (const section of this.sections.values()) {
            const end_address = section.get_virtual_address() + section.get_virtual_size();
            image_size = Math.max(image_size, end_address);
        }
        return this.align_to_section_alignment(image_size);
    }

    get_headers_size(): number {
        let size = 64; // DOS Header
        size += this.generate_dos_stub().length; // DOS Stub
        size += 4; // PE Signature
        size += 20; // File Header
        size += this.get_optional_header_size(); // Optional Header
        size += this.get_sections_count() * 40; // Section Headers (40 bytes per section)
        return this.align_to_file_alignment(size);
    }

    calculate_checksum(): number {
        // 简化的校验和计算
        return 0;
    }

    private align_to_section_alignment(size: number): number {
        return Math.ceil(size / this.section_alignment) * this.section_alignment;
    }

    build(): Uint8Array {
        // 预先计算所有节的虚拟地址和原始数据偏移量
        let current_virtual_address = this.section_alignment; // 从第一个节的对齐地址开始
        let current_raw_offset = this.align_to_file_alignment(this.get_headers_size());

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
            const idata_raw_data = this.import_table.generate_raw_data(idata_section.get_virtual_address(), this.base_address);
            idata_section.set_raw_data(idata_raw_data);

            current_virtual_address += this.align_to_section_alignment(idata_section.get_virtual_size());
            current_raw_offset += this.align_to_file_alignment(idata_section.get_raw_data_size());
        }

        const writer = new BinaryWriter();

        // 生成DOS头
        const dos_header = this.generate_dos_header();
        writer.write_bytes(dos_header);

        // 生成NT头
        const nt_headers = this.generate_nt_headers();
        writer.write_bytes(nt_headers);

        // 生成节头 (使用 ordered_sections)
        const section_headers = this.generate_section_headers(ordered_sections);
        writer.write_bytes(section_headers);

        // 写入节数据 (使用 ordered_sections)
        for (const section of ordered_sections) {
            writer.write_bytes(section.get_raw_data());
        }

        return writer.get_bytes();
    }

    private generate_dos_header(): Uint8Array {
        return this.pe_headers.generate_dos_header();
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

    private generate_nt_headers(): Uint8Array {
        const writer = new BinaryWriter();

        // PE签名
        writer.write_u32(0x00004550); // 'PE\0\0'

        // 文件头
        const file_header = this.generate_file_header();
        writer.write_bytes(file_header);

        // 可选头
        const optional_header = this.generate_optional_header();
        writer.write_bytes(optional_header);

        return writer.get_bytes();
    }

    private generate_file_header(): Uint8Array {
        const writer = new BinaryWriter();

        const machineType = this.get_machine_type();
        const numberOfSections = this.sections.size;
        const optionalHeaderSize = this.get_optional_header_size();
        const characteristics = this.get_characteristics();

        writer.write_u16(machineType);
        writer.write_u16(numberOfSections); // 节数量
        writer.write_u32(Math.floor(Date.now() / 1000)); // 时间戳
        writer.write_u32(0); // 符号表指针
        writer.write_u32(0); // 符号数量
        writer.write_u16(optionalHeaderSize);
        writer.write_u16(characteristics);

        console.log(`[PeAssembler] File Header - Machine: 0x${machineType.toString(16)}, NumberOfSections: ${numberOfSections}, Characteristics: 0x${characteristics.toString(16)}`);

        return writer.get_bytes();
    }

    private get_machine_type(): number {
        return this.architecture === PeTargetArchitecture.X64 ? 0x8664 : 0x014c;
    }

    private get_optional_header_size(): number {
        return this.architecture === PeTargetArchitecture.X64 ? 240 : 224;
    }

    private get_characteristics(): number {
        let characteristics = 0x0002; // IMAGE_FILE_EXECUTABLE_IMAGE
        characteristics |= 0x0001; // IMAGE_FILE_RELOCS_STRIPPED
        if (this.architecture === PeTargetArchitecture.X86) {
            characteristics |= 0x0100; // IMAGE_FILE_32BIT_MACHINE
        } else if (this.architecture === PeTargetArchitecture.X64) {
            characteristics |= 0x0020; // IMAGE_FILE_LARGE_ADDRESS_AWARE
        }
        return characteristics;
    }

    private generate_optional_header(): Uint8Array {
        const writer = new BinaryWriter();

        const magic = this.pe_headers.get_pe_magic(this.architecture);
        const address_of_entry_point = this.get_entry_point_rva();
        const image_base = this.base_address;
        const section_alignment = this.section_alignment;
        const file_alignment = this.file_alignment;
        const size_of_image = this.get_image_size();
        const size_of_headers = this.get_headers_size();

        // 标准字段
        writer.write_u16(magic);
        writer.write_u8(0); // 主链接器版本
        writer.write_u8(0); // 副链接器版本
        writer.write_u32(this.get_code_size());
        writer.write_u32(this.get_initialized_data_size());
        writer.write_u32(this.get_uninitialized_data_size());
        writer.write_u32(address_of_entry_point);
        writer.write_u32(this.get_code_base_rva());

        if (this.architecture === PeTargetArchitecture.X86) {
            writer.write_u32(this.get_data_base_rva());
        }

        // Windows特定字段
        if (this.architecture === PeTargetArchitecture.X64) {
            writer.write_u64(BigInt(image_base));
        } else {
            writer.write_u32(image_base);
        }
        writer.write_u32(section_alignment);
        writer.write_u32(file_alignment);
        writer.write_u16(10); // 主操作系统版本 (Windows 10/11)
        writer.write_u16(0); // 副操作系统版本
        writer.write_u16(0); // 主映像版本
        writer.write_u16(0); // 副映像版本
        writer.write_u16(10); // 主子系统版本
        writer.write_u16(0); // 副子系统版本
        writer.write_u32(0); // Win32版本值

        // 大小字段
        writer.write_u32(size_of_image);
        writer.write_u32(size_of_headers);
        writer.write_u32(this.calculate_checksum());
        writer.write_u16(this.get_subsystem());
        writer.write_u16(this.get_dll_characteristics());

        // 栈堆大小
        if (this.architecture === PeTargetArchitecture.X64) {
            writer.write_u64(BigInt(0x00100000)); // 栈保留大小
            writer.write_u64(BigInt(0x00001000)); // 栈提交大小
            writer.write_u64(BigInt(0x00100000)); // 堆保留大小
            writer.write_u64(BigInt(0x00001000)); // 堆提交大小
        } else {
            writer.write_u32(0x00100000); // 栈保留大小
            writer.write_u32(0x00001000); // 栈提交大小
            writer.write_u32(0x00100000); // 堆保留大小
            writer.write_u32(0x00001000); // 堆提交大小
        }

        writer.write_u32(0); // 加载器标志
        writer.write_u32(16); // 数据目录数量

        // 数据目录
        this.write_data_directories(writer);

        console.log(`[PeAssembler] Optional Header - Magic: 0x${magic.toString(16)}, EntryPoint: 0x${address_of_entry_point.toString(16)}, ImageBase: 0x${image_base.toString(16)}, SectionAlignment: 0x${section_alignment.toString(16)}, FileAlignment: 0x${file_alignment.toString(16)}, SizeOfImage: 0x${size_of_image.toString(16)}, SizeOfHeaders: 0x${size_of_headers.toString(16)}`);

        return writer.get_bytes();
    }

    private get_pe_magic(): number {
        return this.architecture === PeTargetArchitecture.X64 ? 0x020b : 0x010b;
    }

    private get_subsystem(): number {
        return 3; // IMAGE_SUBSYSTEM_WINDOWS_CUI (控制台应用)
    }

    private get_dll_characteristics(): number {
        let characteristics = 0x0080; // IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE
        characteristics |= 0x0040; // IMAGE_DLLCHARACTERISTICS_NX_COMPAT
        if (this.architecture === PeTargetArchitecture.X64) {
            characteristics |= 0x0020; // IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA
        }
        return characteristics;
    }

    private write_data_directories(writer: BinaryWriter): void {
        // 导出表
        writer.write_u32(0);
        writer.write_u32(0);

        // 导入表
        writer.write_u32(this.import_table.get_import_directory_rva());
        writer.write_u32(this.import_table.get_import_directory_size());

        // 资源表
        writer.write_u32(0);
        writer.write_u32(0);

        // 异常表 (x64重要)
        const pdata_section = this.sections.get('.pdata');
        if (pdata_section) {
            writer.write_u32(pdata_section.get_virtual_address());
            writer.write_u32(pdata_section.get_virtual_size());
        } else {
            writer.write_u32(0);
            writer.write_u32(0);
        }

        // 证书表
        writer.write_u32(0);
        writer.write_u32(0);

        // 重定位表
        const reloc_section = this.sections.get('.reloc');
        if (reloc_section) {
            writer.write_u32(reloc_section.get_virtual_address());
            writer.write_u32(reloc_section.get_virtual_size());
        } else {
            writer.write_u32(0);
            writer.write_u32(0);
        }

        // 调试信息
        writer.write_u32(0);
        writer.write_u32(0);

        // 架构特定数据
        writer.write_u32(0);
        writer.write_u32(0);

        // 全局指针
        writer.write_u32(0);
        writer.write_u32(0);

        // TLS表
        writer.write_u32(0);
        writer.write_u32(0);

        // 加载配置表
        writer.write_u32(0);
        writer.write_u32(0);

        // 绑定导入表
        writer.write_u32(0);
        writer.write_u32(0);

        // IAT表
        writer.write_u32(0);
        writer.write_u32(0);

        // 延迟导入描述符
        writer.write_u32(0);
        writer.write_u32(0);

        // CLR运行时头部
        writer.write_u32(0);
        writer.write_u32(0);

        // 保留
        writer.write_u32(0);
        writer.write_u32(0);
    }

    private generate_section_headers(sections_to_write: PeSection[]): Uint8Array {
        const writer = new BinaryWriter();

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

        return writer.get_bytes();
    }

    private align_to_file_alignment(size: number): number {
        return Math.ceil(size / this.file_alignment) * this.file_alignment;
    }
}