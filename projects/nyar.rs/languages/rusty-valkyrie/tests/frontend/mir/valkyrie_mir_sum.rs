use rusty_valkyrie::{
    ValkyrieCompiler,
    mir::{MirLowerer, MirOperation, validate_semantic_module},
    types::hir::types::ValkyrieType,
};

#[test]
fn sum_declaration_preserves_generic_fields_and_instantiates_without_variant_spelling() {
    let hir = ValkyrieCompiler::default()
        .compile_source(
            r#"
unite Parcel<First, Second> {
    Arbitrary { head: Second, tail: [First] },
    Empty,
}
"#,
        )
        .expect("当前源码声明");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let sum = &mir.sum_types[0];
    let variant = &sum.variants[0];
    assert_eq!(sum.generics, hir.enums[0].generics);
    assert_eq!(variant.fields[0].name, "head");
    assert_eq!(variant.fields[1].name, "tail");
    let record = variant.declaration.expect("Arbitrary declaration");
    assert_eq!(
        sum.instantiate_payload(variant, &[ValkyrieType::Boolean, ValkyrieType::Utf8]),
        Some(Some(ValkyrieType::Nominal {
            declaration: record,
            arguments: vec![ValkyrieType::Boolean, ValkyrieType::Utf8],
            primitive_identity: None,
        }))
    );
    assert!(sum.instantiate_payload(variant, &[ValkyrieType::Boolean]).is_none());
    validate_semantic_module(&mir).expect("声明合同");
}

#[test]
fn sum_declaration_rejects_duplicate_owner_binder_and_field() {
    let hir = ValkyrieCompiler::default().compile_source("unite Parcel<T> { Arbitrary { value: T } }").expect("当前源码声明");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let mut duplicate_owner = mir.clone();
    duplicate_owner.sum_types.push(mir.sum_types[0].clone());
    assert_eq!(validate_semantic_module(&duplicate_owner).unwrap_err().code, "SMIR006");
    let mut duplicate_binder = mir.clone();
    duplicate_binder.sum_types[0].generics.push(mir.sum_types[0].generics[0].clone());
    assert_eq!(validate_semantic_module(&duplicate_binder).unwrap_err().code, "SMIR006");
    let mut duplicate_field = mir.clone();
    duplicate_field.sum_types[0].variants[0].fields.push(mir.sum_types[0].variants[0].fields[0].clone());
    assert_eq!(validate_semantic_module(&duplicate_field).unwrap_err().code, "SMIR006");
}

#[test]
fn sum_declaration_preserves_variant_result_refinement() {
    let mut hir = ValkyrieCompiler::default().compile_source("unite Parcel<T> { Arbitrary { value: T } }").expect("当前源码声明");
    let item = hir.enums[0].declaration.expect("Parcel declaration");
    let refined = ValkyrieType::Nominal { declaration: item, arguments: vec![ValkyrieType::Boolean], primitive_identity: None };
    hir.enums[0].variants[0].result_type = Some(refined.clone());
    let mir = MirLowerer::lower_module_semantic(&hir);
    let sum = &mir.sum_types[0];
    assert_eq!(sum.variants[0].result_type, Some(refined.clone()));
    assert_eq!(sum.instantiate_result(&sum.variants[0], &[ValkyrieType::Utf8]), Some(refined));
}

#[test]
fn sum_operations_from_source_reject_missing_substitution_and_wrong_payload() {
    let mir = ValkyrieCompiler::default()
        .compile_source_to_mir(
            r#"
unite Envelope<T> { Present { value: T }, Absent }
micro wrap(value: utf8) -> Envelope<utf8> {
    return Present { value: value }
}
"#,
        )
        .expect("当前源码构造合同");
    validate_semantic_module(&mir).expect("正确实例化");
    let mut missing = mir.clone();
    let operation = missing
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match &mut instruction.kind {
            MirOperation::SumNew { type_args, .. } => Some(type_args),
            _ => None,
        })
        .expect("SumNew");
    operation.clear();
    assert_eq!(validate_semantic_module(&missing).unwrap_err().code, "SMIR006");
    let mut wrong = mir;
    let operation = wrong
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match &mut instruction.kind {
            MirOperation::SumNew { payload_type, .. } => Some(payload_type),
            _ => None,
        })
        .expect("SumNew");
    *operation = Some(ValkyrieType::Boolean);
    assert_eq!(validate_semantic_module(&wrong).unwrap_err().code, "SMIR006");
}

