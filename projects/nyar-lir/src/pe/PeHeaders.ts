import { BinaryWriter } from '../BinaryWriter.js';

export class PeHeaders {
    generate_dos_header(): Uint8Array {
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

        writer.write_u32(0x00000080); // e_lfanew (PE头偏移)

        // DOS存根程序
        const stub = this.generate_dos_stub();
        writer.write_bytes(stub);

        return writer.get_bytes();
    }

    generate_nt_headers(pe_builder: ProfessionalPeBuilder): Uint8Array {
        const writer = new BinaryWriter();

        // PE签名
        writer.write_u32(0x00004550); // 'PE\0\0'

        // 文件头
        const file_header = this.generate_file_header(pe_builder);
        writer.write_bytes(file_header);

        // 可选头
        const optional_header = this.generate_optional_header(pe_builder);
        writer.write_bytes(optional_header);

        return writer.get_bytes();
    }

    generate_file_header(pe_builder: ProfessionalPeBuilder): Uint8Array {
        const writer = new BinaryWriter();

        writer.write_u16(this.get_machine_type(pe_builder.get_architecture()));
        writer.write_u16(pe_builder.get_sections_count()); // 节数量
        writer.write_u32(Math.floor(Date.now() / 1000)); // 时间戳
        writer.write_u32(0); // 符号表指针
        writer.write_u32(0); // 符号数量
        writer.write_u16(this.get_optional_header_size(pe_builder.get_architecture()));
        writer.write_u16(this.get_characteristics(pe_builder.get_architecture()));

        return writer.get_bytes();
    }

    generate_optional_header(pe_builder: ProfessionalPeBuilder): Uint8Array {
        const writer = new BinaryWriter();

        // 标准字段
        writer.write_u16(this.get_pe_magic(pe_builder.get_architecture()));
        writer.write_u8(0); // 主链接器版本
        writer.write_u8(0); // 副链接器版本
        writer.write_u32(pe_builder.get_code_size());
        writer.write_u32(pe_builder.get_initialized_data_size());
        writer.write_u32(pe_builder.get_uninitialized_data_size());
        writer.write_u32(pe_builder.get_entry_point_rva());
        writer.write_u32(pe_builder.get_code_base_rva());

        if (pe_builder.get_architecture() === 'x86') {
            writer.write_u32(pe_builder.get_data_base_rva());
        }

        // Windows特定字段
        writer.write_u32(pe_builder.get_base_address()); // 映像基址
        writer.write_u32(pe_builder.get_section_alignment());
        writer.write_u32(pe_builder.get_file_alignment());
        writer.write_u16(6); // 主操作系统版本 (Windows 10)
        writer.write_u16(0); // 副操作系统版本
        writer.write_u16(0); // 主映像版本
        writer.write_u16(0); // 副映像版本
        writer.write_u16(5); // 主子系统版本
        writer.write_u16(0); // 副子系统版本
        writer.write_u32(0); // Win32版本值

        // 大小字段
        writer.write_u32(pe_builder.get_image_size());
        writer.write_u32(pe_builder.get_headers_size());
        writer.write_u32(pe_builder.calculate_checksum());
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
        this.write_data_directories(writer, pe_builder);

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

    private get_machine_type(architecture: string): number {
        return architecture === 'x64' ? 0x8664 : 0x014c;
    }

    private get_optional_header_size(architecture: string): number {
        return architecture === 'x64' ? 240 : 224;
    }

    private get_characteristics(architecture: string): number {
        let characteristics = 0x0002; // IMAGE_FILE_EXECUTABLE_IMAGE
        if (architecture === 'x86') {
            characteristics |= 0x0100; // IMAGE_FILE_32BIT_MACHINE
        }
        return characteristics;
    }

    private get_pe_magic(architecture: string): number {
        return architecture === 'x64' ? 0x020b : 0x010b;
    }

    private get_subsystem(): number {
        return 3; // IMAGE_SUBSYSTEM_WINDOWS_CUI (控制台应用)
    }

    private get_dll_characteristics(): number {
        let characteristics = 0x0080; // IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE
        characteristics |= 0x0040; // IMAGE_DLLCHARACTERISTICS_NX_COMPAT
        return characteristics;
    }

    private write_data_directories(writer: BinaryWriter, pe_builder: ProfessionalPeBuilder): void {
        // 导出表
        const export_table = pe_builder.get_exports().get_directory_entry();
        writer.write_u32(export_table.rva);
        writer.write_u32(export_table.size);

        // 导入表
        const import_table = pe_builder.get_imports().get_directory_entry();
        writer.write_u32(import_table.rva);
        writer.write_u32(import_table.size);

        // 资源表
        const resource_table = pe_builder.get_resources().get_directory_entry();
        writer.write_u32(resource_table.rva);
        writer.write_u32(resource_table.size);

        // 异常表 (x64重要)
        const exception_table = pe_builder.get_exception_directory();
        writer.write_u32(exception_table.rva);
        writer.write_u32(exception_table.size);

        // 证书表
        writer.write_u32(0);
        writer.write_u32(0);

        // 重定位表
        const reloc_table = pe_builder.get_relocations().get_directory_entry();
        writer.write_u32(reloc_table.rva);
        writer.write_u32(reloc_table.size);

        // 调试信息
        const debug_table = pe_builder.get_debug_info().get_directory_entry();
        writer.write_u32(debug_table.rva);
        writer.write_u32(debug_table.size);

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
}

// 前向声明，避免循环依赖
interface ProfessionalPeBuilder {
    get_architecture(): string;
    get_sections_count(): number;
    get_base_address(): number;
    get_section_alignment(): number;
    get_file_alignment(): number;
    get_exports(): ExportTable;
    get_imports(): ImportTable;
    get_resources(): ResourceTable;
    get_relocations(): RelocationTable;
    get_debug_info(): DebugInfo;
    get_exception_directory(): DirectoryEntry;
}

interface DirectoryEntry {
    rva: number;
    size: number;
}

interface ExportTable {
    get_directory_entry(): DirectoryEntry;
}

interface ImportTable {
    get_directory_entry(): DirectoryEntry;
}

interface ResourceTable {
    get_directory_entry(): DirectoryEntry;
}

interface RelocationTable {
    get_directory_entry(): DirectoryEntry;
}

interface DebugInfo {
    get_directory_entry(): DirectoryEntry;
}
