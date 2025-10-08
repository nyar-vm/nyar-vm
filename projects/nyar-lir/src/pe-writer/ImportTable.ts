import { BinaryWriter } from '../BinaryWriter';

export class ImportFunction {
    name: string | null;
    ordinal: number | null;
    hint: number;
    rva: number; // RVA to the function name/ordinal

    constructor(name: string | null, ordinal: number | null, hint: number) {
        this.name = name;
        this.ordinal = ordinal;
        this.hint = hint;
        this.rva = 0;
    }
}

export class ImportLibrary {
    name: string;
    functions: Map<string, ImportFunction>;
    original_first_thunk_rva: number; // RVA to OriginalFirstThunk (ILT)
    first_thunk_rva: number; // RVA to FirstThunk (IAT)
    name_rva: number; // RVA to DLL name
    time_date_stamp: number;
    forwarder_chain: number;

    constructor(name: string) {
        this.name = name;
        this.functions = new Map();
        this.original_first_thunk_rva = 0;
        this.first_thunk_rva = 0;
        this.name_rva = 0;
        this.time_date_stamp = 0;
        this.forwarder_chain = 0;
    }

    add_function(name: string | null, ordinal: number | null, hint: number): ImportFunction {
        const func = new ImportFunction(name, ordinal, hint);
        if (name) {
            this.functions.set(name, func);
        } else if (ordinal !== null) {
            this.functions.set(`_ordinal_${ordinal}`, func);
        }
        return func;
    }
}

export class ImportTable {
    libraries: Map<string, ImportLibrary>;
    private current_rva: number;

    constructor() {
        this.libraries = new Map();
        this.current_rva = 0; // This will be updated during layout
    }

    add_library(name: string): ImportLibrary {
        let library = this.libraries.get(name);
        if (!library) {
            library = new ImportLibrary(name);
            this.libraries.set(name, library);
        }
        return library;
    }

    add_import(library_name: string, function_name: string | null, ordinal: number | null = null, hint: number = 0): ImportFunction {
        const library = this.add_library(library_name);
        return library.add_function(function_name, ordinal, hint);
    }

    // 计算导入表所需的总大小，并设置RVA
    layout(base_rva: number): number {
        this.current_rva = base_rva;

        // 1. 计算并设置 DLL 名称的 RVA
        for (const library of this.libraries.values()) {
            library.name_rva = this.current_rva;
            this.current_rva += library.name.length + 1; // +1 for null terminator
        }

        // 2. 计算并设置 Hint/Name Table 的 RVA
        for (const library of this.libraries.values()) {
            for (const func of library.functions.values()) {
                func.rva = this.current_rva;
                if (func.name) {
                    this.current_rva += 2 + func.name.length + 1; // Hint + Name + null terminator
                } else if (func.ordinal !== null) {
                    this.current_rva += 2; // Only Hint for ordinal imports
                }
                // 确保对齐
                if (this.current_rva % 2 !== 0) {
                    this.current_rva++;
                }
            }
        }

        // 3. 计算并设置 OriginalFirstThunk (ILT) 和 FirstThunk (IAT) 的 RVA
        // ILT 和 IAT 是并行的数组，每个函数一个条目
        for (const library of this.libraries.values()) {
            library.original_first_thunk_rva = this.current_rva;
            this.current_rva += library.functions.size * (8); // x64 uses 8 bytes per THUNK
            library.first_thunk_rva = this.current_rva;
            this.current_rva += library.functions.size * (8); // x64 uses 8 bytes per THUNK
        }

        // 4. 计算并设置 Import Directory Table 的 RVA
        const import_directory_table_rva = this.current_rva;
        this.current_rva += (this.libraries.size + 1) * 20; // Each entry is 20 bytes, plus a null terminator entry

        return this.current_rva - base_rva; // 返回总大小
    }

    // 写入导入表数据
    write(writer: BinaryWriter, image_base: number): void {
        const import_directory_table_start_rva = this.current_rva - (this.libraries.size + 1) * 20;

        // 1. 写入 DLL 名称
        for (const library of this.libraries.values()) {
            writer.seek(library.name_rva - (import_directory_table_start_rva - (this.current_rva - (this.libraries.size + 1) * 20))); // Adjust seek position
            writer.write_string(library.name);
            writer.write_u8(0); // Null terminator
        }

        // 2. 写入 Hint/Name Table
        for (const library of this.libraries.values()) {
            for (const func of library.functions.values()) {
                writer.seek(func.rva - (import_directory_table_start_rva - (this.current_rva - (this.libraries.size + 1) * 20))); // Adjust seek position
                writer.write_u16(func.hint);
                if (func.name) {
                    writer.write_string(func.name);
                    writer.write_u8(0); // Null terminator
                }
                // 确保对齐
                if (writer.offset % 2 !== 0) {
                    writer.write_u8(0);
                }
            }
        }

        // 3. 写入 OriginalFirstThunk (ILT) 和 FirstThunk (IAT)
        for (const library of this.libraries.values()) {
            // ILT
            writer.seek(library.original_first_thunk_rva - (import_directory_table_start_rva - (this.current_rva - (this.libraries.size + 1) * 20))); // Adjust seek position
            for (const func of library.functions.values()) {
                if (func.ordinal !== null) {
                    writer.write_u64(0x8000000000000000 | func.ordinal); // Ordinal import
                } else {
                    writer.write_u64(func.rva); // RVA to Hint/Name Table
                }
            }
            // IAT (初始时与ILT相同)
            writer.seek(library.first_thunk_rva - (import_directory_table_start_rva - (this.current_rva - (this.libraries.size + 1) * 20))); // Adjust seek position
            for (const func of library.functions.values()) {
                if (func.ordinal !== null) {
                    writer.write_u64(0x8000000000000000 | func.ordinal); // Ordinal import
                } else {
                    writer.write_u64(func.rva); // RVA to Hint/Name Table
                }
            }
        }

        // 4. 写入 Import Directory Table
        writer.seek(import_directory_table_start_rva - (import_directory_table_start_rva - (this.current_rva - (this.libraries.size + 1) * 20))); // Adjust seek position
        for (const library of this.libraries.values()) {
            writer.write_u32(library.original_first_thunk_rva); // OriginalFirstThunk (ILT)
            writer.write_u32(library.time_date_stamp);
            writer.write_u32(library.forwarder_chain);
            writer.write_u32(library.name_rva);
            writer.write_u32(library.first_thunk_rva); // FirstThunk (IAT)
        }
        // Null terminator entry
        writer.write_u32(0);
        writer.write_u32(0);
        writer.write_u32(0);
        writer.write_u32(0);
        writer.write_u32(0);
    }

    get_iat_rva(library_name: string, function_name: string): number {
        const library = this.libraries.get(library_name);
        if (!library) {
            throw new Error(`Library ${library_name} not found in import table.`);
        }
        let offset = 0;
        for (const func of library.functions.values()) {
            if (func.name === function_name) {
                return library.first_thunk_rva + offset;
            }
            offset += 8; // x64 THUNK size
        }
        throw new Error(`Function ${function_name} not found in library ${library_name}.`);
    }
}