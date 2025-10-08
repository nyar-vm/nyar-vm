import { BinaryWriter } from '../BinaryWriter.js';
import { PeAssembler } from '../pe/PeBuilder.js';

export class DotNetModuleBuilder {
    private module_name: string;
    private output_type: string;
    private metadata_tables: Map<string, unknown[]>;
    private heap_data: Map<string, unknown>; // #Strings, #US, #GUID, #Blob
    private assembly_info: unknown;
    private type_defs: unknown[];
    private method_defs: unknown[];
    private field_defs: unknown[];
    private custom_attributes: unknown[];
    private pe_builder: PeAssembler;

    constructor(module_name: string, output_type = 'dll') {
        this.module_name = module_name;
        this.output_type = output_type;

        // .NET 特定的数据结构
        this.metadata_tables = new Map();
        this.heap_data = new Map();
        this.assembly_info = null;
        this.type_defs = [];
        this.method_defs = [];
        this.field_defs = [];
        this.custom_attributes = [];

        // 重用现有的 PE 构建器
        this.pe_builder = new PeAssembler('x86');
        this.init_dotnet_sections();
    }

    private init_dotnet_sections(): void {
        // .NET 特定的节
        this.pe_builder.add_section('.text', 0x60000020); // 代码和元数据
        this.pe_builder.add_section('.rsrc', 0x40000040); // 资源
        this.pe_builder.add_section('.reloc', 0x42000040); // 重定位
    }

    generate_cor20_header(): Uint8Array {
        const writer = new BinaryWriter();

        // IMAGE_COR20_HEADER
        writer.write_u32(0x48); // cb
        writer.write_u16(0x0002); // MajorRuntimeVersion
        writer.write_u16(0x0005); // MinorRuntimeVersion
        writer.write_u32(this.get_metadata_rva()); // MetaData
        writer.write_u32(0x00000004); // Flags: ILOnly
        writer.write_u32(0x00000000); // EntryPointToken
        writer.write_u32(0x00000000); // Resources
        writer.write_u32(0x00000000); // StrongNameSignature
        writer.write_u32(0x00000000); // CodeManagerTable
        writer.write_u32(0x00000000); // VTableFixups
        writer.write_u32(0x00000000); // ExportAddressTableJumps
        writer.write_u32(0x00000000); // ManagedNativeHeader

        return writer.get_bytes();
    }

    generate_metadata_root(): Uint8Array {
        const writer = new BinaryWriter();

        // 存储签名
        writer.write_u32(0x424a5342); // BSJB

        // 主版本和次版本
        writer.write_u16(1);
        writer.write_u16(1);

        // 保留
        writer.write_u32(0);

        // 版本字符串长度
        const version_string = 'v4.0.30319\0';
        writer.write_u32(version_string.length);
        writer.write_cstring(version_string);

        // 标志和流数量
        writer.write_u16(0);
        writer.write_u16(5); // 5个流

        // 流头
        this.write_stream_header(writer, '#~', this.generate_tilde_stream());
        this.write_stream_header(writer, '#Strings', this.generate_strings_heap());
        this.write_stream_header(writer, '#US', this.generate_us_heap());
        this.write_stream_header(writer, '#GUID', this.generate_guid_heap());
        this.write_stream_header(writer, '#Blob', this.generate_blob_heap());

        return writer.get_bytes();
    }

    private write_stream_header(
        writer: BinaryWriter,
        name: string,
        data: { offset: number; size: number }
    ): void {
        writer.write_u32(this.align_to_4(data.offset)); // Offset
        writer.write_u32(data.size); // Size
        writer.write_cstring(name); // Name
        writer.align(4); // 对齐到4字节
    }

    private align_to_4(value: number): number {
        return Math.ceil(value / 4) * 4;
    }

    private get_metadata_rva(): number {
        // 返回元数据的RVA
        return 0x2000; // 默认值
    }

    private generate_tilde_stream(): { offset: number; size: number } {
        // 简化的实现
        return { offset: 0, size: 0 };
    }

    private generate_strings_heap(): { offset: number; size: number } {
        // 简化的实现
        return { offset: 0, size: 0 };
    }

    private generate_us_heap(): { offset: number; size: number } {
        // 简化的实现
        return { offset: 0, size: 0 };
    }

    private generate_guid_heap(): { offset: number; size: number } {
        // 简化的实现
        return { offset: 0, size: 0 };
    }

    private generate_blob_heap(): { offset: number; size: number } {
        // 简化的实现
        return { offset: 0, size: 0 };
    }

    build(): Uint8Array {
        // 生成 .NET 模块
        const cor20_header = this.generate_cor20_header();
        const metadata = this.generate_metadata_root();

        // 将 .NET 数据添加到 PE 构建器
        const text_section = this.pe_builder.add_section('.text', 0x60000020);
        const text_data = new BinaryWriter();
        text_data.write_bytes(cor20_header);
        text_data.write_bytes(metadata);
        text_section.set_raw_data(text_data.get_bytes());

        return this.pe_builder.build();
    }
}

export class MetadataTables {
    private tables: Map<string, unknown[]>;
    private valid_tables: Set<string>;

    constructor() {
        this.tables = new Map();
        this.valid_tables = new Set([
            'Module',
            'TypeRef',
            'TypeDef',
            'Field',
            'Method',
            'Param',
            'InterfaceImpl',
            'MemberRef',
            'Constant',
            'CustomAttribute',
            'Assembly',
            'AssemblyRef',
            'File',
            'ExportedType',
            'ManifestResource',
        ]);

        this.init_tables();
    }

    private init_tables(): void {
        for (const table_name of this.valid_tables) {
            this.tables.set(table_name, []);
        }
    }

    add_table_row(table_name: string, row_data: unknown): number {
        if (!this.valid_tables.has(table_name)) {
            throw new Error(`无效的元数据表: ${table_name}`);
        }

        const table = this.tables.get(table_name) as unknown[];
        const row_index = table.length;
        table.push(row_data);

        return (this.get_table_index(table_name) << 24) | row_index;
    }

    private get_table_index(table_name: string): number {
        const table_order = Array.from(this.valid_tables);
        return table_order.indexOf(table_name) + 1;
    }

    generate_tilde_stream(): { offset: number; size: number; data: Uint8Array } {
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
        writer.write_u64(0n);

        // 行计数
        for (const table_name of this.valid_tables) {
            if (this.is_table_valid(table_name)) {
                writer.write_u32((this.tables.get(table_name) as unknown[]).length);
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
            data: writer.get_bytes(),
        };
    }

    private calculate_valid_tables_mask(): bigint {
        let mask = 0n;
        let bit_position = 0n;

        for (const table_name of this.valid_tables) {
            if (this.is_table_valid(table_name)) {
                mask |= 1n << bit_position;
            }
            bit_position++;
        }

        return mask;
    }

    private is_table_valid(table_name: string): boolean {
        return (this.tables.get(table_name) as unknown[]).length > 0;
    }

    private write_table_data(writer: BinaryWriter, table_name: string): void {
        const table = this.tables.get(table_name) as unknown[];

        for (let i = 0; i < table.length; i++) {
            this.write_table_row(writer);
        }
    }

    private write_table_row(writer: BinaryWriter): void {
        // Simplified table row writing implementation
        writer.write_u32(0); // Placeholder
    }
}
