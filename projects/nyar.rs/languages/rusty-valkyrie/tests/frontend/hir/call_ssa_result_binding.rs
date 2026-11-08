use crate::primitives::SourceFixtureCompile;
use rusty_valkyrie::{MirLowerer, MirOperation, ValkyrieCompiler, compiler::types::SourceID};

/// primitive 运算符的声明事实来自语言 core；frontend 不再为 primitive 补造无声明实现，
/// 因此使用运算符的 fixture 必须像真实源码一样携带 `imply` 声明与宿主合同。
const I64_OPERATORS: &str = r#"
[primitive("std::primitive::i64")] structure i64 { }
imply i64 {
    infix `!=`(self, rhs: Self): bool { __i64_ne(self, rhs) }
    infix `%`(self, rhs: Self): Self { __i64_rem(self, rhs) }
    infix `/`(self, rhs: Self): Self { __i64_div(self, rhs) }
    infix `-`(self, rhs: Self): Self { __i64_sub(self, rhs) }
    infix `<`(self, rhs: Self): bool { __i64_lt(self, rhs) }
    infix `>`(self, rhs: Self): bool { __i64_gt(self, rhs) }
    infix `+`(self, rhs: Self): Self { __i64_add(self, rhs) }
    infix `*`(self, rhs: Self): Self { __i64_mul(self, rhs) }
}
[host_contract] private micro __i64_ne(lhs: i64, rhs: i64): bool
[host_contract] private micro __i64_rem(lhs: i64, rhs: i64): i64
[host_contract] private micro __i64_div(lhs: i64, rhs: i64): i64
[host_contract] private micro __i64_sub(lhs: i64, rhs: i64): i64
[host_contract] private micro __i64_lt(lhs: i64, rhs: i64): bool
[host_contract] private micro __i64_gt(lhs: i64, rhs: i64): bool
[host_contract] private micro __i64_add(lhs: i64, rhs: i64): i64
[host_contract] private micro __i64_mul(lhs: i64, rhs: i64): i64
"#;

#[test]
fn call_instructions_bind_one_ssa_result() {
    let source = r#"
namespace demo.overflow;

micro clamped_mul_add(base: i64, step: i64) -> i64 {
    let lo: i64 = -2147483648
    let hi: i64 = 2147483647
    let mut acc: i64 = base
    let mut n: i64 = step
    while n != 0 {
        let bound: i64 = if lo % 10 != 0 { lo / 10 - 1 } else { lo / 10 }
        if acc < bound + 1 || acc > hi / 10 {
            return 0
        }
        let digit: i64 = n % 10
        acc = acc * 10 + digit
        n = n / 10
    }
    return acc
}
"#;
    let hir = ValkyrieCompiler::new(SourceID { version_id: 1 })
        .compile_fixture_source(&format!("{I64_OPERATORS}{source}"))
        .expect("hir");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let func = mir.functions.iter().find(|f| f.display_symbol.contains("clamped_mul_add")).expect("clamped_mul_add");
    for block in &func.blocks {
        for insn in &block.instructions {
            if matches!(&insn.kind, MirOperation::Call { .. }) {
                assert_eq!(insn.results.len(), 1, "call must bind one SSA result: {:?}", insn.kind);
            }
        }
    }
}
