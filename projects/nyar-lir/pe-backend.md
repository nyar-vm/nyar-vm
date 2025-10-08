# JavaScript静态子集到原生EXE编译器开发计划
## 专注于PE文件格式、x86/x64架构的后端实现

## 执行摘要

本计划专注于编译器后端实现，特别是PE文件格式生成和x86/x64机器码生成。前端将使用现有JavaScript解析库，专注于构建高效、可扩展的后端架构。

**核心目标**：构建工业级的PE文件生成器和x86/x64代码生成器，支持完整的Windows可执行文件生成。

## 阶段一：PE文件格式深度实现 (第1-3个月)

### 1.1 专业级PE构建器 (第1-4周)

**目标**：实现符合Microsoft PE/COFF规范的完整PE文件生成器

#### 1.1.1 增强PE构建器架构

```javascript
class ProfessionalPeBuilder {
    constructor(target_architecture = 'x86') {
        this.sections = new Map();
        this.symbol_table = new SymbolTable();
        this.relocations = new RelocationTable();
        this.imports = new ImportTable();
        this.exports = new ExportTable();
        this.resources = new ResourceTable();
        this.debug_info = new DebugInfo();
        
        this.architecture = target_architecture;
        this.base_address = target_architecture === 'x64' ? 0x140000000 : 0x400000;
        this.file_alignment = 0x200;
        this.section_alignment = 0x1000;
        
        this.init_standard_sections();
    }
    
    init_standard_sections() {
        // 标准PE节
        this.add_section('.text', 0x60000020);  // 可执行、可读、代码
        this.add_section('.data', 0xC0000040);  // 可读、可写、初始化数据
        this.add_section('.rdata', 0x40000040); // 只读数据
        this.add_section('.pdata', 0x40000040); // 异常信息(x64)
        this.add_section('.reloc', 0x42000040); // 重定位
    }
    
    add_section(name, characteristics, raw_data = null) {
        const section = new PeSection(name, characteristics);
        if (raw_data) {
            section.set_raw_data(raw_data);
        }
        this.sections.set(name, section);
        return section;
    }
}
```

#### 1.1.2 完整的PE头结构

```javascript
class PeHeaders {
    generate_dos_header() {
        const writer = new BinaryWriter();
        
        // DOS MZ头
        writer.write_u16(0x5A4D);    // e_magic: 'MZ'
        writer.write_u16(0x0090);    // e_cblp
        writer.write_u16(0x0003);    // e_cp
        // ... 完整的DOS头字段
        writer.write_u32(0x00000040); // e_lfarlc
        writer.write_u32(0x00000000); // e_ovno
        // ... 保留字段
        writer.write_u32(0x00000080); // e_lfanew (PE头偏移)
        
        // DOS存根程序
        const stub = this.generate_dos_stub();
        writer.write_bytes(stub);
        
        return writer.get_bytes();
    }
    
    generate_nt_headers(pe_builder) {
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
    
    generate_file_header(pe_builder) {
        const writer = new BinaryWriter();
        
        writer.write_u16(this.get_machine_type(pe_builder.architecture));
        writer.write_u16(pe_builder.sections.size); // 节数量
        writer.write_u32(Math.floor(Date.now() / 1000)); // 时间戳
        writer.write_u32(0); // 符号表指针
        writer.write_u32(0); // 符号数量
        writer.write_u16(this.get_optional_header_size(pe_builder.architecture));
        writer.write_u16(this.get_characteristics(pe_builder.architecture));
        
        return writer.get_bytes();
    }
    
    generate_optional_header(pe_builder) {
        const writer = new BinaryWriter();
        
        // 标准字段
        writer.write_u16(this.get_pe_magic(pe_builder.architecture));
        writer.write_u8(0); // 主链接器版本
        writer.write_u8(0); // 副链接器版本
        writer.write_u32(pe_builder.get_code_size());
        writer.write_u32(pe_builder.get_initialized_data_size());
        writer.write_u32(pe_builder.get_uninitialized_data_size());
        writer.write_u32(pe_builder.get_entry_point_rva());
        writer.write_u32(pe_builder.get_code_base_rva());
        
        if (pe_builder.architecture === 'x86') {
            writer.write_u32(pe_builder.get_data_base_rva());
        }
        
        // Windows特定字段
        writer.write_u32(pe_builder.base_address); // 映像基址
        writer.write_u32(pe_builder.section_alignment);
        writer.write_u32(pe_builder.file_alignment);
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
}
```

#### 1.1.3 数据目录实现

```javascript
class DataDirectories {
    write_data_directories(writer, pe_builder) {
        // 导出表
        const export_table = pe_builder.exports.get_directory_entry();
        writer.write_u32(export_table.rva);
        writer.write_u32(export_table.size);
        
        // 导入表
        const import_table = pe_builder.imports.get_directory_entry();
        writer.write_u32(import_table.rva);
        writer.write_u32(import_table.size);
        
        // 资源表
        const resource_table = pe_builder.resources.get_directory_entry();
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
        const reloc_table = pe_builder.relocations.get_directory_entry();
        writer.write_u32(reloc_table.rva);
        writer.write_u32(reloc_table.size);
        
        // 调试信息
        const debug_table = pe_builder.debug_info.get_directory_entry();
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
        
        // IAT
        const iat = pe_builder.imports.get_iat_directory();
        writer.write_u32(iat.rva);
        writer.write_u32(iat.size);
        
        // 延迟导入表
        writer.write_u32(0);
        writer.write_u32(0);
        
        // COM描述符
        writer.write_u32(0);
        writer.write_u32(0);
        
        // 保留
        writer.write_u32(0);
        writer.write_u32(0);
    }
}
```

### 1.2 导入表与动态链接 (第5-8周)

**目标**：实现完整的Windows DLL导入机制

#### 1.2.1 专业导入表实现

```javascript
class ProfessionalImportTable {
    constructor() {
        this.libraries = new Map(); // DLL名称 -> ImportLibrary
        this.iat_rva = 0x2000;     // 导入地址表RVA
        this.ilt_rva = 0x3000;     // 导入查找表RVA
        this.hint_name_rva = 0x4000; // 提示名称表RVA
        this.dll_names_rva = 0x5000; // DLL名称RVA
    }
    
    add_library(name) {
        const library = new ImportLibrary(name);
        this.libraries.set(name, library);
        return library;
    }
    
    add_import(library_name, function_name, ordinal = null) {
        let library = this.libraries.get(library_name);
        if (!library) {
            library = this.add_library(library_name);
        }
        
        return library.add_import(function_name, ordinal);
    }
    
    calculate_size() {
        let size = 0;
        
        // 导入目录表大小 (每个库20字节，以空项结束)
        size += (this.libraries.size + 1) * 20;
        
        // IAT和ILT大小
        let total_imports = 0;
        for (const library of this.libraries.values()) {
            total_imports += library.imports.size;
        }
        size += (total_imports + 1) * 4 * 2; // IAT和ILT各一个数组
        
        // 提示名称表大小
        for (const library of this.libraries.values()) {
            for (const imp of library.imports.values()) {
                size += 2; // 提示
                size += imp.name.length + 1; // 名称 + null终止符
                if (size % 2 !== 0) size++; // 对齐到2字节
            }
        }
        
        // DLL名称大小
        for (const library of this.libraries.values()) {
            size += library.name.length + 1; // 名称 + null终止符
        }
        
        return size;
    }
    
    generate_rdata(pe_builder) {
        const writer = new BinaryWriter();
        const rdata_section = pe_builder.sections.get('.rdata');
        
        // 设置RVA基址
        const base_rva = rdata_section.virtual_address;
        let current_rva = base_rva;
        
        // 生成导入目录表
        const import_directories = this.generate_import_directories(base_rva);
        writer.write_bytes(import_directories);
        
        // 更新当前RVA
        current_rva += import_directories.length;
        
        // 生成导入地址表(IAT)和导入查找表(ILT)
        const iat_ilt = this.generate_iat_and_ilt(base_rva + current_rva);
        writer.write_bytes(iat_ilt.iat);
        writer.align(4);
        writer.write_bytes(iat_ilt.ilt);
        
        current_rva += iat_ilt.iat.length + iat_ilt.ilt.length;
        
        // 生成提示名称表
        const hint_names = this.generate_hint_name_table(base_rva + current_rva);
        writer.write_bytes(hint_names);
        
        current_rva += hint_names.length;
        
        // 生成DLL名称
        const dll_names = this.generate_dll_names(base_rva + current_rva);
        writer.write_bytes(dll_names);
        
        rdata_section.set_raw_data(writer.get_bytes());
        
        // 更新数据目录
        pe_builder.data_directories.import_table = {
            rva: base_rva,
            size: import_directories.length
        };
        
        pe_builder.data_directories.iat = {
            rva: base_rva + import_directories.length,
            size: iat_ilt.iat.length
        };
    }
    
    generate_import_directories(base_rva) {
        const writer = new BinaryWriter();
        let current_ilt_rva = base_rva + (this.libraries.size + 1) * 20;
        let current_name_rva = current_ilt_rva + this.get_total_imports() * 8;
        
        for (const library of this.libraries.values()) {
            // 导入查找表RVA
            writer.write_u32(current_ilt_rva);
            // 时间戳
            writer.write_u32(0);
            // 转发链
            writer.write_u32(0);
            // 名称RVA
            writer.write_u32(current_name_rva);
            // 导入地址表RVA
            writer.write_u32(current_ilt_rva - base_rva + this.iat_rva);
            
            current_ilt_rva += library.imports.size * 4 + 4;
            current_name_rva += library.name.length + 1;
        }
        
        // 结束标记 (全零)
        for (let i = 0; i < 5; i++) {
            writer.write_u32(0);
        }
        
        return writer.get_bytes();
    }
}
```

