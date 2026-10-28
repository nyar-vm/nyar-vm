//! Compiler 在依赖闭包完成后生产唯一的已验证成功载荷。

use nyar_types::{CompileStage, CompiledProgram, StageResult, pipeline::RepresentationPlanStage};

use super::{CanonicalRepresentationPlanner, canonical_program_from_semantic_mir, fail_stage};
use crate::compiler::mir::{MirModule, validation::validate_semantic_module};

/// 校验 Semantic MIR 并生产唯一的已验证成功载荷；集成测试夹具。
#[doc(hidden)]
pub fn compile_linked_semantic_mir(module: &MirModule) -> StageResult<CompiledProgram> {
    if let Err(error) = validate_semantic_module(module) {
        return fail_stage(CompileStage::ValidateMir, "PIPE004", &module.name, format!("Semantic MIR 合同失败: {error:#?}"));
    }
    let canonical = canonical_program_from_semantic_mir(module)?;
    let representation = CanonicalRepresentationPlanner.plan(&canonical)?;
    match CompiledProgram::new(canonical, representation) {
        Ok(program) => Ok(program),
        Err(error) => fail_stage(
            CompileStage::RepresentationPlan,
            "PIPE005",
            &module.name,
            format!("CanonicalProgram 与 RepresentationPlan 合同不一致: {error:#?}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{hir::lowering::ValkyrieCompiler, mir::ssa::MirLowerer};

    /// 复现 ArrayList<i64> 泛型实例化 PIPE004 错误码。
    #[test]
    fn arraylist_generic_instantiation_shows_smir_error_code() {
        let source = r#"
namespace test.arraylist;

class ArrayList<T> {
    _items: [T]
    _capacity: usize
}

imply ArrayList<T> {
    micro new(): Self {
        return ArrayList { _items: [], _capacity: 0 }
    }
}

micro add(nums: ArrayList<i64>, target: i64): ArrayList<i64> {
    return ArrayList::new()
}
"#;
        let hir = ValkyrieCompiler::default().compile_source(source).expect("源码必须通过 HIR 分析");
        // HIR 诊断
        for func in &hir.functions {
            eprintln!("  HIR func: {} ret={:?} generics={:?}", func.name, func.return_type, func.generics);
        }
        for imp in &hir.impls {
            eprintln!("  HIR impl: target={:?} generics={:?} methods={}", imp.target, imp.generics, imp.methods.len());
            for m in &imp.methods {
                eprintln!("    method: {} ret={:?} generics={:?}", m.name, m.return_type, m.generics);
            }
        }
        for s in &hir.structs {
            eprintln!("  HIR struct/class: {} generics={:?} methods={}", s.name, s.generics, s.methods.len());
            for m in &s.methods {
                eprintln!("    struct method: {} ret={:?}", m.name, m.return_type);
            }
        }
        let mir = MirLowerer::lower_module_semantic(&hir);
        eprintln!(
            "=== MirModule name={} functions={} structs={} sum_types={} ===",
            mir.name,
            mir.functions.len(),
            mir.structs.len(),
            mir.sum_types.len()
        );
        for decl in &mir.structs {
            eprintln!(
                "  struct: {} decl={:?} nom={:?} generics={:?} fields={:?}",
                decl.name,
                decl.declaration,
                decl.nominal,
                decl.generics,
                decl.fields.iter().map(|f| format!("{:?}:{:?}:{:?}", f.name, f.id, f.ty)).collect::<Vec<_>>()
            );
        }
        for func in &mir.functions {
            eprintln!("  func: {} params={:?} ret={:?}", func.display_symbol, func.param_types, func.return_type);
            for (vref, vtype) in &func.value_types {
                eprintln!("    vref={:?} vtype={:?}", vref, vtype);
            }
            for block in &func.blocks {
                for inst in &block.instructions {
                    eprintln!("    block {:?} inst results={:?} kind={:?}", block.id, inst.results, inst.kind);
                }
                eprintln!("    block {:?} term={:?}", block.id, block.terminator);
            }
        }
        // 单独跑 SMIR 合同校验拿具体错误码
        match validate_semantic_module(&mir) {
            Ok(_) => eprintln!("=== SMIR validate OK ==="),
            Err(err) => eprintln!("=== SMIR ERROR code={} func={} loc={} detail={} ===", err.code, err.function, err.location, err.detail),
        }
        // 完整 pipeline
        match compile_linked_semantic_mir(&mir) {
            Ok(_) => eprintln!("=== FULL PIPELINE OK ==="),
            Err(e) => eprintln!("=== PIPE004/PIPE005 FAILED: {e:#?} ==="),
        }
    }

    /// 简单 primitive add：临时调试 BPHYS004。源码不是有效 Valkyrie（structure i32 不是合法语法），
    /// 等前端层正确合成 primitive imply 块后再实现。
    #[test]
    #[ignore]
    fn simple_i32_add_passes_smir_contract() {
        let source = r#"
[primitive("core::primitive::i32")]
structure i32 { }

micro add(a: i32, b: i32): i32 {
    return a + b
}
"#;
        let hir = ValkyrieCompiler::default().compile_source(source).expect("i32 add 源码必须通过 HIR 分析");
        let mir = MirLowerer::lower_module_semantic(&hir);
        eprintln!("=== simple i32 add: functions={} ===", mir.functions.len());
        for func in &mir.functions {
            eprintln!("  func: {}", func.display_symbol);
        }
        match validate_semantic_module(&mir) {
            Ok(_) => eprintln!("=== simple i32 add: SMIR OK ==="),
            Err(err) => eprintln!("=== simple i32 add SMIR ERROR code={} func={} detail={} ===", err.code, err.function, err.detail),
        }
    }
}
