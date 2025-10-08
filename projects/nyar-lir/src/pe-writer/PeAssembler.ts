import {BinaryWriter} from '../BinaryWriter';
import {PeSection} from "./PeSection";
import {ImportTable} from "./ImportTable";
import {PeTargetArchitecture} from "@/pe-writer/PeTargetArchitecture";

export class PeAssembler {
    private sections: Map<string, PeSection>;
    private architecture: PeTargetArchitecture;
    private base_address: number;
    private file_alignment: number;
    private section_alignment: number;
    private import_table: ImportTable;

    constructor(target_architecture = PeTargetArchitecture.X86) {
        this.sections = new Map();

        this.architecture = target_architecture;
        this.base_address = target_architecture === PeTargetArchitecture.X64 ? 0x140000000 : 0x400000;
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
        this.add_section('.pdata', 0x40000040); // 异常信息(x64)
        this.add_section('.reloc', 0x42000040); // 重定位
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
        const text_section = this.sections.get('.text');
        return text_section ? text_section.get_raw_data_size() : 0;
    }

    get_initialized_data_size(): number {
        const data_section = this.sections.get('.data');
        return data_section ? data_section.get_raw_data_size() : 0;
    }

    get_uninitialized_data_size(): number {
        const bss_section = this.sections.get('.bss');
        return bss_section ? bss_section.get_virtual_size() : 0;
    }

    get_entry_point_rva(): number {
        // 默认从.text节开始
        const text_section = this.sections.get('.text');
        return text_section ? text_section.get_virtual_address() : 0;
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
        return this.sections.size + (this.import_table.libraries.size > 0 ? 1 : 0);
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
        // DOS头 + PE头 + 节头
        return 0x400; // 1KB对齐
    }

    calculate_checksum(): number {
        // 简化的校验和计算
        return 0;
    }

    private align_to_section_alignment(size: number): number {
        return Math.ceil(size / this.section_alignment) * this.section_alignment;
    }

    build(): Uint8Array {
        let import_table_size = 0;
        if (this.import_table.libraries.size > 0) {
            import_table_size = this.import_table.layout(this.architecture);
        }

        // 如果有导入，则添加.idata节
        if (this.import_table.libraries.size > 0) {
            // 先添加节，raw_data 稍后设置
            const idata_section = this.add_section('.idata', 0xc0000040); // 可读、可写、初始化数据
            idata_section.set_virtual_size(import_table_size);
        }

        const writer = new BinaryWriter();

        // 生成DOS头
        const dos_header = this.generate_dos_header();
        writer.write_bytes(dos_header);

        // 生成NT头
        const nt_headers = this.generate_nt_headers();
        writer.write_bytes(nt_headers);

        // 生成节头
        const section_headers = this.generate_section_headers();
        writer.write_bytes(section_headers);

        // 如果有导入，设置 .idata 节的 raw_data
        if (this.import_table.libraries.size > 0) {
            const idata_section = this.sections.get('.idata');
            if (idata_section) {
                const idata_raw_data = this.import_table.generate_raw_data(idata_section.get_virtual_address(), this.base_address);
                idata_section.set_raw_data(idata_raw_data);
            }
        }

        // 写入节数据
        for (const section of this.sections.values()) {
            writer.write_bytes(section.get_raw_data());
        }

        return writer.get_bytes();
    }

