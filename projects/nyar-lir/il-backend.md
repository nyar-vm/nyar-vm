# 基于现有架构实现 .NET ILASM 功能

基于我们现有的 PE 文件生成器和 x86/x64 后端，实现 .NET IL 汇编器功能需要构建一个完整的 .NET 模块生成系统。以下是详细的实现计划：

## 阶段一：.NET 模块基础架构 (第1-3个月)

### 1.1 .NET 模块头与元数据结构

```javascript
class DotNetModuleBuilder {
    constructor(module_name, output_type = 'dll') {
        this.module_name = module_name;
        this.output_type = output_type;

        // .NET 特定的数据结构
        this.metadata_tables = new Map();
        this.heap_data = new Map(); // #Strings, #US, #GUID, #Blob
        this.assembly_info = null;
        this.type_defs = [];
        this.method_defs = [];
        this.field_defs = [];
        this.custom_attributes = [];

        // 重用现有的 PE 构建器
        this.pe_builder = new PeAssembler('x86', output_type);
        this.init_dotnet_sections();
    }

    init_dotnet_sections() {
        // .NET 特定的节
        this.pe_builder.add_section('.text', 0x60000020);  // 代码和元数据
        this.pe_builder.add_section('.rsrc', 0x40000040);  // 资源
        this.pe_builder.add_section('.reloc', 0x42000040); // 重定位
    }

    generate_cor20_header() {
        const writer = new BinaryWriter();

        // IMAGE_COR20_HEADER
        writer.write_u32(0x48);                   // cb
        writer.write_u16(0x0002);                 // MajorRuntimeVersion
        writer.write_u16(0x0005);                 // MinorRuntimeVersion
        writer.write_u32(this.get_metadata_rva()); // MetaData
        writer.write_u32(0x00000004);             // Flags: ILOnly
        writer.write_u32(0x00000000);             // EntryPointToken
        writer.write_u32(0x00000000);             // Resources
        writer.write_u32(0x00000000);             // StrongNameSignature
        writer.write_u32(0x00000000);             // CodeManagerTable
        writer.write_u32(0x00000000);             // VTableFixups
        writer.write_u32(0x00000000);             // ExportAddressTableJumps
        writer.write_u32(0x00000000);             // ManagedNativeHeader

        return writer.get_bytes();
    }

    generate_metadata_root() {
        const writer = new BinaryWriter();

        // 存储签名
        writer.write_u32(0x424A5342); // BSJB

        // 主版本和次版本
        writer.write_u16(1);
        writer.write_u16(1);

        // 保留
        writer.write_u32(0);

        // 版本字符串长度
        const version_string = "v4.0.30319\0";
        writer.write_u32(version_string.length);
        writer.write_cstring(version_string);

        // 标志和流数量
        writer.write_u16(0);
        writer.write_u16(5); // 5个流

        // 流头
        this.write_stream_header(writer, "#~", this.generate_tilde_stream());
        this.write_stream_header(writer, "#Strings", this.generate_strings_heap());
        this.write_stream_header(writer, "#US", this.generate_us_heap());
        this.write_stream_header(writer, "#GUID", this.generate_guid_heap());
        this.write_stream_header(writer, "#Blob", this.generate_blob_heap());

        return writer.get_bytes();
    }

    write_stream_header(writer, name, data) {
        writer.write_u32(this.align_to_4(data.offset)); // Offset
        writer.write_u32(data.size);                   // Size
        writer.write_cstring(name);                    // Name
        writer.align(4);                              // 对齐到4字节
    }
}
```

### 1.2 元数据表系统