#### 1.2.2 延迟加载导入

```javascript
class DelayLoadImportTable {
    constructor() {
        this.libraries = new Map();
        this.attributes = 0;
    }
    
    generate_delay_load_directory(pe_builder) {
        const writer = new BinaryWriter();
        const base_rva = pe_builder.allocate_rdata_space(this.calculate_size());
        
        for (const library of this.libraries.values()) {
            // 延迟加载描述符
            writer.write_u32(this.attributes);
            writer.write_u32(library.name_rva);
            writer.write_u32(library.module_handle_rva);
            writer.write_u32(library.import_address_table_rva);
            writer.write_u32(library.import_name_table_rva);
            writer.write_u32(library.bound_import_address_table_rva);
            writer.write_u32(library.unload_information_table_rva);
            writer.write_u32(library.time_stamp);
        }
        
        // 结束标记
        for (let i = 0; i < 8; i++) {
            writer.write_u32(0);
        }
        
        return {
            rva: base_rva,
            size: writer.get_bytes().length,
            data: writer.get_bytes()
        };
    }
}
```

### 1.3 重定位表实现 (第9-12周)

**目标**：实现完整的基址重定位支持

#### 1.3.1 重定位表生成器

```javascript
class RelocationTable {
    constructor() {
        this.blocks = new Map(); // 页RVA -> 重定位项数组
        this.base_relocation_rva = 0x4000;
    }
    
    add_relocation(page_rva, offset, type) {
        if (!this.blocks.has(page_rva)) {
            this.blocks.set(page_rva, []);
        }
        
        const block = this.blocks.get(page_rva);
        block.push({
            offset: offset,
            type: type
        });
    }
    
    calculate_size() {
        let size = 0;
        
        for (const [page_rva, relocations] of this.blocks) {
            // 每个块: 页RVA(4) + 块大小(4) + 重定位项(每个2字节)
            size += 8 + relocations.length * 2;
        }
        
        return size;
    }
    
    generate_reloc_section(pe_builder) {
        const writer = new BinaryWriter();
        const reloc_section = pe_builder.sections.get('.reloc');
        
        // 按页RVA排序
        const sorted_pages = Array.from(this.blocks.keys()).sort((a, b) => a - b);
        
        for (const page_rva of sorted_pages) {
            const relocations = this.blocks.get(page_rva);
            
            // 块头
            writer.write_u32(page_rva);
            writer.write_u32(8 + relocations.length * 2); // 块大小
            
            // 重定位项
            for (const reloc of relocations) {
                const entry = (reloc.offset & 0x0FFF) | (reloc.type << 12);
                writer.write_u16(entry);
            }
        }
        
        reloc_section.set_raw_data(writer.get_bytes());
        
        // 更新数据目录
        if (writer.get_bytes().length > 0) {
            pe_builder.data_directories.base_relocation_table = {
                rva: reloc_section.virtual_address,
                size: writer.get_bytes().length
            };
        }
    }
    
    // 自动分析代码并添加重定位
    analyze_code_for_relocations(code_section, base_address) {
        const code = code_section.raw_data;
        const scanner = new RelocationScanner(code, base_address);
        
        const relocations = scanner.find_absolute_references();
        for (const reloc of relocations) {
            const page_rva = reloc.address & 0xFFFFF000;
            const offset = reloc.address & 0x00000FFF;
            
            this.add_relocation(page_rva, offset, RelocationType.ABSOLUTE);
        }
        
        return relocations.length;
    }
}

class RelocationScanner {
    constructor(code, base_address) {
        this.code = code;
        this.base_address = base_address;
        this.positions = [];
    }
    
    find_absolute_references() {
        const relocations = [];
        
        // 扫描可能的32位绝对地址
        for (let i = 0; i <= this.code.length - 4; i++) {
            const value = this.read_u32(i);
            
            // 检查是否在预期的基址范围内
            if (this.is_likely_absolute_reference(value)) {
                relocations.push({
                    address: this.base_address + i,
                    type: RelocationType.ABSOLUTE,
                    value: value
                });
            }
        }
        
        return relocations;
    }
    
    is_likely_absolute_reference(value) {
        // 检查是否在常见的PE内存范围内
        return (value >= 0x400000 && value < 0x80000000) ||
               (value >= 0x100000000 && value < 0x800000000000);
    }
    
    read_u32(offset) {
        return (this.code[offset] |
               (this.code[offset + 1] << 8) |
               (this.code[offset + 2] << 16) |
               (this.code[offset + 3] << 24)) >>> 0;
    }
}
```

## 阶段二：x86架构深度实现 (第4-6个月)

### 2.1 x86指令编码系统 (第1-4周)

**目标**：实现完整的x86指令编码器

#### 2.1.1 指令编码架构

```javascript
class X86InstructionEncoder {
    constructor() {
        this.current_address = 0;
        this.pending_fixes = [];
        this.labels = new Map();
    }
    
    encode_instruction(mnemonic, operands = []) {
        const encoder = this.get_encoder_for_mnemonic(mnemonic);
        return encoder(operands);
    }
    
    get_encoder_for_mnemonic(mnemonic) {
        switch (mnemonic.toLowerCase()) {
            case 'mov': return this.encode_mov.bind(this);
            case 'add': return this.encode_add.bind(this);
            case 'sub': return this.encode_sub.bind(this);
            case 'push': return this.encode_push.bind(this);
            case 'pop': return this.encode_pop.bind(this);
            case 'call': return this.encode_call.bind(this);
            case 'ret': return this.encode_ret.bind(this);
            case 'jmp': return this.encode_jmp.bind(this);
            case 'je': return this.encode_je.bind(this);
            case 'jne': return this.encode_jne.bind(this);
            // ... 更多指令
            default: throw new Error(`未知指令: ${mnemonic}`);
        }
    }
    
    encode_mov(operands) {
        const [dst, src] = operands;
        
        if (dst.type === 'register' && src.type === 'immediate') {
            return this.encode_mov_reg_imm(dst, src);
        } else if (dst.type === 'register' && src.type === 'register') {
            return this.encode_mov_reg_reg(dst, src);
        } else if (dst.type === 'register' && src.type === 'memory') {
            return this.encode_mov_reg_mem(dst, src);
        } else if (dst.type === 'memory' && src.type === 'register') {
            return this.encode_mov_mem_reg(dst, src);
        } else if (dst.type === 'memory' && src.type === 'immediate') {
            return this.encode_mov_mem_imm(dst, src);
        }
        
        throw new Error(`不支持的MOV操作数组合: ${dst.type} <- ${src.type}`);
    }
    
    encode_mov_reg_imm(dst, src) {
        const bytes = [];
        const reg_code = this.get_register_code(dst.value);
        
        if (dst.size === 8) {
            // MOV r8, imm8
            bytes.push(0xB0 + reg_code);
            bytes.push(src.value & 0xFF);
        } else if (dst.size === 16) {
            // MOV r16, imm16
            bytes.push(0x66); // 操作数大小前缀
            bytes.push(0xB8 + reg_code);
            bytes.push(src.value & 0xFF);
            bytes.push((src.value >> 8) & 0xFF);
        } else if (dst.size === 32) {
            // MOV r32, imm32
            bytes.push(0xB8 + reg_code);
            this.write_u32(bytes, src.value);
        }
        
        return bytes;
    }
    
    encode_mov_reg_reg(dst, src) {
        const bytes = [];
        const dst_code = this.get_register_code(dst.value);
        const src_code = this.get_register_code(src.value);
        
        // MOV r32, r/m32
        bytes.push(0x8B);
        
        // ModR/M字节: mod=11 (寄存器模式), reg=dst, r/m=src
        const modrm = (0b11 << 6) | (dst_code << 3) | src_code;
        bytes.push(modrm);
        
        return bytes;
    }
    
    encode_call(operands) {
        const [target] = operands;
        const bytes = [];
        
        if (target.type === 'immediate') {
            // CALL rel32
            bytes.push(0xE8);
            
            if (typeof target.value === 'number') {
                const relative_offset = target.value - (this.current_address + 5);
                this.write_u32(bytes, relative_offset);
            } else if (target.type === 'label') {
                // 延迟解析标签
                this.pending_fixes.push({
                    position: this.current_address + 1,
                    label: target.value,
                    type: 'relative_32'
                });
                this.write_u32(bytes, 0); // 临时填充
            }
        } else if (target.type === 'register') {
            // CALL r/m32
            bytes.push(0xFF);
            
            const reg_code = this.get_register_code(target.value);
            const modrm = (0b11 << 6) | (0b010 << 3) | reg_code; // CALL r32
            bytes.push(modrm);
        } else if (target.type === 'memory') {
            // CALL [mem]
            bytes.push(0xFF);
            bytes.push(...this.encode_memory_operand(0b010, target));
        }
        
        return bytes;
    }
    
    encode_memory_operand(reg_field, mem_operand) {
        const bytes = [];
        
        // 处理不同的寻址模式
        if (mem_operand.base && !mem_operand.index && !mem_operand.displacement) {
            // [base]
            bytes.push((0b00 << 6) | (reg_field << 3) | this.get_register_code(mem_operand.base));
        } else if (mem_operand.base && mem_operand.displacement === 0) {
            // [base]
            bytes.push((0b00 << 6) | (reg_field << 3) | this.get_register_code(mem_operand.base));
        } else if (mem_operand.base && this.is_8bit_displacement(mem_operand.displacement)) {
            // [base + disp8]
            bytes.push((0b01 << 6) | (reg_field << 3) | this.get_register_code(mem_operand.base));
            bytes.push(mem_operand.displacement & 0xFF);
        } else if (mem_operand.base) {
            // [base + disp32]
            bytes.push((0b10 << 6) | (reg_field << 3) | this.get_register_code(mem_operand.base));
            this.write_u32(bytes, mem_operand.displacement);
        } else if (mem_operand.displacement !== undefined) {
            // [disp32]
            bytes.push((0b00 << 6) | (reg_field << 3) | 0b101);
            this.write_u32(bytes, mem_operand.displacement);
        } else {
            throw new Error('不支持的寻址模式');
        }
        
        // SIB字节 (如果需要)
        if (mem_operand.base && mem_operand.index) {
            bytes.push(this.encode_sib_byte(mem_operand));
        }
        
        return bytes;
    }
    
    get_register_code(reg_name) {
        const registers = {
            'eax': 0, 'ecx': 1, 'edx': 2, 'ebx': 3,
            'esp': 4, 'ebp': 5, 'esi': 6, 'edi': 7,
            'ax': 0, 'cx': 1, 'dx': 2, 'bx': 3,
            'sp': 4, 'bp': 5, 'si': 6, 'di': 7,
            'al': 0, 'cl': 1, 'dl': 2, 'bl': 3,
            'ah': 4, 'ch': 5, 'dh': 6, 'bh': 7
        };
        
        const code = registers[reg_name.toLowerCase()];
        if (code === undefined) {
            throw new Error(`未知寄存器: ${reg_name}`);
        }
        return code;
    }
    
    write_u32(bytes, value) {
        bytes.push(value & 0xFF);
        bytes.push((value >> 8) & 0xFF);
        bytes.push((value >> 16) & 0xFF);
        bytes.push((value >> 24) & 0xFF);
    }
}
```

