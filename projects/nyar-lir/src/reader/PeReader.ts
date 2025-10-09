import {PeAssembler} from "@/pe-writer/PeAssembler";

export interface DosHeader {
    e_magic: number;        // MZ头标识
    e_cblp: number;         // 最后页的字节数
    e_cp: number;           // 文件中的页数
    e_crlc: number;         // 重定位项数
    e_cparhdr: number;      // 头部段落数
    e_minalloc: number;     // 需要的最小额外段落数
    e_maxalloc: number;     // 需要的最大额外段落数
    e_ss: number;           // 初始SS值
    e_sp: number;           // 初始SP值
    e_csum: number;         // 校验和
    e_ip: number;           // 初始IP值
    e_cs: number;           // 初始CS值
    e_lfarlc: number;       // 重定位表的文件地址
    e_ovno: number;         // 覆盖号
    e_res: number[];        // 保留字
    e_oemid: number;        // OEM标识符
    e_oeminfo: number;      // OEM信息
    e_res2: number[];       // 保留字
    e_lfanew: number;       // PE头偏移
}

export interface FileHeader {
    machine: number;                // 目标机器类型
    number_of_sections: number;     // 节数量
    time_date_stamp: number;        // 时间戳
    pointer_to_symbol_table: number; // 符号表指针
    number_of_symbols: number;      // 符号数量
    size_of_optional_header: number; // 可选头大小
    characteristics: number;       // 特征值
}

export interface OptionalHeader {
    magic: number;                      // 魔数
    major_linker_version: number;       // 链接器主版本号
    minor_linker_version: number;       // 链接器次版本号
    size_of_code: number;               // 代码段大小
    size_of_initialized_data: number;   // 已初始化数据大小
    size_of_uninitialized_data: number; // 未初始化数据大小
    address_of_entry_point: number;    // 入口点地址
    base_of_code: number;               // 代码基址
    base_of_data: number;               // 数据基址（仅32位）
    image_base: number;                 // 镜像基址
    section_alignment: number;          // 段对齐
    file_alignment: number;             // 文件对齐
    major_operating_system_version: number; // 操作系统主版本号
    minor_operating_system_version: number; // 操作系统次版本号
    major_image_version: number;        // 镜像主版本号
    minor_image_version: number;        // 镜像次版本号
    major_subsystem_version: number;    // 子系统主版本号
    minor_subsystem_version: number;    // 子系统次版本号
    win32_version_value: number;        // Win32版本值
    size_of_image: number;              // 镜像大小
    size_of_headers: number;            // 头部大小
    checksum: number;                   // 校验和
    subsystem: number;                   // 子系统
    dll_characteristics: number;        // DLL特征
    size_of_stack_reserve: number;       // 栈保留大小
    size_of_stack_commit: number;        // 栈提交大小
    size_of_heap_reserve: number;       // 堆保留大小
    size_of_heap_commit: number;        // 堆提交大小
    loader_flags: number;               // 加载器标志
    number_of_rva_and_sizes: number;    // RVA和数量
    data_directories: DataDirectory[];  // 数据目录
}

export interface DataDirectory {
    virtual_address: number;    // 虚拟地址
    size: number;               // 大小
}

export interface SectionHeader {
    name: string;                // 节名称
    virtual_size: number;       // 虚拟大小
    virtual_address: number;    // 虚拟地址
    size_of_raw_data: number;   // 原始数据大小
    pointer_to_raw_data: number; // 原始数据指针
    pointer_to_relocations: number; // 重定位指针
    pointer_to_line_numbers: number; // 行号指针
    number_of_relocations: number; // 重定位数量
    number_of_line_numbers: number; // 行号数量
    characteristics: number;    // 特征值
}

export class PeReader {
    private data: Uint8Array;
    private view: DataView;
    
    private dos_header: DosHeader | null = null;
    private file_header: FileHeader | null = null;
    private optional_header: OptionalHeader | null = null;
    private section_headers: SectionHeader[] = [];
    
    constructor(data: Uint8Array | ArrayBuffer | File) {
        if (data instanceof File) {
            throw new Error('File input not implemented yet');
        }
        
        if (data instanceof ArrayBuffer) {
            this.data = new Uint8Array(data);
        } else {
            this.data = data;
        }
        
        this.view = new DataView(this.data.buffer);
    }
    
    public read(): PeAssembler {
        this.parse_dos_header();
        this.parse_nt_headers();
        this.parse_section_headers();
        
        // 创建 PeAssembler 实例并填充解析的数据
        const pe_assembler = new PeAssembler(this.get_target_architecture() as any);
        
        // 这里可以添加更多的逻辑来将解析的数据填充到 PeAssembler 中
        
        return pe_assembler;
    }
    
