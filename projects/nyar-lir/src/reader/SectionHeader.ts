export interface SectionHeader {
    name: string;                // 节名称
    virtual_size: bigint;       // 虚拟大小
    virtual_address: bigint;    // 虚拟地址
    size_of_raw_data: bigint;   // 原始数据大小
    pointer_to_raw_data: bigint; // 原始数据指针
    pointer_to_relocations: bigint; // 重定位指针
    pointer_to_line_numbers: bigint; // 行号指针
    number_of_relocations: bigint; // 重定位数量
    number_of_line_numbers: bigint; // 行号数量
    characteristics: bigint;    // 特征值
}