    private generate_dos_header(): Uint8Array {
        const writer = new BinaryWriter();

        // DOS MZ头
        writer.write_u16(0x5a4d); // e_magic: 'MZ'
        writer.write_u16(0x0090); // e_cblp
        writer.write_u16(0x0003); // e_cp
        writer.write_u16(0x0000); // e_crlc
        writer.write_u16(0x0004); // e_cparhdr
        writer.write_u16(0x0000); // e_minalloc
        writer.write_u16(0xffff); // e_maxalloc
        writer.write_u16(0x0000); // e_ss
        writer.write_u16(0x00b8); // e_sp
        writer.write_u16(0x0000); // e_csum
        writer.write_u16(0x0000); // e_ip
        writer.write_u16(0x0000); // e_cs
        writer.write_u32(0x00000040); // e_lfarlc
        writer.write_u32(0x00000000); // e_ovno

        // 保留字段
        for (let i = 0; i < 8; i++) {
            writer.write_u16(0);
        }

        writer.write_u32(0x00000074); // e_lfanew (PE头偏移)

        // DOS存根程序
        const stub = this.generate_dos_stub();
        writer.write_bytes(stub);

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

        writer.write_u16(this.get_machine_type());
        writer.write_u16(this.sections.size); // 节数量
        writer.write_u32(Math.floor(Date.now() / 1000)); // 时间戳
        writer.write_u32(0); // 符号表指针
        writer.write_u32(0); // 符号数量
        writer.write_u16(this.get_optional_header_size());
        writer.write_u16(this.get_characteristics());

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
        if (this.architecture === PeTargetArchitecture.X86) {
            characteristics |= 0x0100; // IMAGE_FILE_32BIT_MACHINE
        }
        return characteristics;
    }

    private generate_optional_header(): Uint8Array {
        const writer = new BinaryWriter();

        // 标准字段
        writer.write_u16(this.get_pe_magic());
        writer.write_u8(0); // 主链接器版本
        writer.write_u8(0); // 副链接器版本
        writer.write_u32(this.get_code_size());
        writer.write_u32(this.get_initialized_data_size());
        writer.write_u32(this.get_uninitialized_data_size());
        writer.write_u32(this.get_entry_point_rva());
        writer.write_u32(this.get_code_base_rva());

        if (this.architecture === PeTargetArchitecture.X86) {
            writer.write_u32(this.get_data_base_rva());
        }

        // Windows特定字段
        writer.write_u32(this.base_address); // 映像基址
        writer.write_u32(this.section_alignment);
        writer.write_u32(this.file_alignment);
        writer.write_u16(10); // 主操作系统版本 (Windows 10/11)
        writer.write_u16(0); // 副操作系统版本
        writer.write_u16(0); // 主映像版本
        writer.write_u16(0); // 副映像版本
        writer.write_u16(10); // 主子系统版本
        writer.write_u16(0); // 副子系统版本
        writer.write_u32(0); // Win32版本值

        // 大小字段
        writer.write_u32(this.get_image_size());
        writer.write_u32(this.get_headers_size());
        writer.write_u32(this.calculate_checksum());
        writer.write_u16(this.get_subsystem());
        writer.write_u16(this.get_dll_characteristics());

        // 栈堆大小
        writer.write_u32(0x00100000); // 栈保留大小
        writer.write_u32(0x00001000); // 栈提交大小
        writer.write_u32(0x00100000); // 堆保留大小
        writer.write_u32(0x00001000); // 堆提交大小

        writer.write_u32(0); // 加载器标志
        writer.write_u32(16); // 数据目录数量

        // 数据目录
        this.write_data_directories(writer);

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
        writer.write_u32(0);
        writer.write_u32(0);

        // 证书表
        writer.write_u32(0);
        writer.write_u32(0);

        // 重定位表
        writer.write_u32(0);
        writer.write_u32(0);

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

    private generate_section_headers(): Uint8Array {
        const writer = new BinaryWriter();

        let virtual_address = this.section_alignment;
        let raw_offset = this.align_to_file_alignment(this.get_headers_size());

        // 将导入表节添加到 sections 映射中，以便在生成节头时处理
        if (this.import_table.libraries.size > 0) {
            const idata_section = this.sections.get('.idata');
            if (idata_section) {
                // 确保 .idata 节在迭代器中被处理
                // 这里我们只是确保它存在，实际的 raw_data 已经在 build 方法中设置
            }
        }

        for (const [name, section] of this.sections) {
            section.set_virtual_address(virtual_address);
            section.set_raw_offset(raw_offset);

            // 节名 (8字节)
            const name_bytes = new TextEncoder().encode(name);
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

            virtual_address += this.align_to_section_alignment(section.get_virtual_size());
            raw_offset += this.align_to_file_alignment(section.get_raw_data_size());
        }

        return writer.get_bytes();
    }

    private align_to_file_alignment(size: number): number {
        return Math.ceil(size / this.file_alignment) * this.file_alignment;
    }
}