```javascript
class MetadataTables {
    constructor() {
        this.tables = new Map();
        this.valid_tables = new Set([
            'Module', 'TypeRef', 'TypeDef', 'Field', 'Method', 'Param', 
            'InterfaceImpl', 'MemberRef', 'Constant', 'CustomAttribute',
            'Assembly', 'AssemblyRef', 'File', 'ExportedType', 'ManifestResource'
        ]);
        
        this.init_tables();
    }
    
    init_tables() {
        for (const table_name of this.valid_tables) {
            this.tables.set(table_name, []);
        }
    }
    
    add_table_row(table_name, row_data) {
        if (!this.valid_tables.has(table_name)) {
            throw new Error(`无效的元数据表: ${table_name}`);
        }
        
        const table = this.tables.get(table_name);
        const row_index = table.length;
        table.push(row_data);
        
        return (this.get_table_index(table_name) << 24) | row_index;
    }
    
    get_table_index(table_name) {
        const table_order = Array.from(this.valid_tables);
        return table_order.indexOf(table_name) + 1;
    }
    
    generate_tilde_stream() {
        const writer = new BinaryWriter();
        
        // 保留
        writer.write_u32(0);
        
        // 主版本和次版本
        writer.write_u8(2); // 主版本
        writer.write_u8(0); // 次版本
        
        // 堆大小和有效表标志
        writer.write_u8(0x03); // 堆大小: #String | #Blob
        writer.write_u8(0x00); // 保留
        
        // 有效表掩码
        const valid_tables = this.calculate_valid_tables_mask();
        writer.write_u64(valid_tables);
        
        // 排序的表掩码
        writer.write_u64(0);
        
        // 行计数
        for (const table_name of this.valid_tables) {
            if (this.is_table_valid(table_name)) {
                writer.write_u32(this.tables.get(table_name).length);
            }
        }
        
        // 表数据
        for (const table_name of this.valid_tables) {
            if (this.is_table_valid(table_name)) {
                this.write_table_data(writer, table_name);
            }
        }
        
        return {
            offset: 0,
            size: writer.get_bytes().length,
            data: writer.get_bytes()
        };
    }
    
    calculate_valid_tables_mask() {
        let mask = 0n;
        let bit_position = 0n;
        
        for (const table_name of this.valid_tables) {
            if (this.is_table_valid(table_name)) {
                mask |= (1n << bit_position);
            }
            bit_position++;
        }
        
        return mask;
    }
    
    is_table_valid(table_name) {
        return this.tables.get(table_name).length > 0;
    }
    
    write_table_data(writer, table_name) {
        const table = this.tables.get(table_name);
        
        for (const row of table) {
            this.write_table_row(writer, table_name, row);
        }
    }
    
    write_table_row(writer, table_name, row) {
        switch (table_name) {
            case 'Module':
                this.write_module_row(writer, row);
                break;
            case 'TypeDef':
                this.write_typedef_row(writer, row);
                break;
            case 'Method':
                this.write_method_row(writer, row);
                break;
            case 'Field':
                this.write_field_row(writer, row);
                break;
            case 'Assembly':
                this.write_assembly_row(writer, row);
                break;
            // 其他表...
        }
    }
    
    write_module_row(writer, row) {
        writer.write_u16(0); // Generation
        writer.write_string_index(row.name); // Name
        writer.write_guid_index(row.mvid); // Mvid
        writer.write_guid_index(0); // EncId
        writer.write_guid_index(0); // EncBaseId
    }
    
    write_typedef_row(writer, row) {
        writer.write_u32(row.flags);
        writer.write_string_index(row.type_name);
        writer.write_string_index(row.namespace);
        writer.write_type_def_or_ref(row.extends);
        writer.write_field_index(row.field_list);
        writer.write_method_index(row.method_list);
    }
    
    write_method_row(writer, row) {
        writer.write_u32(row.rva);
        writer.write_u16(row.impl_flags);
        writer.write_u16(row.flags);
        writer.write_string_index(row.name);
        writer.write_blob_index(row.signature);
        writer.write_param_index(row.param_list);
    }
}
```

### 1.3 #Strings, #US, #GUID, #Blob 堆