#### 2.1.2 条件跳转编码

```javascript
class X86ControlFlowEncoder extends X86InstructionEncoder {
    encode_conditional_jump(mnemonic, operands) {
        const [target] = operands;
        const bytes = [];
        
        const opcode = this.get_conditional_jump_opcode(mnemonic);
        if (opcode.short) {
            bytes.push(opcode.short);
            
            if (target.type === 'immediate') {
                const relative_offset = target.value - (this.current_address + 2);
                if (this.is_8bit_displacement(relative_offset)) {
                    bytes.push(relative_offset & 0xFF);
                } else {
                    // 使用长跳转
                    return this.encode_long_conditional_jump(mnemonic, operands);
                }
            } else if (target.type === 'label') {
                this.pending_fixes.push({
                    position: this.current_address + 1,
                    label: target.value,
                    type: 'relative_8'
                });
                bytes.push(0); // 临时填充
            }
        }
        
        return bytes;
    }
    
    encode_long_conditional_jump(mnemonic, operands) {
        const [target] = operands;
        const bytes = [];
        
        const opcode = this.get_conditional_jump_opcode(mnemonic);
        
        // 短跳转跳过长跳转指令
        bytes.push(opcode.short);
        bytes.push(0x06); // 跳过6字节
        
        // 近跳转
        bytes.push(0xE9); // JMP rel32
        
        if (target.type === 'immediate') {
            const relative_offset = target.value - (this.current_address + 6);
            this.write_u32(bytes, relative_offset);
        } else if (target.type === 'label') {
            this.pending_fixes.push({
                position: this.current_address + 2,
                label: target.value,
                type: 'relative_32'
            });
            this.write_u32(bytes, 0); // 临时填充
        }
        
        return bytes;
    }
    
    get_conditional_jump_opcode(mnemonic) {
        const jump_codes = {
            'jo':  { short: 0x70 }, // 溢出
            'jno': { short: 0x71 }, // 无溢出
            'jb':  { short: 0x72 }, // 低于
            'jnae': { short: 0x72 }, // 不高于等于
            'jc':  { short: 0x72 }, // 进位
            'jnb': { short: 0x73 }, // 不低于
            'jae': { short: 0x73 }, // 高于等于
            'jnc': { short: 0x73 }, // 无进位
            'jz':  { short: 0x74 }, // 为零
            'je':  { short: 0x74 }, // 相等
            'jnz': { short: 0x75 }, // 非零
            'jne': { short: 0x75 }, // 不相等
            'jbe': { short: 0x76 }, // 低于等于
            'jna': { short: 0x76 }, // 不高于
            'ja':  { short: 0x77 }, // 高于
            'jnbe': { short: 0x77 }, // 不低于等于
            'js':  { short: 0x78 }, // 符号位设置
            'jns': { short: 0x79 }, // 符号位清除
            'jp':  { short: 0x7A }, // 奇偶校验为偶
            'jpe': { short: 0x7A }, // 奇偶校验为偶
            'jnp': { short: 0x7B }, // 奇偶校验为奇
            'jpo': { short: 0x7B }, // 奇偶校验为奇
            'jl':  { short: 0x7C }, // 小于
            'jnge': { short: 0x7C }, // 不大于等于
            'jge': { short: 0x7D }, // 大于等于
            'jnl': { short: 0x7D }, // 不小于
            'jle': { short: 0x7E }, // 小于等于
            'jng': { short: 0x7E }, // 不大于
            'jg':  { short: 0x7F }, // 大于
            'jnle': { short: 0x7F }  // 不小于等于
        };
        
        const opcode = jump_codes[mnemonic.toLowerCase()];
        if (!opcode) {
            throw new Error(`未知的条件跳转指令: ${mnemonic}`);
        }
        return opcode;
    }
}
```

### 2.2 函数调用约定与栈帧 (第5-8周)

**目标**：实现完整的函数调用机制

#### 2.2.1 调用约定实现

```javascript
class CallingConvention {
    constructor(convention = 'cdecl') {
        this.convention = convention;
        this.parameter_registers = [];
        this.return_register = 'eax';
        this.callee_saved_registers = ['ebx', 'esi', 'edi'];
        this.caller_saved_registers = ['eax', 'ecx', 'edx'];
        
        this.setup_convention(convention);
    }
    
    setup_convention(convention) {
        switch (convention) {
            case 'cdecl':
                this.parameter_passing = 'stack';
                this.stack_cleanup = 'caller';
                this.stack_alignment = 4;
                break;
                
            case 'stdcall':
                this.parameter_passing = 'stack';
                this.stack_cleanup = 'callee';
                this.stack_alignment = 4;
                break;
                
            case 'fastcall':
                this.parameter_passing = 'mixed';
                this.parameter_registers = ['ecx', 'edx'];
                this.stack_cleanup = 'callee';
                this.stack_alignment = 4;
                break;
                
            default:
                throw new Error(`不支持的调用约定: ${convention}`);
        }
    }
    
    generate_prologue(frame_size, codegen) {
        const instructions = [];
        
        // 保存调用者基址指针
        instructions.push(...codegen.encode_instruction('push', ['ebp']));
        instructions.push(...codegen.encode_instruction('mov', ['ebp', 'esp']));
        
        // 分配栈空间
        if (frame_size > 0) {
            instructions.push(...codegen.encode_instruction('sub', ['esp', frame_size]));
        }
        
        // 保存被调用者保存的寄存器
        for (const reg of this.callee_saved_registers) {
            instructions.push(...codegen.encode_instruction('push', [reg]));
        }
        
        return instructions;
    }
    
    generate_epilogue(frame_size, codegen) {
        const instructions = [];
        
        // 恢复被调用者保存的寄存器
        for (let i = this.callee_saved_registers.length - 1; i >= 0; i--) {
            const reg = this.callee_saved_registers[i];
            instructions.push(...codegen.encode_instruction('pop', [reg]));
        }
        
        // 恢复栈指针
        if (frame_size > 0) {
            instructions.push(...codegen.encode_instruction('mov', ['esp', 'ebp']));
        }
        
        // 恢复调用者基址指针
        instructions.push(...codegen.encode_instruction('pop', ['ebp']));
        
        // 返回
        if (this.stack_cleanup === 'callee') {
            instructions.push(...codegen.encode_instruction('ret', [frame_size]));
        } else {
            instructions.push(...codegen.encode_instruction('ret', []));
        }
        
        return instructions;
    }
    
    generate_parameter_pass(parameters, codegen) {
        const instructions = [];
        
        switch (this.parameter_passing) {
            case 'stack':
                // 从右向左压入参数
                for (let i = parameters.length - 1; i >= 0; i--) {
                    instructions.push(...this.push_parameter(parameters[i], codegen));
                }
                break;
                
            case 'mixed':
                // 前两个参数用寄存器，其余用栈
                for (let i = 0; i < parameters.length; i++) {
                    if (i < this.parameter_registers.length) {
                        instructions.push(...this.move_to_register(
                            parameters[i], 
                            this.parameter_registers[i], 
                            codegen
                        ));
                    } else {
                        instructions.push(...this.push_parameter(parameters[i], codegen));
                    }
                }
                break;
        }
        
        return instructions;
    }
    
    push_parameter(param, codegen) {
        if (param.type === 'immediate') {
            return codegen.encode_instruction('push', [param.value]);
        } else if (param.type === 'register') {
            return codegen.encode_instruction('push', [param.value]);
        } else if (param.type === 'memory') {
            const temp_reg = 'eax';
            const instructions = [];
            instructions.push(...codegen.encode_instruction('mov', [temp_reg, param]));
            instructions.push(...codegen.encode_instruction('push', [temp_reg]));
            return instructions;
        }
        
        throw new Error(`不支持的参数类型: ${param.type}`);
    }
    
    generate_stack_cleanup(param_count, codegen) {
        if (this.stack_cleanup === 'caller' && param_count > 0) {
            const stack_size = param_count * 4;
            return codegen.encode_instruction('add', ['esp', stack_size]);
        }
        return [];
    }
}

class FunctionFrame {
    constructor(function_name, return_type) {
        this.name = function_name;
        this.return_type = return_type;
        this.parameters = new Map();
        this.local_variables = new Map();
        this.temp_variables = new Map();
        this.stack_size = 0;
        this.parameter_offset = 8; // EBP+8 是第一个参数
    }
    
    add_parameter(name, type, size = 4) {
        this.parameters.set(name, {
            name: name,
            type: type,
            size: size,
            offset: this.parameter_offset,
            is_parameter: true
        });
        this.parameter_offset += size;
    }
    
    add_local_variable(name, type, size = 4) {
        this.stack_size += size;
        const offset = -this.stack_size;
        
        this.local_variables.set(name, {
            name: name,
            type: type,
            size: size,
            offset: offset,
            is_local: true
        });
        
        return offset;
    }
    
    get_variable_address(name) {
        if (this.parameters.has(name)) {
            const param = this.parameters.get(name);
            return { base: 'ebp', displacement: param.offset };
        } else if (this.local_variables.has(name)) {
            const local = this.local_variables.get(name);
            return { base: 'ebp', displacement: local.offset };
        }
        
        throw new Error(`未定义的变量: ${name}`);
    }
    
    calculate_frame_size() {
        // 栈帧大小 = 局部变量 + 对齐填充
        let size = this.stack_size;
        
        // 对齐到16字节（Windows要求）
        if (size % 16 !== 0) {
            size += 16 - (size % 16);
        }
        
        return size;
    }
}
```

