export interface FileHeader {
    machine: bigint;              // 机器类型
    number_of_sections: bigint;   // 节数量
    time_date_stamp: bigint;       // 时间日期戳
    pointer_to_symbol_table: bigint; // 符号表指针
    number_of_symbols: bigint;     // 符号数量
    size_of_optional_header: bigint; // 可选头大小
    characteristics: bigint;       // 特征值
}