```javascript
class MetadataHeaps {
    constructor() {
        this.strings_heap = new Map();
        this.us_heap = new Map();        // User Strings
        this.guid_heap = new Map();
        this.blob_heap = new Map();
        
        this.strings_offset = 1;  // 从1开始，0为空
        this.us_offset = 1;
        this.guid_offset = 1;
        this.blob_offset = 1;
    }
    
    add_string(str) {
        if (this.strings_heap.has(str)) {
            return this.strings_heap.get(str);
        }
        
        const offset = this.strings_offset;
        this.strings_heap.set(str, offset);
        
        // 计算下一个偏移量 (字符串长度 + null终止符)
        this.strings_offset += str.length + 1;
        
        return offset;
    }
    
    add_user_string(str) {
        if (this.us_heap.has(str)) {
            return this.us_heap.get(str);
        }
        
        const offset = this.us_offset;
        this.us_heap.set(str, offset);
        
        // UTF-16编码，带长度前缀和终止符
        const encoded_length = str.length * 2 + 3; // 长度前缀(1) + 数据 + 终止符(2)
        this.us_offset += encoded_length;
        
        return offset;
    }
    
    add_guid(guid) {
        if (this.guid_heap.has(guid)) {
            return this.guid_heap.get(guid);
        }
        
        const offset = this.guid_offset;
        this.guid_heap.set(guid, offset);
        
        this.guid_offset += 16; // GUID是16字节
        
        return offset;
    }
    
    add_blob(data) {
        const key = this.get_blob_key(data);
        if (this.blob_heap.has(key)) {
            return this.blob_heap.get(key);
        }
        
        const offset = this.blob_offset;
        this.blob_heap.set(key, offset);
        
        // 压缩长度前缀
        let length_bytes;
        if (data.length < 0x80) {
            length_bytes = 1;
        } else if (data.length < 0x4000) {
            length_bytes = 2;
        } else {
            length_bytes = 4;
        }
        
        this.blob_offset += length_bytes + data.length;
        
        return offset;
    }
    
    generate_strings_heap() {
        const writer = new BinaryWriter();
        
        // 第一个字节是0
        writer.write_u8(0);
        
        // 按偏移量排序的字符串
        const sorted_strings = Array.from(this.strings_heap.entries())
            .sort((a, b) => a[1] - b[1]);
        
        for (const [str, offset] of sorted_strings) {
            // 确保我们在正确的位置
            while (writer.get_bytes().length < offset) {
                writer.write_u8(0);
            }
            writer.write_cstring(str);
        }
        
        return {
            offset: 0,
            size: writer.get_bytes().length,
            data: writer.get_bytes()
        };
    }
    
    generate_us_heap() {
        const writer = new BinaryWriter();
        
        writer.write_u8(0); // 第一个字节是0
        
        const sorted_strings = Array.from(this.us_heap.entries())
            .sort((a, b) => a[1] - b[1]);
        
        for (const [str, offset] of sorted_strings) {
            while (writer.get_bytes().length < offset) {
                writer.write_u8(0);
            }
            
            // 写入压缩长度
            this.write_compressed_int(writer, str.length);
            
            // 写入UTF-16数据
            for (let i = 0; i < str.length; i++) {
                writer.write_u16(str.charCodeAt(i));
            }
            
            // 终止符
            writer.write_u16(0);
        }
        
        return {
            offset: 0,
            size: writer.get_bytes().length,
            data: writer.get_bytes()
        };
    }
    
    generate_blob_heap() {
        const writer = new BinaryWriter();
        
        writer.write_u8(0); // 第一个字节是0
        
        const sorted_blobs = Array.from(this.blob_heap.entries())
            .sort((a, b) => a[1] - b[1]);
        
        for (const [key, offset] of sorted_blobs) {
            const data = this.get_blob_data(key);
            
            while (writer.get_bytes().length < offset) {
                writer.write_u8(0);
            }
            
            // 写入压缩长度
            this.write_compressed_int(writer, data.length);
            
            // 写入数据
            writer.write_bytes(data);
        }
        
        return {
            offset: 0,
            size: writer.get_bytes().length,
            data: writer.get_bytes()
        };
    }
    
    write_compressed_int(writer, value) {
        if (value <= 0x7F) {
            writer.write_u8(value);
        } else if (value <= 0x3FFF) {
            writer.write_u16(0x8000 | value);
        } else {
            writer.write_u32(0xC0000000 | value);
        }
    }
}
```

## 阶段二：IL 指令集与代码生成 (第4-6个月)

### 2.1 IL 指令编码系统