const MULTI_FIELD_RECORD_SOURCE: &str = r#"
unite Pair {
    [constructor]
    Both {
        left: bool,
        right: utf8,
    },
    Neither,
}
micro build(left: bool, right: utf8) -> Pair {
    return Both {
        left: left,
        right: right,
    }
}
micro describe(input: Pair) -> utf8 {
    match input {
        case Both(left, right):
            right
        case Neither:
            ""
    }
}
"#;

fn pair_record(
    module: &rusty_valkyrie::compiler::mir::MirModule,
) -> (&rusty_valkyrie::compiler::mir::MirSumDeclaration, &rusty_valkyrie::compiler::mir::MirSumVariant, ValkyrieType) {
    let mir = module;
    let sum = mir.sum_types.iter().find(|item| item.name == "Pair").expect("Pair");
    let both = sum.variants.iter().find(|variant| variant.name == "Both").expect("Both");
    let record = both.declaration.expect("Both declaration");
    let record_type = ValkyrieType::Nominal { declaration: record, arguments: Vec::new(), primitive_identity: None };
    (sum, both, record_type)
}

#[test]
fn multi_field_variant_payload_is_record_and_binds_non_first_field() {
    let module = ValkyrieCompiler::default().compile_source_to_mir(MULTI_FIELD_RECORD_SOURCE).expect("多字段变体 MIR");
    let (sum, both, record_type) = pair_record(&module);
    assert_eq!(both.fields.len(), 2);
    let neither = sum.variants.iter().find(|variant| variant.name == "Neither").expect("Neither");
    assert_eq!(sum.instantiate_payload(both, &[]), Some(Some(record_type.clone())), "多字段 payload 必须是名义变体记录");
    assert_eq!(sum.instantiate_payload(neither, &[]), Some(None), "无字段变体 payload 必须为空");

    let build = module.functions.iter().find(|function| function.display_symbol.contains("build")).expect("build");
    let (payload_type, payloads) = build
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match &instruction.kind {
            MirOperation::SumNew { payload_type, payloads, .. } => Some((payload_type.clone(), payloads.clone())),
            _ => None,
        })
        .expect("build 的 SumNew");
    assert_eq!(payload_type, Some(record_type.clone()), "payload 类型不得退化成首字段类型");
    assert_eq!(payloads.len(), 2, "SumNew 必须携带全部变体字段操作数");

    let describe = module.functions.iter().find(|function| function.display_symbol.contains("describe")).expect("describe");
    let field_gets: Vec<_> = describe
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::FieldGet { field, .. } => Some(*field),
            _ => None,
        })
        .collect();
    assert!(field_gets.contains(&both.fields[0].id), "首字段 left 必须按记录字段身份读取");
    assert!(field_gets.contains(&both.fields[1].id), "非首字段 right 必须真正按记录字段身份读取，不得用首字段伪装");
    assert_ne!(both.fields[0].id, both.fields[1].id, "同一变体的不同字段必须各有独立字段身份");

    validate_semantic_module(&module).expect("多字段 SumNew / FieldGet 合同");
    let output = ValkyrieCompiler::default().compile_source_to_program(MULTI_FIELD_RECORD_SOURCE).expect("记录 payload 须贯穿 Compiler");
    output.canonical().validate().expect("canonical 必须按已登记记录 nominal 合成 StructNew 并闭合 FieldGet 偏移");
}

