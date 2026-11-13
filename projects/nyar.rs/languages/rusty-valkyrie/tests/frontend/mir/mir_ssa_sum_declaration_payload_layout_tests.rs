//! 声明级 sum 布局与具体实例共用同一条 payload 结算链路。
//!
//! `utf8` 在 std 里是 `type utf8 = Utf8Text` 别名，`Utf8Text` 是文本载体声明。非泛型
//! `unite` 的单字段名义 payload 曾经只能在实例级按已冻结声明拼写结算，声明级仍走严格
//! `concretize_type` 并以 `CAN054` 拒绝（`at DecodedPart::Plain`）。回归要求两侧同一
//! 结果：平台载体是 `NyarType::Utf8`，语义身份仍指向文本载体，不留 Named 短名残差。

use nyar::NyarType;
use nyar_types::CanonicalPrimitiveType;
use rusty_valkyrie::{
    MirLowerer, ValkyrieCompiler,
    compiler::{emitter::pipeline::canonical_program_from_semantic_mir, mir::ssa::MirModule},
};

/// decode-string 触发形状：单字段 `Plain { text: utf8 }` 与多字段
/// `Repeated { text: utf8, times: i64 }` 同在一个非泛型 `unite` 上。
const DECODE_STRING_SHAPE: &str = r#"
namespace std.text;

class Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text;

unite DecodedPart {
    [constructor]
    Plain { text: utf8 }
    [constructor]
    Repeated { text: utf8, times: i64 }
}

micro render(part: DecodedPart) -> utf8 {
    match part {
        case Plain(text):
            return text
        case Repeated(text, times):
            return text
    }
}
"#;

use crate::primitives::SourceFixtureCompile;

fn compile() -> MirModule {
    let hir = ValkyrieCompiler::default().compile_fixture_source(DECODE_STRING_SHAPE).expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

/// 反例（本次修复的观测点）：无 `[primitive]` 身份、与内建数值同名的声明不得借
/// 短名变成整数载体。
///
/// 修复前 `sum_payload_layout` 先调用 `from_surface_primitive(spelling)`，把
/// `acme::u32` 按末段 `u32` 结算成 `Integer32 { signed: true }`；修复后只认声明
/// 身份与已登记布局，同名无身份声明保留自己的名义布局。
#[test]
fn identity_less_numeric_payload_name_does_not_become_integer_carrier() {
    const NUMERIC_NAME_SHAPE: &str = r#"
namespace acme;

structure u32 {
}

unite Wrap {
    [constructor]
    Value { inner: u32 }
}
"#;
    let hir = ValkyrieCompiler::default().compile_fixture_source(NUMERIC_NAME_SHAPE).expect("fixture 必须完成 HIR 分析");
    let module = MirLowerer::lower_module_semantic(&hir);
    let program = canonical_program_from_semantic_mir(&module).expect("声明级 sum payload 必须完成实例化");
    let layout = program.linked.sum_types.iter().find(|sum| sum.name == "Wrap").expect("Wrap 声明级布局");
    let value = layout.variants.iter().find(|variant| variant.name == "Value").expect("Value variant");
    assert!(
        !matches!(value.payload_type, Some(NyarType::Integer32 { .. })),
        "无 primitive 身份的 `u32` 不得按短名变成整数载体: {:?}",
        value.payload_type
    );
}

/// 反例（本次修复的观测点）：泛型 impl 方法返回裸 owner 名时不得在 MIR 补造实参。
///
/// 修复前 `impl_method_return_type` 对二参数 owner 会把方法末位泛型放到返回容器
/// 首位、impl 第二个实参放到第二位；`-> Pair` 因此可能被补成 `Pair<U, E>`。
/// 现有前端已在返回值合同处拒绝缺参的裸 owner 名，MIR 不得再获得按形状补造的机会。
#[test]
fn generic_impl_method_with_bare_owner_return_is_not_patched_in_mir() {
    const BARE_OWNER_RETURN: &str = r#"
namespace demo;

unite Pair<T, E> {
    [constructor]
    Both { left: T, right: E }
}

imply Pair<i32, i32> {
    micro broken(self) -> Pair {
        return self
    }
}

[export]
micro probe(pair: Pair<i32, i32>) -> Pair<i32, i32> {
    return pair.broken()
}
"#;
    let error = ValkyrieCompiler::default()
        .compile_fixture_source(BARE_OWNER_RETURN)
        .expect_err("裸 owner 返回类型缺少泛型实参，必须在返回合同处结构化拒绝");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains("ReturnValueTypeMismatch") || rendered.contains("UnresolvedSumInstanceFacts"),
        "拒绝必须来自完整签名合同，而不是 MIR 按形状补造: {rendered}"
    );
}

/// 回归：声明级布局必须结算单字段名义 payload，不得因严格 concretize 失败关闭。
#[test]
fn declaration_level_sum_layout_resolves_nominal_text_payload() {
    let module = compile();
    let program = canonical_program_from_semantic_mir(&module).expect("声明级 sum payload 必须完成实例化");
    let layout = program.linked.sum_types.iter().find(|sum| sum.name == "DecodedPart").expect("DecodedPart 声明级布局");
    let plain = layout.variants.iter().find(|variant| variant.name == "Plain").expect("Plain variant");
    assert_eq!(plain.payload_type, Some(NyarType::Utf8), "`utf8` payload 必须结算为平台 Utf8 载体，不得退化成 Named 短名");
    let repeated = layout.variants.iter().find(|variant| variant.name == "Repeated").expect("Repeated variant");
    assert!(repeated.payload_type.is_some(), "多字段 payload 必须由已登记变体记录布局给出");
}

/// 回归：布局结算不改写语义身份——单字段 payload 仍指向文本载体声明。
#[test]
fn nominal_text_payload_keeps_carrier_identity() {
    let module = compile();
    let program = canonical_program_from_semantic_mir(&module).expect("声明级 sum payload 必须完成实例化");
    let layout = program.linked.sum_types.iter().find(|sum| sum.name == "DecodedPart").expect("DecodedPart 声明级布局");
    let plain = layout.variants.iter().find(|variant| variant.name == "Plain").expect("Plain variant");
    let record = program.linked.variants.get(&(layout.nominal, plain.id)).expect("Plain payload 合同");
    let payload = record.payload_type.expect("Plain 必须携带 payload 类型");
    assert_eq!(
        program.linked.text_encoding_types.get(&payload),
        Some(&CanonicalPrimitiveType::Utf8),
        "payload 的语义身份必须仍是文本载体编码，不得被布局拼写改写"
    );
}
