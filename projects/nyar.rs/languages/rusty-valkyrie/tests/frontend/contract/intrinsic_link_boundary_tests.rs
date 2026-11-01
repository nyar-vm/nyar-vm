//! 链接边界按冻结 intrinsic 声明身份编码数组结构操作。
//!
//! 前端 `lower_module_semantic` 仍按冻结合同把源码 `[intrinsic(...)]` 自由函数调用
//! 降级为普通 Call（见 `array_length_intrinsic_declaration_tests.rs`）；链接阶段才把
//! 这些没有函数体、也没有外部导入合同的实例改写为结构 opcode，因此绑定后的调用图里
//! 不存在未定义合同调用。

use nyar_types::{CanonicalOperation, ItemInstanceId};
use rusty_valkyrie::compiler::hir::lowering::{CompilerSourceGroup, ValkyrieCompiler};

const TEST_ARCH: &str = "native";

const CORE_SOURCE: &str = "namespace core::primitive;\n\
    [primitive(\"core::primitive::usize\")] structure usize { }\n\
    [primitive(\"core::primitive::i32\")] structure i32 { }";

const STD_SOURCE: &str = "namespace demo.std;\n\
    [intrinsic(\"builtin.array.length\")] private micro __len<T>(self: [T]): usize { }\n\
    [intrinsic(\"builtin.array.get\")] private micro __get<T>(self: [T], ordinal: usize): T { }\n\
    [intrinsic(\"builtin.array.set\")] private micro __set<T>(mut self: [T], ordinal: usize, value: T): unit { }\n\
    micro rewrite(values: [i32]) -> i32 {\n\
        \x20\x20let ignored: usize = __len(values)\n\
        \x20\x20__set(values, 0, __get(values, 0))\n\
        \x20\x20return __get(values, 1)\n\
    }";

const APP_SOURCE: &str = "namespace demo;\n\
    using demo.std.{rewrite};\n\
    [main] micro main() -> i32 {\n\
        \x20\x20let values: [i32] = []\n\
        \x20\x20return rewrite(values)\n\
    }";

fn groups() -> [CompilerSourceGroup; 3] {
    [
        CompilerSourceGroup::single_source("core", "core", CORE_SOURCE),
        CompilerSourceGroup::single_source("std", "demo.std", STD_SOURCE).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP_SOURCE).as_entry().with_direct_dependencies(vec!["core".into(), "std".into()]),
    ]
}

fn canonical_operations(program: &nyar_types::CanonicalProgram) -> Vec<(ItemInstanceId, CanonicalOperation)> {
    program
        .mir
        .functions
        .iter()
        .flat_map(|(instance, function)| {
            function
                .blocks
                .values()
                .flat_map(move |block| block.instructions.iter().map(move |instruction| (*instance, instruction.operation.clone())))
        })
        .collect()
}

#[test]
fn declared_array_intrinsics_are_encoded_as_structural_ops_at_link_boundary() {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups(), &[], TEST_ARCH)
        .expect("绑定后的调用图里不得残留未定义的 intrinsic 合同调用");
    let operations = canonical_operations(program.canonical());
    assert!(
        operations.iter().any(|(_, operation)| matches!(operation, CanonicalOperation::ArrayLength { .. })),
        "declared `builtin.array.length` call must become ArrayLength"
    );
    assert!(
        operations.iter().any(|(_, operation)| matches!(operation, CanonicalOperation::ArrayGet { .. })),
        "declared `builtin.array.get` call must become ArrayGet"
    );
    assert!(
        operations.iter().any(|(_, operation)| matches!(operation, CanonicalOperation::ArraySet { .. })),
        "declared `builtin.array.set` call must become ArraySet"
    );
}