    private parse_dos_header(): void {
        if (this.data.length < 64) {
            throw new Error('File too small to be a valid PE file');
        }
        
        // 检查 MZ 标识
        if (this.data[0] !== 0x4D || this.data[1] !== 0x5A) {
            throw new Error('Invalid DOS header signature');
        }
        
        this.dos_header = {
            e_magic: this.read_u16(0),
            e_cblp: this.read_u16(2),
            e_cp: this.read_u16(4),
            e_crlc: this.read_u16(6),
            e_cparhdr: this.read_u16(8),
            e_minalloc: this.read_u16(10),
            e_maxalloc: this.read_u16(12),
            e_ss: this.read_u16(14),
            e_sp: this.read_u16(16),
            e_csum: this.read_u16(18),
            e_ip: this.read_u16(20),
            e_cs: this.read_u16(22),
            e_lfarlc: this.read_u16(24),
            e_ovno: this.read_u16(26),
            e_res: [
                this.read_u16(28),
                this.read_u16(30),
                this.read_u16(32),
                this.read_u16(34)
            ],
            e_oemid: this.read_u16(36),
            e_oeminfo: this.read_u16(38),
            e_res2: [
                this.read_u16(40),
                this.read_u16(42),
                this.read_u16(44),
                this.read_u16(46),
                this.read_u16(48),
                this.read_u16(50),
                this.read_u16(52),
                this.read_u16(54),
                this.read_u16(56),
                this.read_u16(58)
            ],
            e_lfanew: this.read_u32(60)
        };
    }
    
    private parse_nt_headers(): void {
        if (!this.dos_header) {
            throw new Error('DOS header not parsed');
        }
        
        const pe_offset = this.dos_header.e_lfanew;
        
        if (this.data.length < pe_offset + 24) {
            throw new Error('File too small to contain NT headers');
        }
        
        // 检查 PE 标识
        if (this.data[pe_offset] !== 0x50 || 
            this.data[pe_offset + 1] !== 0x45 || 
            this.data[pe_offset + 2] !== 0x00 || 
            this.data[pe_offset + 3] !== 0x00) {
            throw new Error('Invalid NT header signature');
        }
        
        // 解析文件头
        this.file_header = {
            machine: this.read_u16(pe_offset + 4),
            number_of_sections: this.read_u16(pe_offset + 6),
            time_date_stamp: this.read_u32(pe_offset + 8),
            pointer_to_symbol_table: this.read_u32(pe_offset + 12),
            number_of_symbols: this.read_u32(pe_offset + 16),
            size_of_optional_header: this.read_u16(pe_offset + 20),
            characteristics: this.read_u16(pe_offset + 22)
        };
        
        // 解析可选头
        const optional_header_offset = pe_offset + 24;
        const magic = this.read_u16(optional_header_offset);
        
        if (magic === 0x10B) { // PE32
            this.parse_optional_header_32(optional_header_offset);
        } else if (magic === 0x20B) { // PE32+
            this.parse_optional_header_64(optional_header_offset);
        } else {
            throw new Error(`Invalid optional header magic: 0x${magic.toString(16)}`);
        }
    }
    
    private parse_optional_header_32(offset: number): void {
        this.optional_header = {
            magic: this.read_u16(offset),
            major_linker_version: this.read_u8(offset + 2),
            minor_linker_version: this.read_u8(offset + 3),
            size_of_code: this.read_u32(offset + 4),
            size_of_initialized_data: this.read_u32(offset + 8),
            size_of_uninitialized_data: this.read_u32(offset + 12),
            address_of_entry_point: this.read_u32(offset + 16),
            base_of_code: this.read_u32(offset + 20),
            base_of_data: this.read_u32(offset + 24),
            image_base: this.read_u32(offset + 28),
            section_alignment: this.read_u32(offset + 32),
            file_alignment: this.read_u32(offset + 36),
            major_operating_system_version: this.read_u16(offset + 40),
            minor_operating_system_version: this.read_u16(offset + 42),
            major_image_version: this.read_u16(offset + 44),
            minor_image_version: this.read_u16(offset + 46),
            major_subsystem_version: this.read_u16(offset + 48),
            minor_subsystem_version: this.read_u16(offset + 50),
            win32_version_value: this.read_u32(offset + 52),
            size_of_image: this.read_u32(offset + 56),
            size_of_headers: this.read_u32(offset + 60),
            checksum: this.read_u32(offset + 64),
            subsystem: this.read_u16(offset + 68),
            dll_characteristics: this.read_u16(offset + 70),
            size_of_stack_reserve: this.read_u32(offset + 72),
            size_of_stack_commit: this.read_u32(offset + 76),
            size_of_heap_reserve: this.read_u32(offset + 80),
            size_of_heap_commit: this.read_u32(offset + 84),
            loader_flags: this.read_u32(offset + 88),
            number_of_rva_and_sizes: this.read_u32(offset + 92),
            data_directories: this.parse_data_directories(offset + 96)
        };
    }
    