#[test]
fn zero_field_and_single_field_variants_keep_payload_shapes_through_canonical() {
    let source = r#"
unite Flag {
    On,
    Off,
}
unite Wrapper {
    [constructor]
    Value { text: utf8 },
    Empty,
}
micro off() -> Flag { return Off }
micro wrap(text: utf8) -> Wrapper {
    return Value { text: text }
}
"#;
    let mir = ValkyrieCompiler::default().compile_source_to_mir(source).expect("零/单字段变体 MIR");
    validate_semantic_module(&mir).expect("零/单字段 payload 合同");
    let flag = mir.sum_types.iter().find(|item| item.name == "Flag").expect("Flag");
    let off = flag.variants.iter().find(|variant| variant.name == "Off").expect("Off");
    assert_eq!(flag.instantiate_payload(off, &[]), Some(None));
    let wrapper = mir.sum_types.iter().find(|item| item.name == "Wrapper").expect("Wrapper");
    let value = wrapper.variants.iter().find(|variant| variant.name == "Value").expect("Value");
    assert_eq!(wrapper.instantiate_payload(value, &[]), Some(Some(ValkyrieType::Utf8)));
    let output = ValkyrieCompiler::default().compile_source_to_program(source).expect("零/单字段变体须贯穿 Compiler");
    output.canonical().validate().expect("零字段 payload 不得被记录化，单字段 payload 不得被记录化");
}

#[test]
fn nested_record_payload_lowers_through_canonical() {
    let source = r#"
unite Inner {
    [constructor]
    Leaf { text: utf8, flag: bool },
    Nil,
}
unite Outer {
    [constructor]
    Nested { inner: Inner, code: bool },
    None_,
}
micro make(text: utf8) -> Outer {
    return Nested {
        inner: Leaf { text: text, flag: true },
        code: true,
    }
}
micro read(value: Outer) -> bool {
    match value {
        case Nested(inner, code):
            code
        case None_:
            false
    }
}
"#;
    let mir = ValkyrieCompiler::default().compile_source_to_mir(source).expect("嵌套 payload MIR");
    validate_semantic_module(&mir).expect("嵌套记录 payload 合同");
    let output = ValkyrieCompiler::default().compile_source_to_program(source).expect("嵌套 payload 须贯穿 Compiler");
    output.canonical().validate().expect("嵌套记录布局与 FieldGet 偏移必须闭合");
}

#[test]
fn multi_field_payload_contract_rejects_missing_field_and_wrong_field_type() {
    let mir = ValkyrieCompiler::default().compile_source_to_mir(MULTI_FIELD_RECORD_SOURCE).expect("多字段变体 MIR");
    let right_field = pair_record(&mir).1.fields[1].id;

    // 字段类型不匹配：把非首字段的 SSA 类型改成 bool（声明是 utf8）。
    let mut wrong_type = mir.clone();
    let build = wrong_type.functions.iter_mut().find(|function| function.display_symbol.contains("build")).expect("build");
    for block in &mut build.blocks {
        for instruction in &mut block.instructions {
            if let MirOperation::SumNew { payloads, .. } = &instruction.kind {
                let target = payloads.get(1).expect("非首字段操作数").clone();
                if let rusty_valkyrie::compiler::mir::MirOperand::Value(value) = target {
                    build.value_types.insert(value, ValkyrieType::Boolean);
                }
            }
        }
    }
    assert_eq!(validate_semantic_module(&wrong_type).unwrap_err().code, "SMIR006", "非首字段类型不匹配必须失败闭合");

    // 字段缺失：SumNew 少一个字段操作数，合同不得只校验首字段。
    let mut missing_field = mir.clone();
    let build = missing_field.functions.iter_mut().find(|function| function.display_symbol.contains("build")).expect("build");
    for block in &mut build.blocks {
        for instruction in &mut block.instructions {
            if let MirOperation::SumNew { payloads, .. } = &mut instruction.kind {
                payloads.pop();
            }
        }
    }
    assert_eq!(validate_semantic_module(&missing_field).unwrap_err().code, "SMIR006", "缺失字段必须失败闭合");

    // 不存在的字段身份：FieldGet 指向变体记录声明之外的 FieldId，必须结构化失败。
    let mut unknown_field = mir;
    let describe = unknown_field.functions.iter_mut().find(|function| function.display_symbol.contains("describe")).expect("describe");
    let mut replaced = false;
    for block in &mut describe.blocks {
        for instruction in &mut block.instructions {
            if let MirOperation::FieldGet { field, .. } = &mut instruction.kind {
                if *field == right_field {
                    *field = nyar_types::FieldId::from_index(9999).expect("测试 FieldId");
                    replaced = true;
                }
            }
        }
    }
    assert!(replaced, "必须定位到非首字段的 FieldGet");
    assert_eq!(validate_semantic_module(&unknown_field).unwrap_err().code, "SMIR006", "不存在字段名不得回落到成员/自由函数查找");
}
