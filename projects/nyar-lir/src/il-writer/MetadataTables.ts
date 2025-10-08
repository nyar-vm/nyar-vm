import { BinaryWriter } from '../BinaryWriter';

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