#### 2.2.2 复杂函数生成

```javascript
class FunctionGenerator {
    constructor(function_name, return_type, calling_convention = 'cdecl') {
        this.function_name = function_name;
        this.return_type = return_type;
        this.calling_convention = new CallingConvention(calling_convention);
        this.frame = new FunctionFrame(function_name, return_type);
        this.codegen = new X86ControlFlowEncoder();
        this.instructions = [];
        this.labels = new Map();
        this.current_label_id = 0;
    }
    
    add_parameter(name, type) {
        this.frame.add_parameter(name, type);
    }
    
    add_local_variable(name, type, size = 4) {
        return this.frame.add_local_variable(name, type, size);
    }
    
    generate_prologue() {
        const frame_size = this.frame.calculate_frame_size();
        const prologue = this.calling_convention.generate_prologue(frame_size, this.codegen);
        this.instructions.push(...prologue);
    }
    
    generate_epilogue() {
        const frame_size = this.frame.calculate_frame_size();
        const epilogue = this.calling_convention.generate_epilogue(frame_size, this.codegen);
        this.instructions.push(...epilogue);
    }
    
    generate_function_call(target_function, parameters, cleanup = true) {
        const instructions = [];
        
        // 传递参数
        const param_pass = this.calling_convention.generate_parameter_pass(parameters, this.codegen);
        instructions.push(...param_pass);
        
        // 调用函数
        instructions.push(...this.codegen.encode_instruction('call', [target_function]));
        
        // 清理栈（如果需要）
        if (cleanup && this.calling_convention.stack_cleanup === 'caller') {
            const cleanup_code = this.calling_convention.generate_stack_cleanup(parameters.length, this.codegen);
            instructions.push(...cleanup_code);
        }
        
        this.instructions.push(...instructions);
        return instructions;
    }
    
    generate_return(value = null) {
        const instructions = [];
        
        if (value !== null) {
            // 设置返回值
            if (value.type === 'immediate') {
                instructions.push(...this.codegen.encode_instruction('mov', ['eax', value.value]));
            } else if (value.type === 'register') {
                if (value.value !== 'eax') {
                    instructions.push(...this.codegen.encode_instruction('mov', ['eax', value.value]));
                }
            } else if (value.type === 'memory') {
                instructions.push(...this.codegen.encode_instruction('mov', ['eax', value]));
            }
        }
        
        this.instructions.push(...instructions);
        return instructions;
    }
    
    generate_variable_assignment(variable_name, value) {
        const variable_addr = this.frame.get_variable_address(variable_name);
        const instructions = [];
        
        if (value.type === 'immediate') {
            instructions.push(...this.codegen.encode_instruction('mov', [
                { type: 'memory', ...variable_addr },
                value.value
            ]));
        } else if (value.type === 'register') {
            instructions.push(...this.codegen.encode_instruction('mov', [
                { type: 'memory', ...variable_addr },
                value.value
            ]));
        }
        
        this.instructions.push(...instructions);
        return instructions;
    }
    
    generate_conditional(condition, then_block, else_block = null) {
        const else_label = this.new_label('else');
        const end_label = this.new_label('endif');
        
        const instructions = [];
        
        // 生成条件表达式
        instructions.push(...this.generate_expression(condition));
        
        // 条件跳转
        const jump_instruction = else_block ? 'je' : 'jne';
        instructions.push(...this.codegen.encode_instruction(jump_instruction, [else_label]));
        
        // then块
        instructions.push(...then_block);
        
        if (else_block) {
            // 跳转到结束
            instructions.push(...this.codegen.encode_instruction('jmp', [end_label]));
            
            // else块
            instructions.push(...this.codegen.place_label(else_label));
            instructions.push(...else_block);
            instructions.push(...this.codegen.place_label(end_label));
        } else {
            instructions.push(...this.codegen.place_label(else_label));
        }
        
        this.instructions.push(...instructions);
        return instructions;
    }
    
    new_label(prefix = 'label') {
        const label = `${prefix}_${this.current_label_id++}`;
        this.labels.set(label, this.instructions.length);
        return label;
    }
    
    get_machine_code() {
        // 解析所有指令并生成机器码
        const assembler = new X86Assembler();
        return assembler.assemble(this.instructions, this.labels);
    }
}
```

### 2.3 系统调用与运行时 (第9-12周)

**目标**：实现Windows系统调用和运行时支持

#### 2.3.1 Windows API封装

```javascript
class WindowsRuntime {
    constructor(pe_builder) {
        this.pe_builder = pe_builder;
        this.import_table = pe_builder.imports;
        this.init_essential_imports();
    }
    
    init_essential_imports() {
        // 内核32.dll基本函数
        this.import_table.add_library('kernel32.dll');
        this.import_table.add_import('kernel32.dll', 'GetStdHandle');
        this.import_table.add_import('kernel32.dll', 'WriteConsoleA');
        this.import_table.add_import('kernel32.dll', 'ReadConsoleA');
        this.import_table.add_import('kernel32.dll', 'ExitProcess');
        this.import_table.add_import('kernel32.dll', 'GetCommandLineA');
        this.import_table.add_import('kernel32.dll', 'GetModuleHandleA');
        this.import_table.add_import('kernel32.dll', 'GetProcAddress');
        
        // 高级API
        this.import_table.add_import('kernel32.dll', 'VirtualAlloc');
        this.import_table.add_import('kernel32.dll', 'VirtualFree');
        this.import_table.add_import('kernel32.dll', 'HeapAlloc');
        this.import_table.add_import('kernel32.dll', 'HeapFree');
        
        // 如果需要GUI，添加user32.dll
        // this.import_table.add_library('user32.dll');
        // this.import_table.add_import('user32.dll', 'MessageBoxA');
    }
    
    generate_runtime_stubs() {
        const stubs = new Map();
        
        // 控制台输出函数
        stubs.set('console_write', this.generate_console_write_stub());
        stubs.set('console_read', this.generate_console_read_stub());
        stubs.set('exit_process', this.generate_exit_process_stub());
        
        // 内存管理函数
        stubs.set('memory_alloc', this.generate_memory_alloc_stub());
        stubs.set('memory_free', this.generate_memory_free_stub());
        
        return stubs;
    }
    
    generate_console_write_stub() {
        const generator = new FunctionGenerator('runtime_console_write', 'void', 'stdcall');
        
        // 参数: string_ptr, length
        generator.add_parameter('string_ptr', 'char*');
        generator.add_parameter('length', 'int');
        
        generator.generate_prologue();
        
        // 获取标准输出句柄
        generator.instructions.push(...generator.codegen.encode_instruction('push', [-11])); // STD_OUTPUT_HANDLE
        generator.instructions.push(...generator.codegen.encode_instruction('call', ['GetStdHandle']));
        
        // 保存句柄
        generator.instructions.push(...generator.codegen.encode_instruction('mov', ['ebx', 'eax']));
        
        // 调用WriteConsoleA
        generator.instructions.push(...generator.codegen.encode_instruction('push', [0])); // reserved
        generator.add_local_variable('chars_written', 'int');
        const chars_written_addr = generator.frame.get_variable_address('chars_written');
        generator.instructions.push(...generator.codegen.encode_instruction('lea', ['eax', chars_written_addr]));
        generator.instructions.push(...generator.codegen.encode_instruction('push', ['eax'])); // lpNumberOfCharsWritten
        
        // 长度参数
        const length_addr = generator.frame.get_variable_address('length');
        generator.instructions.push(...generator.codegen.encode_instruction('push', [length_addr]));
        
        // 字符串指针
        const string_addr = generator.frame.get_variable_address('string_ptr');
        generator.instructions.push(...generator.codegen.encode_instruction('push', [string_addr]));
        
        // 句柄
        generator.instructions.push(...generator.codegen.encode_instruction('push', ['ebx']));
        
        generator.instructions.push(...generator.codegen.encode_instruction('call', ['WriteConsoleA']));
        
        generator.generate_epilogue();
        
        return generator.get_machine_code();
    }
    
    generate_exit_process_stub() {
        const generator = new FunctionGenerator('runtime_exit_process', 'void', 'stdcall');
        generator.add_parameter('exit_code', 'int');
        
        generator.generate_prologue();
        
        // 调用ExitProcess
        const exit_code_addr = generator.frame.get_variable_address('exit_code');
        generator.instructions.push(...generator.codegen.encode_instruction('push', [exit_code_addr]));
        generator.instructions.push(...generator.codegen.encode_instruction('call', ['ExitProcess']));
        
        // ExitProcess不会返回，但为了完整性还是生成epilogue
        generator.generate_epilogue();
        
        return generator.get_machine_code();
    }
}
```

