declare_op!(
    ///
    UnreachableOp,
    "unreachable",
    "wasm"
);

declare_op!(
    ///
    NopOp,
    "nop",
    "wasm"
);

declare_op!(
    ///
    EndOp,
    "end",
    "wasm"
);

declare_op!(
    ///
    BlockOp,
    "block",
    "wasm"
);

declare_op!(
    ///
    LoopOp,
    "loop",
    "wasm"
);
declare_op!(
    ///
    IfOp,
    "if",
    "wasm"
);

declare_op!(
    ///
    BreakOp,
    "br",
    "wasm"
);

declare_op!(
    ///
    BreakIfOp,
    "br_if",
    "wasm"
);


declare_op!(
    ///
    BreakTableOp,
    "br_table",
    "wasm"
);

declare_op!(
    ///
    ReturnOp,
    "return",
    "wasm"
);

declare_op!(
    ///
    CallOp,
    "call",
    "wasm"
);


declare_op!(
    ///
    CallIndirectOp,
    "call_indirect",
    "wasm"
);



declare_op!(
    ///
    DropOp,
    "drop",
    "wasm"
);


declare_op!(
    ///
    SelectOp,
    "select",
    "wasm"
);

declare_op!(
    ///
    LocalGet,
    "local.get",
    "wasm"
);

declare_op!(
    ///
    LocalSet,
    "local.set",
    "wasm"
);

declare_op!(
    ///
    LocalTee,
    "local.tee",
    "wasm"
);

declare_op!(
    ///
    GlobalGet,
    "global.get",
    "wasm"
);


declare_op!(
    ///
    GlobalSet,
    "global.set",
    "wasm"
);

declare_op!(
    ///
    TableGet,
    "table.get",
    "wasm"
);

declare_op!(
    ///
    TableSet,
    "table.set",
    "wasm"
);
declare_op!(
    ///
    TableInit,
    "table.init",
    "wasm"
);
declare_op!(
    ///
    TableCopy,
    "table.copy",
    "wasm"
);
declare_op!(
    ///
    TableGrow,
    "table.grow",
    "wasm"
);

declare_op!(
    ///
    TableSize,
    "table.size",
    "wasm"
);


declare_op!(
    ///
    TableFill,
    "table.fill",
    "wasm"
);

declare_op!(
    ///
    ElementDrop,
    "elem.drop",
    "wasm"
);

declare_op!(
    ///
    I32Constant,
    "i32.const",
    "wasm"
);

declare_op!(
    ///
    I32Load,
    "i32.load",
    "wasm"
);
declare_op!(
    ///
    I32LoadAsI8,
    "i32.load8_s",
    "wasm"
);
declare_op!(
    ///
    I32Store,
    "i32.store",
    "wasm"
);

declare_op!(
    ///
    I32Store8,
    "i32.store8",
    "wasm"
);

declare_op!(
    ///
    MemorySize,
    "memory.size",
    "wasm"
);


declare_op!(
    ///
    MemoryGrow,
    "memory.size",
    "wasm"
);


declare_op!(
    ///
    MemoryInit,
    "memory.size",
    "wasm"
);
declare_op!(
    ///
    MemoryCopy,
    "memory.size",
    "wasm"
);
declare_op!(
    ///
    MemoryFill,
    "memory.fill",
    "wasm"
);
declare_op!(
    ///
    DataDrop,
    "data.drop",
    "wasm"
);

declare_op!(
    ///
    I32Eqz,
    "i32.eqz",
    "wasm"
);
declare_op!(
    ///
    I32Clz,
    "i32.clz",
    "wasm"
);
declare_op!(
    ///
    I32WrapI64,
    "i32.wrap_i64",
    "wasm"
);