    private parse_optional_header_64(offset: number): void {
        this.optional_header = {
            magic: this.read_u16(offset),
            major_linker_version: this.read_u8(offset + 2),
            minor_linker_version: this.read_u8(offset + 3),
            size_of_code: this.read_u32(offset + 4),
            size_of_initialized_data: this.read_u32(offset + 8),
            size_of_uninitialized_data: this.read_u32(offset + 12),
            address_of_entry_point: this.read_u32(offset + 16),
            base_of_code: this.read_u32(offset + 20),
            base_of_data: 0, // PE64+ 没有这个字段
            image_base: this.read_u64(offset + 24),
            section_alignment: this.read_u32(offset + 32),
            file_alignment: this.read_u32(offset + 36),
            major_operating_system_version: this.read_u16(offset + 40),
            minor_operating_system_version: this.read_u16(offset + 42),
            major_image_version: this.read_u16(offset + 44),
            minor_image_version: this.read_u16(offset + 46),
            major_subsystem_version: this.read_u16(offset + 48),
            minor_subsystem_version: this.read_u16(offset + 50),
            win32_version_value: this.read_u32(offset + 52),
            size_of_image: this.read_u32(offset + 56),
            size_of_headers: this.read_u32(offset + 60),
            checksum: this.read_u32(offset + 64),
            subsystem: this.read_u16(offset + 68),
            dll_characteristics: this.read_u16(offset + 70),
            size_of_stack_reserve: this.read_u64(offset + 72),
            size_of_stack_commit: this.read_u64(offset + 80),
            size_of_heap_reserve: this.read_u64(offset + 88),
            size_of_heap_commit: this.read_u64(offset + 96),
            loader_flags: this.read_u32(offset + 104),
            number_of_rva_and_sizes: this.read_u32(offset + 108),
            data_directories: this.parse_data_directories(offset + 112)
        };
    }
    
    private parse_data_directories(offset: number): DataDirectory[] {
        const directories: DataDirectory[] = [];
        const count = 16; // 标准PE文件有16个数据目录
        
        for (let i = 0; i < count; i++) {
            directories.push({
                virtual_address: this.read_u32(offset + i * 8),
                size: this.read_u32(offset + i * 8 + 4)
            });
        }
        
        return directories;
    }
    
    private parse_section_headers(): void {
        if (!this.file_header || !this.optional_header) {
            throw new Error('NT headers not parsed');
        }
        
        const section_header_offset = this.dos_header!.e_lfanew + 4 + 20 + this.file_header.size_of_optional_header;
        
        for (let i = 0; i < this.file_header.number_of_sections; i++) {
            const offset = section_header_offset + i * 40;
            
            // 读取节名称（8字节，可能不是以null结尾）
            const name_bytes = this.data.slice(offset, offset + 8);
            const name = String.fromCharCode(...name_bytes).replace(/\0+$/, '');
            
            this.section_headers.push({
                name,
                virtual_size: this.read_u32(offset + 8),
                virtual_address: this.read_u32(offset + 12),
                size_of_raw_data: this.read_u32(offset + 16),
                pointer_to_raw_data: this.read_u32(offset + 20),
                pointer_to_relocations: this.read_u32(offset + 24),
                pointer_to_line_numbers: this.read_u32(offset + 28),
                number_of_relocations: this.read_u16(offset + 32),
                number_of_line_numbers: this.read_u16(offset + 34),
                characteristics: this.read_u32(offset + 36)
            });
        }
    }
    
    private get_target_architecture(): string {
        if (!this.file_header) {
            throw new Error('File header not parsed');
        }
        
        switch (this.file_header.machine) {
            case 0x014c: // IMAGE_FILE_MACHINE_I386
                return 'x86';
            case 0x8664: // IMAGE_FILE_MACHINE_AMD64
                return 'x64';
            default:
                throw new Error(`Unsupported machine type: 0x${this.file_header.machine.toString(16)}`);
        }
    }
    
    private read_u8(offset: number): number {
        return this.data[offset];
    }
    
    private read_u16(offset: number): number {
        return this.view.getUint16(offset, true);
    }
    
    private read_u32(offset: number): number {
        return this.view.getUint32(offset, true);
    }
    
    private read_u64(offset: number): number {
        // 简化处理，只返回低32位
        return this.view.getUint32(offset, true);
    }
    
    // 获取解析后的头部信息
    public get_dos_header(): DosHeader | null {
        return this.dos_header;
    }
    
    public get_file_header(): FileHeader | null {
        return this.file_header;
    }
    
    public get_optional_header(): OptionalHeader | null {
        return this.optional_header;
    }
    
    public get_section_headers(): SectionHeader[] {
        return this.section_headers;
    }
}