## 阶段三：x64架构支持 (第7-9个月)

### 3.1 x64 PE文件格式 (第1-4周)

**目标**：实现x64 PE文件支持

#### 3.1.1 x64 PE头差异

```javascript
class X64PeBuilder extends PeAssembler {
    constructor() {
        super('x64');
        this.base_address = 0x140000000;
        this.init_x64_specific_sections();
    }

    init_x64_specific_sections() {
        // x64特有节
        this.add_section('.pdata', 0x40000040); // 异常信息
        this.add_section('.xdata', 0x40000040); // 异常处理数据
    }

    generate_optional_header() {
        const writer = new BinaryWriter();

        // x64可选头魔术字
        writer.write_u16(0x020B); // PE32+ magic

        // 标准字段
        writer.write_u8(0); // 主链接器版本
        writer.write_u8(0); // 副链接器版本
        writer.write_u32(this.get_code_size());
        writer.write_u32(this.get_initialized_data_size());
        writer.write_u32(this.get_uninitialized_data_size());
        writer.write_u32(this.get_entry_point_rva());
        writer.write_u32(this.get_code_base_rva());

        // x64没有单独的数据基址

        // Windows特定字段
        writer.write_u64(this.base_address); // 64位基址
        writer.write_u32(this.section_alignment);
        writer.write_u32(this.file_alignment);
        writer.write_u16(6); // 主操作系统版本
        writer.write_u16(0); // 副操作系统版本
        writer.write_u16(0); // 主映像版本
        writer.write_u16(0); // 副映像版本
        writer.write_u16(5); // 主子系统版本
        writer.write_u16(0); // 副子系统版本
        writer.write_u32(0); // Win32版本值

        // 大小字段
        writer.write_u32(this.get_image_size());
        writer.write_u32(this.get_headers_size());
        writer.write_u32(this.calculate_checksum());
        writer.write_u16(this.get_subsystem());
        writer.write_u16(this.get_dll_characteristics());

        // 栈堆大小 (64位)
        writer.write_u64(0x0000000000200000); // 栈保留大小
        writer.write_u64(0x0000000000002000); // 栈提交大小
        writer.write_u64(0x0000000000200000); // 堆保留大小
        writer.write_u64(0x0000000000002000); // 堆提交大小

        writer.write_u32(0); // 加载器标志
        writer.write_u32(16); // 数据目录数量

        // 数据目录
        this.write_data_directories(writer);

        return writer.get_bytes();
    }
}
```

#### 3.1.2 x64异常处理

```javascript
class X64ExceptionHandler {
    generate_pdata_section(functions) {
        const writer = new BinaryWriter();
        
        for (const func of functions) {
            // RUNTIME_FUNCTION结构
            writer.write_u32(func.begin_rva);    // BeginAddress
            writer.write_u32(func.end_rva);      // EndAddress
            writer.write_u32(func.unwind_info_rva); // UnwindInfoAddress
        }
        
        return writer.get_bytes();
    }
    
    generate_unwind_info(function_info) {
        const writer = new BinaryWriter();
        
        // UNWIND_INFO结构
        writer.write_u8(0x01); // Version and flags
        writer.write_u8(function_info.prologue_size);
        writer.write_u8(function_info.unwind_code_count);
        writer.write_u8(function_info.frame_register << 4); // Frame register and offset
        
        // 展开代码数组
        for (const code of function_info.unwind_codes) {
            writer.write_u8(code.offset);
            writer.write_u8(code.unwind_operation);
            if (code.operation === 'UWOP_ALLOC_LARGE') {
                writer.write_u16(code.alloc_size);
            }
        }
        
        // 对齐到4字节
        if (writer.get_bytes().length % 4 !== 0) {
            const padding = 4 - (writer.get_bytes().length % 4);
            for (let i = 0; i < padding; i++) {
                writer.write_u8(0);
            }
        }
        
        return writer.get_bytes();
    }
}
```

### 3.2 x64指令编码 (第5-8周)

**目标**：实现x64指令编码

#### 3.2.1 x64指令编码器

```javascript
class X64InstructionEncoder extends X86InstructionEncoder {
    constructor() {
        super();
        this.rex_prefix = 0x40; // REX前缀基值
    }
    
    encode_mov_reg_imm(dst, src) {
        const bytes = [];
        const reg_code = this.get_register_code(dst.value);
        
        if (dst.size === 64) {
            // 需要REX.W前缀
            bytes.push(this.rex_prefix | 0x08); // REX.W = 1
            
            // MOV r64, imm64
            bytes.push(0xB8 + (reg_code & 0x07));
            this.write_u64(bytes, src.value);
        } else if (dst.size === 32) {
            // MOV r32, imm32 (在x64中零扩展到64位)
            bytes.push(0xB8 + (reg_code & 0x07));
            this.write_u32(bytes, src.value);
        } else if (dst.size === 16) {
            // 66前缀用于16位操作数
            bytes.push(0x66);
            bytes.push(0xB8 + (reg_code & 0x07));
            this.write_u16(bytes, src.value);
        } else if (dst.size === 8) {
            // MOV r8, imm8
            if (reg_code >= 4) {
                // 需要REX前缀来访问sil, dil等
                bytes.push(this.rex_prefix);
            }
            bytes.push(0xB0 + (reg_code & 0x07));
            bytes.push(src.value & 0xFF);
        }
        
        return bytes;
    }
    
    encode_mov_reg_reg(dst, src) {
        const bytes = [];
        const dst_code = this.get_register_code(dst.value);
        const src_code = this.get_register_code(src.value);
        
        // 决定是否需要REX前缀
        let rex = 0x40;
        if (dst.size === 64 || src.size === 64) {
            rex |= 0x08; // REX.W
        }
        if (dst_code >= 8) {
            rex |= 0x04; // REX.R
        }
        if (src_code >= 8) {
            rex |= 0x01; // REX.B
        }
        
        if (rex !== 0x40) {
            bytes.push(rex);
        }
        
        // MOV r64, r/m64 或 MOV r32, r/m32
        bytes.push(0x8B);
        
        // ModR/M字节
        const modrm = (0b11 << 6) | 
                     ((dst_code & 0x07) << 3) | 
                     (src_code & 0x07);
        bytes.push(modrm);
        
        return bytes;
    }
    
    get_register_code(reg_name) {
        const x64_registers = {
            // 64位寄存器
            'rax': 0, 'rcx': 1, 'rdx': 2, 'rbx': 3,
            'rsp': 4, 'rbp': 5, 'rsi': 6, 'rdi': 7,
            'r8': 8, 'r9': 9, 'r10': 10, 'r11': 11,
            'r12': 12, 'r13': 13, 'r14': 14, 'r15': 15,
            
            // 32位寄存器
            'eax': 0, 'ecx': 1, 'edx': 2, 'ebx': 3,
            'esp': 4, 'ebp': 5, 'esi': 6, 'edi': 7,
            'r8d': 8, 'r9d': 9, 'r10d': 10, 'r11d': 11,
            'r12d': 12, 'r13d': 13, 'r14d': 14, 'r15d': 15,
            
            // 16位寄存器
            'ax': 0, 'cx': 1, 'dx': 2, 'bx': 3,
            'sp': 4, 'bp': 5, 'si': 6, 'di': 7,
            'r8w': 8, 'r9w': 9, 'r10w': 10, 'r11w': 11,
            'r12w': 12, 'r13w': 13, 'r14w': 14, 'r15w': 15,
            
            // 8位寄存器
            'al': 0, 'cl': 1, 'dl': 2, 'bl': 3,
            'spl': 4, 'bpl': 5, 'sil': 6, 'dil': 7,
            'r8b': 8, 'r9b': 9, 'r10b': 10, 'r11b': 11,
            'r12b': 12, 'r13b': 13, 'r14b': 14, 'r15b': 15
        };
        
        const code = x64_registers[reg_name.toLowerCase()];
        if (code === undefined) {
            throw new Error(`未知的x64寄存器: ${reg_name}`);
        }
        return code;
    }
    
    write_u64(bytes, value) {
        this.write_u32(bytes, value & 0xFFFFFFFF);
        this.write_u32(bytes, (value >> 32) & 0xFFFFFFFF);
    }
}
```

#### 3.2.2 x64调用约定