```javascript
class ILInstructionEncoder {
    constructor() {
        this.instructions = [];
        this.labels = new Map();
        this.exceptions = [];
    }
    
    encode_instruction(opcode, operand = null) {
        const instruction = {
            opcode: opcode,
            operand: operand,
            offset: this.get_current_offset()
        };
        
        this.instructions.push(instruction);
        return instruction;
    }
    
    // 加载指令
    ldarg(index) {
        if (index <= 3) {
            const opcodes = [0x02, 0x03, 0x04, 0x05]; // ldarg.0 to ldarg.3
            return this.encode_instruction(opcodes[index]);
        } else if (index <= 255) {
            return this.encode_instruction(0x0E, index); // ldarg.s
        } else {
            return this.encode_instruction(0xFE09, index); // ldarg
        }
    }
    
    ldloc(index) {
        if (index <= 3) {
            const opcodes = [0x06, 0x07, 0x08, 0x09]; // ldloc.0 to ldloc.3
            return this.encode_instruction(opcodes[index]);
        } else if (index <= 255) {
            return this.encode_instruction(0x11, index); // ldloc.s
        } else {
            return this.encode_instruction(0xFE0C, index); // ldloc
        }
    }
    
    ldc_i4(value) {
        if (value >= -1 && value <= 8) {
            const opcodes = [
                0x16, // ldc.i4.m1
                0x17, 0x18, 0x19, 0x1A, // ldc.i4.0 to ldc.i4.3
                0x1B, 0x1C, 0x1D, 0x1E  // ldc.i4.4 to ldc.i4.8
            ];
            const opcode = value === -1 ? 0x16 : opcodes[value + 1];
            return this.encode_instruction(opcode);
        } else if (value >= -128 && value <= 127) {
            return this.encode_instruction(0x1F, value); // ldc.i4.s
        } else {
            return this.encode_instruction(0x20, value); // ldc.i4
        }
    }
    
    // 存储指令
    stloc(index) {
        if (index <= 3) {
            const opcodes = [0x0A, 0x0B, 0x0C, 0x0D]; // stloc.0 to stloc.3
            return this.encode_instruction(opcodes[index]);
        } else if (index <= 255) {
            return this.encode_instruction(0x13, index); // stloc.s
        } else {
            return this.encode_instruction(0xFE0E, index); // stloc
        }
    }
    
    // 方法调用
    call(token) {
        return this.encode_instruction(0x28, token); // call
    }
    
    callvirt(token) {
        return this.encode_instruction(0x6F, token); // callvirt
    }
    
    // 分支指令
    br(target) {
        return this.encode_instruction(0x38, target); // br
    }
    
    br_s(target) {
        return this.encode_instruction(0x2B, target); // br.s
    }
    
    beq(target) {
        return this.encode_instruction(0x3B, target); // beq
    }
    
    bne_un(target) {
        return this.encode_instruction(0x40, target); // bne.un
    }
    
    // 算术指令
    add() {
        return this.encode_instruction(0x58); // add
    }
    
    sub() {
        return this.encode_instruction(0x59); // sub
    }
    
    mul() {
        return this.encode_instruction(0x5A); // mul
    }
    
    div() {
        return this.encode_instruction(0x5B); // div
    }
    
    // 返回指令
    ret() {
        return this.encode_instruction(0x2A); // ret
    }
    
    // 标签管理
    define_label(name) {
        const label = {
            name: name,
            offset: this.get_current_offset()
        };
        this.labels.set(name, label);
        return label;
    }
    
    mark_label(label) {
        label.offset = this.get_current_offset();
    }
    
    // 生成方法体
    generate_method_body(local_variables = []) {
        const writer = new BinaryWriter();
        
        // 方法头
        const header = this.generate_method_header(local_variables);
        writer.write_bytes(header);
        
        // 指令
        const code = this.generate_instructions();
        writer.write_bytes(code);
        
        // 异常处理表
        if (this.exceptions.length > 0) {
            const exception_table = this.generate_exception_table();
            writer.write_bytes(exception_table);
        }
        
        return writer.get_bytes();
    }
    
    generate_method_header(local_variables) {
        const writer = new BinaryWriter();
        
        const flags = this.calculate_method_flags();
        const code_size = this.calculate_code_size();
        const local_count = local_variables.length;
        
        if (flags === 0x00 && local_count === 0) {
            // Tiny格式
            writer.write_u8((code_size << 2) | 0x02);
        } else {
            // Fat格式
            writer.write_u16(0x3000 | flags); // 标志位
            writer.write_u16(0x00);           // 头部大小
            writer.write_u32(code_size);
            writer.write_u32(0);              // 本地变量签名
            writer.write_u16(0);              // 最大栈
            writer.write_u16(local_count);    // 本地变量计数
        }
        
        return writer.get_bytes();
    }
    
    generate_instructions() {
        const writer = new BinaryWriter();
        
        for (const instruction of this.instructions) {
            // 写入操作码
            if (instruction.opcode > 0xFF) {
                writer.write_u16(instruction.opcode);
            } else {
                writer.write_u8(instruction.opcode);
            }
            
            // 写入操作数
            if (instruction.operand !== null) {
                this.write_operand(writer, instruction);
            }
        }
        
        return writer.get_bytes();
    }
    
    write_operand(writer, instruction) {
        const operand = instruction.operand;
        
        if (typeof operand === 'number') {
            if (instruction.opcode === 0x1F || instruction.opcode === 0x20) {
                // ldc.i4.s 或 ldc.i4
                writer.write_u32(operand);
            } else if (instruction.opcode >= 0x00 && instruction.opcode <= 0xFF) {
                // 短格式指令
                writer.write_u8(operand);
            } else {
                // 长格式指令
                writer.write_u32(operand);
            }
        } else if (operand.type === 'label') {
            // 分支目标
            const target_label = this.labels.get(operand.name);
            const offset = target_label.offset - instruction.offset;
            
            if (instruction.opcode === 0x2B || instruction.opcode === 0x2C) {
                // 短分支
                writer.write_u8(offset);
            } else {
                // 长分支
                writer.write_u32(offset);
            }
        } else if (operand.type === 'token') {
            // 元数据令牌
            writer.write_u32(operand.value);
        }
    }
}
```

