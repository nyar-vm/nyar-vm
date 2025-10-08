import {BinaryWriter} from '../BinaryWriter';

import {PeTargetArchitecture} from '@/pe-writer/PeTargetArchitecture';

export class PeHeaders {
    public readonly architecture: PeTargetArchitecture;
    public number_of_sections: number = 0;
    public optional_header_size: number = 0;
    public characteristics: number = 0;
    public entry_point_rva: number = 0;
    public image_base: number = 0;
    public section_alignment: number = 0;
    public file_alignment: number = 0;
    public size_of_image: number = 0;
    public size_of_headers: number = 0;
    public code_size: number = 0;
    public initialized_data_size: number = 0;
    public uninitialized_data_size: number = 0;
    public code_base_rva: number = 0;
    public data_base_rva: number = 0;
    public checksum: number = 0;
    public subsystem: number = 0;
    public dll_characteristics: number = 0;

    constructor(architecture: PeTargetArchitecture) {
        this.architecture = architecture;
    }

    public write_dos_header(writer: BinaryWriter): void {
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

    }

    public write_nt_headers(writer: BinaryWriter) {
        writer.write_u32(0x00004550); // "PE\0\0"
        this.write_file_header(writer);
        this.write_optional_header(writer);
    }

    public write_file_header(writer: BinaryWriter) {
        const machine_type = this.get_machine_type(this.architecture);

        writer.write_u16(machine_type);
        writer.write_u16(this.number_of_sections); // 节数量
        writer.write_u32(Math.floor(Date.now() / 1000)); // 时间戳
        writer.write_u32(0); // 符号表指针
        writer.write_u32(0); // 符号数量
        writer.write_u16(this.optional_header_size);
        writer.write_u16(this.characteristics);

        console.log(
            `[PeHeaders] File Header - Machine: 0x${machine_type.toString(16)}, NumberOfSections: ${this.number_of_sections}, Characteristics: 0x${this.characteristics.toString(16)}`
        );
    }

    public write_optional_header(writer: BinaryWriter) {
        const magic = this.get_pe_magic();
        // 标准字段
        writer.write_u16(magic);
        writer.write_u8(0); // 主链接器版本
        writer.write_u8(0); // 副链接器版本
        writer.write_u32(this.code_size);
        writer.write_u32(this.initialized_data_size);
        writer.write_u32(this.uninitialized_data_size);
        writer.write_u32(this.entry_point_rva);
        writer.write_u32(this.code_base_rva);

        if (this.architecture === PeTargetArchitecture.X86) {
            writer.write_u32(this.data_base_rva);
        }

        // Windows特定字段
        if (this.architecture === PeTargetArchitecture.X64) {
            writer.write_u64(BigInt(this.image_base));
        } else {
            writer.write_u32(this.image_base);
        }
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
        writer.write_u32(this.size_of_image);
        writer.write_u32(this.size_of_headers);
        writer.write_u32(this.checksum);
        writer.write_u16(this.subsystem);
        writer.write_u16(this.dll_characteristics);

        // 栈堆大小
        if (this.architecture === PeTargetArchitecture.X64) {
            writer.write_u64(BigInt(0x00100000)); // 栈保留大小
            writer.write_u64(BigInt(0x00001000)); // 栈提交大小
        } else {
            writer.write_u32(0x00100000); // 栈保留大小
            writer.write_u32(0x00001000); // 栈提交大小
        }
        writer.write_u32(0x00100000); // 堆保留大小
        writer.write_u32(0x00001000); // 堆提交大小
        writer.write_u32(0); // 加载器标志
        writer.write_u32(16); // 数据目录数量

        // 数据目录 (16个，每个8字节)
        // 导出表
        writer.write_u32(0); // RVA
        writer.write_u32(0); // Size
        // 导入表
        writer.write_u32(0); // RVA - 占位符，实际值在 build() 中设置
        writer.write_u32(0); // Size - 占位符，实际值在 build() 中设置
        // 资源表
        writer.write_u32(0);
        writer.write_u32(0);
        // 异常表
        writer.write_u32(0);
        writer.write_u32(0);
        // 安全目录
        writer.write_u32(0);
        writer.write_u32(0);
        // 重定位表
        writer.write_u32(0);
        writer.write_u32(0);
        // 调试目录
        writer.write_u32(0);
        writer.write_u32(0);
        // 版权所有
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
        // 绑定导入
        writer.write_u32(0);
        writer.write_u32(0);
        // IAT
        writer.write_u32(0);
        writer.write_u32(0);
        // 延迟导入描述符
        writer.write_u32(0);
        writer.write_u32(0);
        // COM描述符
        writer.write_u32(0);
        writer.write_u32(0);
        // 保留
        writer.write_u32(0);
        writer.write_u32(0);

        console.log(
            `[PeHeaders] Optional Header - Magic: 0x${magic.toString(16)}, EntryPoint: 0x${this.entry_point_rva.toString(16)}, ImageBase: 0x${this.image_base.toString(16)}, SectionAlignment: 0x${this.section_alignment.toString(16)}, FileAlignment: 0x${this.file_alignment.toString(16)}, SizeOfImage: 0x${this.size_of_image.toString(16)}, SizeOfHeaders: 0x${this.size_of_headers.toString(16)}`
        );
    }

    public get_pe_magic(): number {
        return this.architecture === PeTargetArchitecture.X64 ? 0x020b : 0x010b;
    }

    private get_machine_type(architecture: PeTargetArchitecture): number {
        return architecture === PeTargetArchitecture.X64 ? 0x8664 : 0x014c;
    }
}