```javascript
class X64CallingConvention extends CallingConvention {
    constructor(convention = 'microsoft') {
        super(convention);
        this.setup_x64_convention(convention);
    }
    
    setup_x64_convention(convention) {
        switch (convention) {
            case 'microsoft':
                this.parameter_registers = ['rcx', 'rdx', 'r8', 'r9'];
                this.return_register = 'rax';
                this.callee_saved_registers = ['rbx', 'rsp', 'rbp', 'rsi', 'rdi', 'r12', 'r13', 'r14', 'r15'];
                this.caller_saved_registers = ['rax', 'rcx', 'rdx', 'r8', 'r9', 'r10', 'r11'];
                this.parameter_passing = 'register_first';
                this.stack_cleanup = 'caller';
                this.stack_alignment = 16;
                this.shadow_space = 32; // 调用者必须分配32字节影子空间
                break;
                
            case 'system_v':
                this.parameter_registers = ['rdi', 'rsi', 'rdx', 'rcx', 'r8', 'r9'];
                this.return_register = 'rax';
                this.callee_saved_registers = ['rbx', 'rsp', 'rbp', 'r12', 'r13', 'r14', 'r15'];
                this.caller_saved_registers = ['rax', 'rcx', 'rdx', 'rsi', 'rdi', 'r8', 'r9', 'r10', 'r11'];
                this.parameter_passing = 'register_first';
                this.stack_cleanup = 'caller';
                this.stack_alignment = 16;
                this.shadow_space = 0;
                break;
                
            default:
                throw new Error(`不支持的x64调用约定: ${convention}`);
        }
    }
    
    generate_prologue(frame_size, codegen) {
        const instructions = [];
        
        // x64 prologue
        instructions.push(...codegen.encode_instruction('push', ['rbp']));
        instructions.push(...codegen.encode_instruction('mov', ['rbp', 'rsp']));
        
        // 分配栈空间（包括对齐）
        const total_frame_size = this.calculate_total_frame_size(frame_size);
        if (total_frame_size > 0) {
            if (total_frame_size <= 128) {
                instructions.push(...codegen.encode_instruction('sub', ['rsp', total_frame_size]));
            } else {
                // 大帧使用mov指令
                instructions.push(...codegen.encode_instruction('mov', ['rax', total_frame_size]));
                instructions.push(...codegen.encode_instruction('sub', ['rsp', 'rax']));
            }
        }
        
        // 保存被调用者保存的寄存器
        for (const reg of this.callee_saved_registers) {
            if (reg !== 'rsp') { // 不要保存rsp
                instructions.push(...codegen.encode_instruction('push', [reg]));
            }
        }
        
        return instructions;
    }
    
    generate_parameter_pass(parameters, codegen) {
        const instructions = [];
        let stack_parameters = 0;
        
        for (let i = 0; i < parameters.length; i++) {
            const param = parameters[i];
            
            if (i < this.parameter_registers.length) {
                // 使用寄存器传递
                instructions.push(...this.move_to_register(
                    param, 
                    this.parameter_registers[i], 
                    codegen
                ));
            } else {
                // 使用栈传递
                stack_parameters++;
                const stack_offset = this.shadow_space + (stack_parameters - 1) * 8;
                instructions.push(...this.push_parameter_to_stack(param, stack_offset, codegen));
            }
        }
        
        // 分配影子空间（Microsoft ABI要求）
        if (this.shadow_space > 0) {
            instructions.push(...codegen.encode_instruction('sub', ['rsp', this.shadow_space]));
        }
        
        return instructions;
    }
    
    push_parameter_to_stack(param, stack_offset, codegen) {
        const instructions = [];
        
        if (param.type === 'immediate') {
            instructions.push(...codegen.encode_instruction('mov', ['rax', param.value]));
            instructions.push(...codegen.encode_instruction('mov', [`[rsp+${stack_offset}]`, 'rax']));
        } else if (param.type === 'register') {
            instructions.push(...codegen.encode_instruction('mov', [`[rsp+${stack_offset}]`, param.value]));
        } else {
            // 复杂类型需要临时寄存器
            instructions.push(...codegen.encode_instruction('mov', ['rax', param]));
            instructions.push(...codegen.encode_instruction('mov', [`[rsp+${stack_offset}]`, 'rax']));
        }
        
        return instructions;
    }
    
    generate_stack_cleanup(param_count, codegen) {
        const instructions = [];
        
        // 清理影子空间和栈参数
        const total_cleanup = this.shadow_space + 
                            Math.max(0, param_count - this.parameter_registers.length) * 8;
        
        if (total_cleanup > 0) {
            instructions.push(...codegen.encode_instruction('add', ['rsp', total_cleanup]));
        }
        
        return instructions;
    }
    
    calculate_total_frame_size(frame_size) {
        // 包括局部变量、被调用者保存寄存器空间、对齐填充
        let total = frame_size;
        
        // 被调用者保存寄存器占用空间
        total += this.callee_saved_registers.length * 8;
        
        // 对齐到16字节
        if (total % 16 !== 0) {
            total += 16 - (total % 16);
        }
        
        return total;
    }
}
```

#### 3.2.3 x64特定优化

```javascript
class X64Optimizer {
    constructor() {
        this.optimizations = [
            this.optimize_register_usage.bind(this),
            this.optimize_memory_access.bind(this),
            this.optimize_instruction_selection.bind(this)
        ];
    }
    
    optimize_function(function_code) {
        let optimized_code = function_code.slice();
        
        for (const optimization of this.optimizations) {
            optimized_code = optimization(optimized_code);
        }
        
        return optimized_code;
    }
    
    optimize_register_usage(code) {
        // 寄存器分配优化 - 使用更多的x64寄存器
        const optimized = [];
        const register_pool = ['r10', 'r11', 'r12', 'r13', 'r14', 'r15'];
        let next_register = 0;
        
        for (const instruction of code) {
            if (this.uses_memory_temporary(instruction)) {
                // 用寄存器替换内存临时变量
                const new_instruction = this.replace_with_register(
                    instruction, 
                    register_pool[next_register % register_pool.length]
                );
                optimized.push(new_instruction);
                next_register++;
            } else {
                optimized.push(instruction);
            }
        }
        
        return optimized;
    }
    
    optimize_memory_access(code) {
        // 内存访问优化 - 利用x64的寻址模式
        const optimized = [];
        
        for (let i = 0; i < code.length; i++) {
            const instruction = code[i];
            
            if (this.is_redundant_memory_access(instruction, code[i + 1])) {
                // 跳过冗余的内存访问
                i++; // 跳过下一条指令
                continue;
            }
            
            if (this.can_use_relative_addressing(instruction)) {
                optimized.push(this.convert_to_relative_addressing(instruction));
            } else {
                optimized.push(instruction);
            }
        }
        
        return optimized;
    }
    
    optimize_instruction_selection(code) {
        // 指令选择优化 - 选择更高效的x64指令
        const optimized = [];
        
        for (const instruction of code) {
            if (this.is_inefficient_sequence(instruction)) {
                optimized.push(...this.replace_with_efficient_sequence(instruction));
            } else {
                optimized.push(instruction);
            }
        }
        
        return optimized;
    }
    
    uses_memory_temporary(instruction) {
        // 检测是否使用了内存临时变量
        return instruction.op === 'mov' && 
               instruction.operands[0].type === 'memory' &&
               instruction.operands[0].base === 'rsp';
    }
    
    is_redundant_memory_access(inst1, inst2) {
        // 检测冗余的内存加载/存储
        if (!inst1 || !inst2) return false;
        
        return inst1.op === 'mov' && inst2.op === 'mov' &&
               inst1.operands[0].type === 'register' &&
               inst1.operands[1].type === 'memory' &&
               inst2.operands[0].type === 'memory' &&
               inst2.operands[1].type === 'register' &&
               this.same_memory_location(inst1.operands[1], inst2.operands[0]);
    }
    
    can_use_relative_addressing(instruction) {
        // 检测是否可以使用RIP相对寻址
        return instruction.operands.some(op => 
            op.type === 'memory' && 
            op.base === 'rip'
        );
    }
    
    convert_to_relative_addressing(instruction) {
        // 转换为RIP相对寻址
        const new_instruction = { ...instruction };
        new_instruction.operands = new_instruction.operands.map(op => {
            if (op.type === 'memory' && op.base !== 'rip') {
                return { ...op, base: 'rip', displacement: op.displacement };
            }
            return op;
        });
        return new_instruction;
    }
}
```

### 3.3 x64运行时与系统调用 (第9-12周)

**目标**：实现x64特定的运行时支持

#### 3.3.1 x64系统调用接口

```javascript
class X64SystemCall {
    constructor() {
        this.syscall_numbers = new Map([
            ['exit', 0x3C],
            ['read', 0x00],
            ['write', 0x01],
            ['open', 0x02],
            ['close', 0x03],
            ['brk', 0x0C]
        ]);
    }
    
    generate_syscall_stub(syscall_name, parameters) {
        const generator = new FunctionGenerator(
            `syscall_${syscall_name}`, 
            'int', 
            'system_v' // 使用System V ABI进行系统调用
        );
        
        // 设置系统调用号
        const syscall_number = this.syscall_numbers.get(syscall_name);
        if (!syscall_number) {
            throw new Error(`未知的系统调用: ${syscall_name}`);
        }
        
        generator.generate_prologue();
        
        // 设置系统调用参数
        for (let i = 0; i < parameters.length; i++) {
            const param_addr = generator.frame.get_variable_address(`param_${i}`);
            generator.instructions.push(...generator.codegen.encode_instruction('mov', [
                generator.calling_convention.parameter_registers[i],
                param_addr
            ]));
        }
        
        // 设置系统调用号
        generator.instructions.push(...generator.codegen.encode_instruction('mov', ['rax', syscall_number]));
        
        // 执行系统调用
        generator.instructions.push(...generator.codegen.encode_instruction('syscall', []));
        
        generator.generate_epilogue();
        
        return generator.get_machine_code();
    }
    
    generate_windows_api_thunk(api_name, parameter_count) {
        // 为Windows API生成thunk函数，处理x64调用约定转换
        const generator = new FunctionGenerator(
            `thunk_${api_name}`,
            'uint64',
            'microsoft'
        );
        
        generator.generate_prologue();
        
        // 将参数从Microsoft ABI转换为Windows API期望的格式
        if (parameter_count > 0) {
            // 前4个参数已经在rcx, rdx, r8, r9中
            // 额外的参数在栈上 [rsp+32] 开始
            
            // 如果需要，重新排列参数
            for (let i = 0; i < Math.min(parameter_count, 4); i++) {
                const reg = generator.calling_convention.parameter_registers[i];
                generator.instructions.push(...generator.codegen.encode_instruction('mov', [
                    `shadow_${i}`, // 保存到影子空间
                    reg
                ]));
            }
        }
        
        // 调用实际的API
        generator.instructions.push(...generator.codegen.encode_instruction('call', [api_name]));
        
        generator.generate_epilogue();
        
        return generator.get_machine_code();
    }
}
```