### 2.2 方法签名与类型系统

```javascript
class ILTypeSystem {
    constructor() {
        this.type_references = new Map();
        this.method_references = new Map();
        this.field_references = new Map();
    }
    
    encode_type_signature(type) {
        const writer = new BinaryWriter();
        
        switch (type.kind) {
            case 'primitive':
                this.encode_primitive_type(writer, type);
                break;
            case 'class':
                this.encode_class_type(writer, type);
                break;
            case 'array':
                this.encode_array_type(writer, type);
                break;
            case 'generic':
                this.encode_generic_type(writer, type);
                break;
            default:
                throw new Error(`未知的类型种类: ${type.kind}`);
        }
        
        return writer.get_bytes();
    }
    
    encode_primitive_type(writer, type) {
        const type_codes = {
            'void': 0x01,
            'bool': 0x02,
            'char': 0x03,
            'i1': 0x04,   // sbyte
            'u1': 0x05,   // byte
            'i2': 0x06,   // short
            'u2': 0x07,   // ushort
            'i4': 0x08,   // int
            'u4': 0x09,   // uint
            'i8': 0x0A,   // long
            'u8': 0x0B,   // ulong
            'r4': 0x0C,   // float
            'r8': 0x0D,   // double
            'string': 0x0E,
            'object': 0x1C
        };
        
        writer.write_u8(type_codes[type.name] || 0x1C); // 默认为object
    }
    
    encode_method_signature(method) {
        const writer = new BinaryWriter();
        
        // 调用约定
        let calling_convention = 0x00; // DEFAULT
        if (method.has_this) calling_convention |= 0x20;
        if (method.explicit_this) calling_convention |= 0x40;
        if (method.vararg) calling_convention |= 0x05;
        
        writer.write_u8(calling_convention);
        
        // 参数计数
        writer.write_compressed_int(method.parameters.length);
        
        // 返回类型
        this.encode_type_signature(writer, method.return_type);
        
        // 参数类型
        for (const param of method.parameters) {
            this.encode_type_signature(writer, param.type);
        }
        
        return writer.get_bytes();
    }
    
    encode_field_signature(field) {
        const writer = new BinaryWriter();
        
        writer.write_u8(0x06); // FIELD签名
        
        this.encode_type_signature(writer, field.type);
        
        return writer.get_bytes();
    }
    
    get_type_ref_token(type_name, namespace = '', assembly = null) {
        const key = `${namespace}.${type_name}`;
        
        if (this.type_references.has(key)) {
            return this.type_references.get(key);
        }
        
        // 在元数据中添加 TypeRef
        const token = this.metadata_tables.add_table_row('TypeRef', {
            resolution_scope: assembly ? this.get_assembly_ref_token(assembly) : 0,
            type_name: this.heaps.add_string(type_name),
            namespace: this.heaps.add_string(namespace)
        });
        
        this.type_references.set(key, token);
        return token;
    }
    
    get_method_ref_token(type_token, method_name, signature) {
        const key = `${type_token}:${method_name}`;
        
        if (this.method_references.has(key)) {
            return this.method_references.get(key);
        }
        
        const signature_blob = this.encode_method_signature(signature);
        const signature_index = this.heaps.add_blob(signature_blob);
        
        const token = this.metadata_tables.add_table_row('MemberRef', {
            class: type_token,
            name: this.heaps.add_string(method_name),
            signature: signature_index
        });
        
        this.method_references.set(key, token);
        return token;
    }
}
```