#### 3.3.2 x64内存管理

```javascript
class X64MemoryManager {
    constructor(pe_builder) {
        this.pe_builder = pe_builder;
        this.heap_base = 0x0000000000200000; // 2MB处开始堆
        this.current_heap = this.heap_base;
        this.allocations = new Map();
    }
    
    generate_memory_allocator() {
        const generator = new FunctionGenerator('runtime_alloc', 'void*', 'microsoft');
        generator.add_parameter('size', 'size_t');
        
        generator.generate_prologue();
        
        // 简单的堆分配实现
        const heap_ptr_addr = generator.frame.add_local_variable('heap_ptr', 'void**');
        
        // 加载当前堆指针
        generator.instructions.push(...generator.codegen.encode_instruction('mov', [
            'rax',
            { type: 'immediate', value: this.current_heap }
        ]));
        
        // 保存返回地址
        generator.instructions.push(...generator.codegen.encode_instruction('mov', [
            'rdx',
            'rax'
        ]));
        
        // 增加堆指针
        const size_addr = generator.frame.get_variable_address('size');
        generator.instructions.push(...generator.codegen.encode_instruction('add', [
            'rax',
            size_addr
        ]));
        
        // 更新堆指针
        generator.instructions.push(...generator.codegen.encode_instruction('mov', [
            { type: 'memory', displacement: this.heap_base - 8 }, // 堆指针存储位置
            'rax'
        ]));
        
        // 返回分配的内存
        generator.instructions.push(...generator.codegen.encode_instruction('mov', [
            'rax',
            'rdx'
        ]));
        
        generator.generate_epilogue();
        
        return generator.get_machine_code();
    }
    
    generate_garbage_collector() {
        // 简单的标记清除垃圾收集器
        const generator = new FunctionGenerator('runtime_gc', 'void', 'microsoft');
        
        generator.generate_prologue();
        
        // GC实现会遍历堆，标记可达对象，然后清除未标记对象
        // 这里是一个简化的实现框架
        
        // 扫描栈寻找根对象
        generator.instructions.push(...this.generate_stack_scanning(generator));
        
        // 标记阶段
        generator.instructions.push(...this.generate_marking_phase(generator));
        
        // 清除阶段
        generator.instructions.push(...this.generate_sweeping_phase(generator));
        
        generator.generate_epilogue();
        
        return generator.get_machine_code();
    }
    
    generate_stack_scanning(generator) {
        const instructions = [];
        
        // 扫描当前栈帧寻找指针
        instructions.push(...generator.codegen.encode_instruction('mov', ['rcx', 'rsp']));
        instructions.push(...generator.codegen.encode_instruction('mov', ['rdx', 'rbp']));
        instructions.push(...generator.codegen.encode_instruction('call', ['scan_memory_range']));
        
        return instructions;
    }
}
```

## 阶段四：高级优化与生产部署 (第10-12个月)

### 4.1 高级代码优化 (第1-6周)

**目标**：实现工业级编译器优化

#### 4.1.1 控制流优化

```javascript
class ControlFlowOptimizer {
    constructor() {
        this.optimizations = [
            this.dead_code_elimination.bind(this),
            this.constant_propagation.bind(this),
            this.common_subexpression_elimination.bind(this),
            this.loop_optimization.bind(this)
        ];
    }
    
    optimize_control_flow(cfg) {
        let optimized_cfg = cfg;
        let changed = true;
        let iterations = 0;
        
        while (changed && iterations < 100) {
            changed = false;
            
            for (const optimization of this.optimizations) {
                const result = optimization(optimized_cfg);
                if (result.changed) {
                    optimized_cfg = result.cfg;
                    changed = true;
                }
            }
            
            iterations++;
        }
        
        return optimized_cfg;
    }
    
    dead_code_elimination(cfg) {
        const changed = false;
        
        // 构建使用定义链
        const use_def = this.build_use_def_chain(cfg);
        
        // 标记活跃变量
        const live_variables = this.compute_live_variables(cfg, use_def);
        
        // 移除死代码
        for (const block of cfg.blocks) {
            for (let i = block.instructions.length - 1; i >= 0; i--) {
                const instruction = block.instructions[i];
                
                if (this.is_dead_store(instruction, live_variables)) {
                    block.instructions.splice(i, 1);
                    changed = true;
                }
            }
        }
        
        return { cfg, changed };
    }
    
    constant_propagation(cfg) {
        const changed = false;
        const constants = new Map();
        
        for (const block of cfg.blocks) {
            for (const instruction of block.instructions) {
                if (this.is_constant_assignment(instruction)) {
                    // 记录常量值
                    const variable = instruction.operands[0].value;
                    const value = instruction.operands[1].value;
                    constants.set(variable, value);
                } else if (this.uses_constant(instruction, constants)) {
                    // 替换为常量
                    this.replace_with_constant(instruction, constants);
                    changed = true;
                }
            }
        }
        
        return { cfg, changed };
    }
    
    common_subexpression_elimination(cfg) {
        const changed = false;
        const expressions = new Map();
        
        for (const block of cfg.blocks) {
            for (const instruction of block.instructions) {
                const expression_key = this.get_expression_key(instruction);
                
                if (expressions.has(expression_key)) {
                    // 找到公共子表达式，替换为已计算的值
                    const existing_var = expressions.get(expression_key);
                    this.replace_expression(instruction, existing_var);
                    changed = true;
                } else {
                    // 记录新表达式
                    expressions.set(expression_key, instruction.operands[0].value);
                }
            }
        }
        
        return { cfg, changed };
    }
    
    loop_optimization(cfg) {
        const changed = false;
        
        // 识别循环
        const loops = this.identify_loops(cfg);
        
        for (const loop of loops) {
            // 循环不变代码外提
            changed = this.hoist_invariant_code(loop) || changed;
            
            // 归纳变量优化
            changed = this.optimize_induction_variables(loop) || changed;
            
            // 循环展开
            if (this.should_unroll(loop)) {
                changed = this.unroll_loop(loop) || changed;
            }
        }
        
        return { cfg, changed };
    }
    
    build_use_def_chain(cfg) {
        const use_def = new Map();
        
        for (const block of cfg.blocks) {
            for (const instruction of block.instructions) {
                const defs = this.get_instruction_defs(instruction);
                const uses = this.get_instruction_uses(instruction);
                
                use_def.set(instruction, { defs, uses });
            }
        }
        
        return use_def;
    }
    
    compute_live_variables(cfg, use_def) {
        const live_in = new Map();
        const live_out = new Map();
        
        // 初始化
        for (const block of cfg.blocks) {
            live_in.set(block, new Set());
            live_out.set(block, new Set());
        }
        
        let changed = true;
        while (changed) {
            changed = false;
            
            for (const block of cfg.blocks) {
                const old_live_in = new Set(live_in.get(block));
                const old_live_out = new Set(live_out.get(block));
                
                // live_out[block] = union of live_in[successor]
                const new_live_out = new Set();
                for (const successor of block.successors) {
                    const successor_live_in = live_in.get(successor);
                    for (const var_name of successor_live_in) {
                        new_live_out.add(var_name);
                    }
                }
                live_out.set(block, new_live_out);
                
                // live_in[block] = use[block] union (live_out[block] - def[block])
                const new_live_in = new Set(use_def.get(block).uses);
                for (const var_name of new_live_out) {
                    if (!use_def.get(block).defs.has(var_name)) {
                        new_live_in.add(var_name);
                    }
                }
                live_in.set(block, new_live_in);
                
                if (!this.sets_equal(old_live_in, new_live_in) || 
                    !this.sets_equal(old_live_out, new_live_out)) {
                    changed = true;
                }
            }
        }
        
        return { live_in, live_out };
    }
}
```

#### 4.1.2 机器相关优化