## 阶段三：DLL 特定功能与程序集生成 (第7-9个月)

### 3.1 程序集清单生成

```javascript
class AssemblyManifestGenerator {
    constructor(assembly_name, version = '1.0.0.0', culture = 'neutral') {
        this.assembly_name = assembly_name;
        this.version = version;
        this.culture = culture;
        this.public_key = null;
        this.references = [];
        this.files = [];
        this.resources = [];
    }
    
    generate_assembly_definition() {
        const token = this.metadata_tables.add_table_row('Assembly', {
            hash_alg_id: 0x00008004, // SHA1
            major_version: this.get_version_part(0),
            minor_version: this.get_version_part(1),
            build_number: this.get_version_part(2),
            revision_number: this.get_version_part(3),
            flags: this.public_key ? 0x0001 : 0x0000, // PublicKey
            public_key: this.public_key ? this.heaps.add_blob(this.public_key) : 0,
            name: this.heaps.add_string(this.assembly_name),
            culture: this.heaps.add_string(this.culture)
        });
        
        return token;
    }
    
    generate_assembly_reference(assembly_name, version, public_key_token = null) {
        const token = this.metadata_tables.add_table_row('AssemblyRef', {
            major_version: this.get_version_part(0, version),
            minor_version: this.get_version_part(1, version),
            build_number: this.get_version_part(2, version),
            revision_number: this.get_version_part(3, version),
            flags: public_key_token ? 0x0001 : 0x0000,
            public_key_or_token: public_key_token ? this.heaps.add_blob(public_key_token) : 0,
            name: this.heaps.add_string(assembly_name),
            culture: this.heaps.add_string('neutral'),
            hash_value: 0 // 通常为空
        });
        
        this.references.push(token);
        return token;
    }
    
    generate_module_definition() {
        const mvid = this.generate_guid();
        const token = this.metadata_tables.add_table_row('Module', {
            name: this.heaps.add_string(this.assembly_name + '.dll'),
            mvid: this.heaps.add_guid(mvid),
            enc_id: 0,
            enc_base_id: 0
        });
        
        return token;
    }
    
    generate_file_reference(file_name, hash_value = null) {
        const token = this.metadata_tables.add_table_row('File', {
            flags: 0x0000, // ContainsMetadata
            name: this.heaps.add_string(file_name),
            hash_value: hash_value ? this.heaps.add_blob(hash_value) : 0
        });
        
        this.files.push(token);
        return token;
    }
    
    generate_manifest_resource(resource_name, implementation, offset = 0) {
        const token = this.metadata_tables.add_table_row('ManifestResource', {
            offset: offset,
            flags: implementation ? 0x0002 : 0x0001, // Public | Private
            name: this.heaps.add_string(resource_name),
            implementation: implementation || 0
        });
        
        this.resources.push(token);
        return token;
    }
    
    get_version_part(index, version_string = null) {
        const version = version_string || this.version;
        const parts = version.split('.');
        return parseInt(parts[index] || '0', 10);
    }
    
    generate_guid() {
        // 生成随机的GUID
        return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
            const r = Math.random() * 16 | 0;
            const v = c == 'x' ? r : (r & 0x3 | 0x8);
            return v.toString(16);
        });
    }
}
```

### 3.2 类型定义与方法实现

```javascript
class TypeDefinitionGenerator {
    constructor(type_name, namespace = '', base_type = null) {
        this.type_name = type_name;
        this.namespace = namespace;
        this.base_type = base_type || this.get_object_type_ref();
        this.flags = 0x00000000; // 默认可见性
        this.fields = [];
        this.methods = [];
        this.properties = [];
        this.events = [];
        this.custom_attributes = [];
    }
    
    set_flags(flags) {
        this.flags = flags;
        return this;
    }
    
    add_field(field_name, field_type, flags = 0x0000) {
        const field = {
            name: field_name,
            type: field_type,
            flags: flags,
            default_value: null,
            offset: null
        };
        
        this.fields.push(field);
        return field;
    }
    
    add_method(method_name, return_type, parameters = [], flags = 0x0000) {
        const method = {
            name: method_name,
            return_type: return_type,
            parameters: parameters,
            flags: flags,
            impl_flags: 0x0000,
            body: null,
            custom_attributes: []
        };
        
        this.methods.push(method);
        return method;
    }
    
    generate_type_definition() {
        const type_token = this.metadata_tables.add_table_row('TypeDef', {
            flags: this.flags,
            type_name: this.heaps.add_string(this.type_name),
            namespace: this.heaps.add_string(this.namespace),
            extends: this.base_type,
            field_list: this.generate_field_definitions(),
            method_list: this.generate_method_definitions()
        });
        
        return type_token;
    }
    
    generate_field_definitions() {
        let first_field_token = null;
        
        for (const field of this.fields) {
            const signature_blob = this.type_system.encode_field_signature(field);
            const signature_index = this.heaps.add_blob(signature_blob);
            
            const field_token = this.metadata_tables.add_table_row('Field', {
                flags: field.flags,
                name: this.heaps.add_string(field.name),
                signature: signature_index
            });
            
            if (!first_field_token) {
                first_field_token = field_token;
            }
            
            // 生成常量值（如果有）
            if (field.default_value !== null) {
                this.generate_constant(field_token, field.default_value);
            }
        }
        
        return first_field_token;
    }
    
    generate_method_definitions() {
        let first_method_token = null;
        
        for (const method of this.methods) {
            const signature_blob = this.type_system.encode_method_signature(method);
            const signature_index = this.heaps.add_blob(signature_blob);
            
            const method_token = this.metadata_tables.add_table_row('Method', {
                rva: method.body ? this.allocate_method_body(method.body) : 0,
                impl_flags: method.impl_flags,
                flags: method.flags,
                name: this.heaps.add_string(method.name),
                signature: signature_index,
                param_list: this.generate_parameter_definitions(method.parameters)
            });
            
            if (!first_method_token) {
                first_method_token = method_token;
            }
            
            // 生成自定义属性（如果有）
            for (const attr of method.custom_attributes) {
                this.generate_custom_attribute(method_token, attr);
            }
        }
        
        return first_method_token;
    }
    
    generate_parameter_definitions(parameters) {
        let first_param_token = null;
        
        for (let i = 0; i < parameters.length; i++) {
            const param = parameters[i];
            const param_token = this.metadata_tables.add_table_row('Param', {
                flags: param.flags || 0x0000,
                sequence: i + 1, // 参数索引（0是返回值）
                name: param.name ? this.heaps.add_string(param.name) : 0
            });
            
            if (!first_param_token) {
                first_param_token = param_token;
            }
        }
        
        return first_param_token;
    }
    
    allocate_method_body(il_code) {
        // 在.text节中分配方法体空间
        const text_section = this.pe_builder.sections.get('.text');
        const offset = text_section.data.length;
        
        // 将IL代码添加到.text节
        const method_body = il_code.generate_method_body();
        text_section.data = this.concat_arrays(text_section.data, method_body);
        
        // 返回RVA
        return text_section.virtual_address + offset;
    }
}
```