```javascript
class MachineDependentOptimizer {
    constructor(target_architecture) {
        this.architecture = target_architecture;
        this.register_allocator = new RegisterAllocator(target_architecture);
        this.instruction_scheduler = new InstructionScheduler(target_architecture);
        this.peephole_optimizer = new PeepholeOptimizer(target_architecture);
    }
    
    optimize_machine_code(instructions) {
        let optimized = instructions;
        
        // 寄存器分配
        optimized = this.register_allocator.allocate_registers(optimized);
        
        // 指令调度
        optimized = this.instruction_scheduler.schedule_instructions(optimized);
        
        // 窥孔优化
        optimized = this.peephole_optimizer.optimize(optimized);
        
        return optimized;
    }
}

class RegisterAllocator {
    constructor(architecture) {
        this.architecture = architecture;
        this.available_registers = this.get_available_registers();
    }
    
    allocate_registers(instructions) {
        // 图着色寄存器分配
        const interference_graph = this.build_interference_graph(instructions);
        const coloring = this.color_graph(interference_graph);
        
        return this.apply_coloring(instructions, coloring);
    }
    
    build_interference_graph(instructions) {
        const graph = new InterferenceGraph();
        const live_ranges = this.compute_live_ranges(instructions);
        
        for (const [var1, range1] of live_ranges) {
            for (const [var2, range2] of live_ranges) {
                if (var1 !== var2 && this.ranges_interfere(range1, range2)) {
                    graph.add_edge(var1, var2);
                }
            }
        }
        
        return graph;
    }
    
    color_graph(graph) {
        const coloring = new Map();
        const nodes = Array.from(graph.nodes());
        
        // 简化 - 总是选择第一个可用寄存器
        for (const node of nodes) {
            const used_colors = new Set();
            
            for (const neighbor of graph.neighbors(node)) {
                if (coloring.has(neighbor)) {
                    used_colors.add(coloring.get(neighbor));
                }
            }
            
            // 找到第一个未使用的颜色（寄存器）
            for (let i = 0; i < this.available_registers.length; i++) {
                if (!used_colors.has(i)) {
                    coloring.set(node, i);
                    break;
                }
            }
            
            // 如果所有寄存器都用了，需要溢出到内存
            if (!coloring.has(node)) {
                coloring.set(node, 'spill');
            }
        }
        
        return coloring;
    }
}

class InstructionScheduler {
    constructor(architecture) {
        this.architecture = architecture;
        this.dependency_analyzer = new DependencyAnalyzer();
    }
    
    schedule_instructions(instructions) {
        const dependencies = this.dependency_analyzer.analyze(instructions);
        const scheduled = [];
        const ready_list = [];
        
        // 找到没有依赖的指令
        for (const instruction of instructions) {
            if (dependencies.get(instruction).size === 0) {
                ready_list.push(instruction);
            }
        }
        
        while (ready_list.length > 0) {
            // 选择最优指令（基于延迟、资源冲突等）
            const best_instruction = this.select_best_instruction(ready_list);
            scheduled.push(best_instruction);
            
            // 更新依赖关系
            for (const instruction of instructions) {
                const deps = dependencies.get(instruction);
                deps.delete(best_instruction);
                
                if (deps.size === 0 && !scheduled.includes(instruction)) {
                    ready_list.push(instruction);
                }
            }
        }
        
        return scheduled;
    }
    
    select_best_instruction(ready_list) {
        // 简单的启发式：优先选择没有延迟的指令
        return ready_list[0];
    }
}

class PeepholeOptimizer {
    constructor(architecture) {
        this.architecture = architecture;
        this.patterns = this.get_optimization_patterns();
    }
    
    optimize(instructions) {
        let optimized = instructions.slice();
        let changed = true;
        
        while (changed) {
            changed = false;
            
            for (let i = 0; i < optimized.length - 1; i++) {
                const window = optimized.slice(i, i + 3); // 查看3条指令
                
                for (const pattern of this.patterns) {
                    if (this.matches_pattern(window, pattern)) {
                        const replacement = pattern.replacement(window);
                        optimized.splice(i, window.length, ...replacement);
                        changed = true;
                        break;
                    }
                }
            }
        }
        
        return optimized;
    }
    
    get_optimization_patterns() {
        return [
            {
                // mov eax, ebx; mov ebx, eax -> mov eax, ebx
                match: (instructions) => 
                    instructions.length >= 2 &&
                    instructions[0].op === 'mov' &&
                    instructions[1].op === 'mov' &&
                    instructions[0].operands[0].value === instructions[1].operands[1].value &&
                    instructions[0].operands[1].value === instructions[1].operands[0].value,
                replacement: (instructions) => [instructions[0]]
            },
            {
                // add eax, 0 -> (nothing)
                match: (instructions) => 
                    instructions.length >= 1 &&
                    instructions[0].op === 'add' &&
                    instructions[0].operands[1].type === 'immediate' &&
                    instructions[0].operands[1].value === 0,
                replacement: () => []
            },
            {
                // push; pop -> mov
                match: (instructions) => 
                    instructions.length >= 2 &&
                    instructions[0].op === 'push' &&
                    instructions[1].op === 'pop' &&
                    instructions[0].operands[0].value !== instructions[1].operands[0].value,
                replacement: (instructions) => [
                    {
                        op: 'mov',
                        operands: [
                            instructions[1].operands[0],
                            instructions[0].operands[0]
                        ]
                    }
                ]
            }
        ];
    }
}
```

### 4.2 链接器与可执行文件生成 (第7-9周)

**目标**：实现完整的链接器功能

#### 4.2.1 符号解析与重定位

```javascript
class Linker {
    constructor() {
        this.object_files = [];
        this.symbol_table = new Map();
        this.relocations = [];
        this.sections = new Map();
        this.base_address = 0x400000;
    }

    add_object_file(object_file) {
        this.object_files.push(object_file);
        this.process_object_file(object_file);
    }

    process_object_file(object_file) {
        // 处理符号
        for (const symbol of object_file.symbols) {
            if (this.symbol_table.has(symbol.name) && symbol.is_global) {
                throw new Error(`重复的符号定义: ${symbol.name}`);
            }

            if (symbol.is_global || !this.symbol_table.has(symbol.name)) {
                this.symbol_table.set(symbol.name, {
                    ...symbol,
                    object_file: object_file,
                    resolved: false
                });
            }
        }

        // 处理重定位
        for (const relocation of object_file.relocations) {
            this.relocations.push({
                ...relocation,
                object_file: object_file
            });
        }

        // 处理节
        for (const [section_name, section] of object_file.sections) {
            if (!this.sections.has(section_name)) {
                this.sections.set(section_name, {
                    name: section_name,
                    data: [],
                    virtual_address: 0,
                    characteristics: section.characteristics,
                    contributions: []
                });
            }

            this.sections.get(section_name).contributions.push({
                object_file: object_file,
                data: section.data,
                size: section.data.length,
                relocations: section.relocations
            });
        }
    }

    link() {
        // 解析所有符号
        this.resolve_symbols();

        // 分配虚拟地址
        this.assign_virtual_addresses();

        // 应用重定位
        this.apply_relocations();

        // 生成最终映像
        return this.generate_executable();
    }

    resolve_symbols() {
        let changed = true;

        while (changed) {
            changed = false;

            for (const [name, symbol] of this.symbol_table) {
                if (!symbol.resolved && symbol.is_defined) {
                    // 已定义的符号现在可以解析
                    symbol.resolved = true;
                    changed = true;
                }
            }

            // 解析外部引用
            for (const relocation of this.relocations) {
                const symbol = this.symbol_table.get(relocation.symbol_name);
                if (symbol && symbol.resolved && !relocation.resolved) {
                    relocation.resolved = true;
                    relocation.resolved_address = symbol.value;
                    changed = true;
                }
            }
        }

        // 检查未解析的符号
        const unresolved = Array.from(this.symbol_table.values())
            .filter(s => !s.resolved && s.is_global);

        if (unresolved.length > 0) {
            throw new Error(`未解析的符号: ${unresolved.map(s => s.name).join(', ')}`);
        }
    }

    assign_virtual_addresses() {
        let current_rva = 0x1000; // 第一个节从RVA 0x1000开始

        for (const [name, section] of this.sections) {
            section.virtual_address = current_rva;

            // 计算节大小（包括所有贡献）
            let section_size = 0;
            for (const contribution of section.contributions) {
                section_size += contribution.size;
            }

            // 对齐到节对齐
            section_size = this.align_to(section_size, 0x1000);
            current_rva += section_size;
        }
    }

    apply_relocations() {
        for (const section of this.sections.values()) {
            // 合并所有贡献的数据
            let section_data = new Uint8Array(this.calculate_section_size(section));
            let current_offset = 0;

            for (const contribution of section.contributions) {
                // 复制数据
                section_data.set(contribution.data, current_offset);

                // 应用重定位
                for (const relocation of contribution.relocations) {
                    const symbol = this.symbol_table.get(relocation.symbol_name);
                    if (!symbol) {
                        throw new Error(`未找到符号: ${relocation.symbol_name}`);
                    }

                    const target_address = symbol.value + relocation.addend;
                    const relocation_offset = current_offset + relocation.offset;

                    this.apply_single_relocation(
                        section_data,
                        relocation_offset,
                        target_address,
                        relocation.type
                    );
                }

                current_offset += contribution.size;
            }

            section.data = section_data;
        }
    }

    apply_single_relocation(data, offset, target_address, type) {
        switch (type) {
            case 'REL32':
                // 32位相对地址
                const relative_offset = target_address - (this.base_address + offset + 4);
                this.write_u32(data, offset, relative_offset);
                break;

            case 'ABS32':
                // 32位绝对地址
                this.write_u32(data, offset, target_address);
                break;

            case 'ABS64':
                // 64位绝对地址
                this.write_u64(data, offset, target_address);
                break;

            default:
                throw new Error(`未知的重定位类型: ${type}`);
        }
    }

    generate_executable() {
        const pe_builder = new PeAssembler(this.architecture);

        // 添加所有节
        for (const section of this.sections.values()) {
            pe_builder.add_section(
                section.name,
                section.characteristics,
                section.data
            );
        }

        // 设置入口点
        const entry_symbol = this.symbol_table.get('_start') || this.symbol_table.get('main');
        if (entry_symbol) {
            pe_builder.set_entry_point(entry_symbol.value);
        }

        // 生成导入表（如果需要）
        this.generate_imports(pe_builder);

        return pe_builder.build();
    }
}
```