### 3.3 完整的 DLL 生成流程

```javascript
class DotNetDllBuilder {
    constructor(assembly_name) {
        this.assembly_name = assembly_name;
        this.module_builder = new DotNetModuleBuilder(assembly_name, 'dll');
        this.manifest_generator = new AssemblyManifestGenerator(assembly_name);
        this.type_system = new ILTypeSystem();
        
        this.types = [];
        this.references = [];
    }
    
    add_reference(assembly_name, version, public_key_token = null) {
        const ref_token = this.manifest_generator.generate_assembly_reference(
            assembly_name, version, public_key_token
        );
        this.references.push(ref_token);
        return ref_token;
    }
    
    define_type(type_name, namespace = '') {
        const type_generator = new TypeDefinitionGenerator(type_name, namespace);
        this.types.push(type_generator);
        return type_generator;
    }
    
    build() {
        // 1. 生成程序集定义
        this.manifest_generator.generate_assembly_definition();
        
        // 2. 生成模块定义
        this.manifest_generator.generate_module_definition();
        
        // 3. 生成所有类型定义
        for (const type of this.types) {
            type.generate_type_definition();
        }
        
        // 4. 生成元数据
        const metadata = this.module_builder.generate_metadata_root();
        
        // 5. 生成 COR20 头
        const cor20_header = this.module_builder.generate_cor20_header();
        
        // 6. 构建 PE 文件
        const pe_builder = this.module_builder.pe_builder;
        
        // 添加 .text 节（包含元数据和IL代码）
        const text_section = pe_builder.sections.get('.text');
        text_section.data = this.concat_arrays(metadata, text_section.data);
        
        // 设置数据目录
        pe_builder.data_directories.clr_runtime_header = {
            rva: this.calculate_cor20_header_rva(),
            size: cor20_header.length
        };
        
        // 7. 生成最终的 DLL
        return pe_builder.build();
    }
    
    calculate_cor20_header_rva() {
        // COR20 头通常位于 PE 头之后
        const pe_headers_size = 0x200; // 典型的PE头大小
        return pe_headers_size;
    }
    
    concat_arrays(a, b) {
        const result = new Uint8Array(a.length + b.length);
        result.set(a);
        result.set(b, a.length);
        return result;
    }
}
```

## 使用示例

```javascript
// 创建简单的 .NET DLL 示例
function create_simple_dll() {
    const dll_builder = new DotNetDllBuilder('MyAssembly');
    
    // 添加系统引用
    dll_builder.add_reference('mscorlib', '4.0.0.0');
    
    // 定义类
    const math_class = dll_builder.define_type('MathUtils', 'MyNamespace');
    math_class.set_flags(0x00000001); // public
    
    // 添加静态方法
    const add_method = math_class.add_method(
        'Add', 
        { kind: 'primitive', name: 'i4' }, // 返回类型: int
        [
            { name: 'a', type: { kind: 'primitive', name: 'i4' } },
            { name: 'b', type: { kind: 'primitive', name: 'i4' } }
        ],
        0x0016 // public | static
    );
    
    // 生成方法体
    const il_code = new ILInstructionEncoder();
    il_code.ldarg(0);    // 加载第一个参数
    il_code.ldarg(1);    // 加载第二个参数
    il_code.add();       // 相加
    il_code.ret();       // 返回结果
    
    add_method.body = il_code;
    
    // 构建DLL
    const dll_bytes = dll_builder.build();
    
    return dll_bytes;
}
```

这个实现计划提供了从基础 .NET 模块结构到完整 DLL 生成的完整路径，重用了我们现有的 PE 文件生成专业知识，同时添加了 .NET 特定的元数据和 IL 代码生成功能。