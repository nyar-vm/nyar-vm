//! JVM 最小 MIR → `class` lowering 的结构化验证。
//!
//! 这些用例直接构造已冻结的片段计划，验证冻结 operator 的机械展开、比较标签
//! 唯一性，以及未冻结绑定 / 未支持指令的结构化拒绝；不经过前端。

use std::collections::{BTreeMap, BTreeSet};

use nyar::{ExternalImportLink, Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    lowering::backends::jvm::JvmOperatorCode,
    nyar_backend_jvm::{JvmClassFile, JvmInstruction, JvmMethodDescriptor, JvmTypeDescriptor, decode_instructions},
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
    },
    testing::lower_fragment_to_jvm_class,
};
use nyar_types::{
    AggregateLayout, AggregateLayoutPlan, Block, BlockRef, ExecutableFunction, FieldLayout, ImportIndex, Instruction, InstructionKind,
    ItemInstanceId, NyarType, Operand, StorageKind, Terminator, ValueRef,
};

const LONG: NyarType = NyarType::Integer64 { signed: true };
const INTEGER: NyarType = NyarType::Integer32 { signed: true };
const BOOLEAN: NyarType = NyarType::Boolean;

fn function(symbol: &str, return_type: NyarType, parameters: Vec<NyarType>) -> ExecutableFunction {
    let value_types = parameters.iter().enumerate().map(|(index, ty)| (ValueRef(index as u32), ty.clone())).collect();
    let block_parameters = (0..parameters.len() as u32).map(ValueRef).collect();
    ExecutableFunction {
        return_layout: None,
        value_layouts: BTreeMap::new(),
        symbol: symbol.to_string(),
        return_type,
        param_types: parameters,
        value_types,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: block_parameters,
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
        diagnostics: Vec::new(),
    }
}

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

fn operator_import(plan: TargetOperatorPlan) -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("import index"),
        link: ExternalImportLink::host(Some(Identifier::new("jvm-operator")), vec!["long.divide".to_string()]),
        parameter_types: vec![LONG, LONG],
        return_type: LONG,
        target_operator: Some(TargetOperatorBinding::Frozen(plan)),
    }
}

fn operator_plan(code: JvmOperatorCode, operand_type: TargetOperandType, result_type: TargetOperandType) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Jvm(code),
    }
}

/// 构造只调用一个冻结 `long` operator 的导出函数，并返回降低后的 class。
fn lower_single_operator_call(code: JvmOperatorCode, result_type: TargetOperandType, return_type: NyarType) -> JvmClassFile {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let result_value_type = match result_type {
        TargetOperandType::I64 => LONG,
        TargetOperandType::I32 => INTEGER,
        TargetOperandType::F64 => NyarType::Float64,
    };
    let mut fixture = function("jvm.fixture", return_type, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), result_value_type);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(operator), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());
    plan.testing_register_import(operator, operator_import(operator_plan(code, TargetOperandType::I64, result_type)));
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_jvm_class(&submission).expect("frozen operator must lower")
}

fn method_code<'a>(class_file: &'a JvmClassFile, name: &str) -> &'a [JvmInstruction] {
    let method = class_file.methods.iter().find(|method| method.name == name).expect("lowered method");
    method.code.as_ref().expect("lowered method code").instructions.as_slice()
}

/// `long` 移位的冻结配方：发射实参后先 `l2i` 截窄位移量，再执行 `lshl`/`lshr`。
const LONG_SHIFT_STEPS: &[TargetOperandStep] = &[TargetOperandStep::Arguments, TargetOperandStep::NarrowTopI64ToI32];

fn shift_operator_plan(code: JvmOperatorCode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type: TargetOperandType::I64,
        result_type: TargetOperandType::I64,
        recipe: TargetOperatorRecipe::Steps(LONG_SHIFT_STEPS),
        encoding: TargetOperatorEncoding::Jvm(code),
    }
}

/// 构造只调用一个任意冻结配方 operator 的导出函数（结果载体 `I64`），返回降低后的 class。
fn lower_operator_call_with_plan(name: &str, operator: TargetOperatorPlan) -> JvmClassFile {
    let instance = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("jvm.fixture", LONG, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), LONG);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(instance), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), name.to_string());
    plan.testing_register_import(instance, operator_import(operator));
    lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect("frozen operator must lower")
}

#[test]
fn lowers_frozen_long_add_operator_and_roundtrips_class_bytes() {
    let class_file = lower_single_operator_call(JvmOperatorCode::LongAdd, TargetOperandType::I64, LONG);
    let method = class_file.methods.iter().find(|method| method.name == "add").expect("add method");
    assert_eq!(method.descriptor, JvmMethodDescriptor::new(vec![JvmTypeDescriptor::Long, JvmTypeDescriptor::Long], JvmTypeDescriptor::Long));
    assert!(method_code(&class_file, "add").iter().any(|instruction| matches!(instruction, JvmInstruction::LAdd)));

    let bytes = class_file.to_bytes().expect("class encoding");
    let decoded_class = JvmClassFile::from_bytes(&bytes).expect("class decoding");
    let index = decoded_class.methods.iter().position(|method| method.name == "add").expect("decoded add method");
    let code = decoded_class.raw_method_code[index].as_ref().expect("decoded method code");
    let decoded = decode_instructions(code, &decoded_class.constant_pool);
    let mnemonics = decoded.iter().map(|instruction| instruction.mnemonic.as_str()).collect::<Vec<_>>();
    assert!(mnemonics.contains(&"ladd"), "decoded={mnemonics:?}");
    assert_eq!(mnemonics.last().copied(), Some("lreturn"), "decoded={mnemonics:?}");
}

/// `long.shift_left` 的冻结配方要求位移量先 `l2i` 截窄再 `lshl`；backend 不得把
/// `i64` 栈顶直接喂给 `lshl`。截窄步骤必须随 class 字节 roundtrip 保留。
#[test]
fn lowers_frozen_long_shift_recipe_with_l2i_before_lshl() {
    let class_file = lower_operator_call_with_plan("shift", shift_operator_plan(JvmOperatorCode::LongShiftLeft));
    let instructions = method_code(&class_file, "shift");
    let narrow = instructions.iter().position(|instruction| matches!(instruction, JvmInstruction::L2I)).expect("配方必须发射 l2i 截窄位移量");
    let shift = instructions.iter().position(|instruction| matches!(instruction, JvmInstruction::LShl)).expect("配方必须执行 lshl");
    assert!(narrow < shift, "l2i 必须先于 lshl: {instructions:?}");

    let bytes = class_file.to_bytes().expect("shift class encoding");
    let decoded_class = JvmClassFile::from_bytes(&bytes).expect("shift class decoding");
    let index = decoded_class.methods.iter().position(|method| method.name == "shift").expect("decoded shift method");
    let code = decoded_class.raw_method_code[index].as_ref().expect("decoded method code");
    let decoded = decode_instructions(code, &decoded_class.constant_pool);
    let mnemonics = decoded.iter().map(|instruction| instruction.mnemonic.as_str()).collect::<Vec<_>>();
    let narrow_pos =
        mnemonics.iter().position(|mnemonic| *mnemonic == "l2i").unwrap_or_else(|| panic!("l2i 必须随 class 字节保留: {mnemonics:?}"));
    let shift_pos =
        mnemonics.iter().position(|mnemonic| *mnemonic == "lshl").unwrap_or_else(|| panic!("lshl 必须随 class 字节保留: {mnemonics:?}"));
    assert!(narrow_pos < shift_pos, "roundtrip 后 l2i 仍须先于 lshl: {mnemonics:?}");
}

/// `long.and` 走同载体二元配方：单条 `land`，不引入截窄或分支展开。
#[test]
fn lowers_frozen_long_bitand_operator_to_single_land() {
    let class_file = lower_single_operator_call(JvmOperatorCode::LongBitAnd, TargetOperandType::I64, LONG);
    let instructions = method_code(&class_file, "add");
    assert_eq!(instructions.iter().filter(|instruction| matches!(instruction, JvmInstruction::LAnd)).count(), 1, "{instructions:?}");
    assert!(instructions.iter().all(|instruction| !matches!(instruction, JvmInstruction::L2I)), "同载体位运算不得发射截窄: {instructions:?}");
    class_file.to_bytes().expect("land class encoding");
}

/// `integer` 位运算冻结码各落一条同载体指令；`u32` 右移必须走逻辑右移 `iushr`。
#[test]
fn integer_bitwise_codes_lower_to_single_instruction() {
    // 算术/位运算码不消费 scratch 槽，参数值无意义。
    assert!(matches!(JvmOperatorCode::IntegerBitAnd.instructions(0).as_slice(), [JvmInstruction::IAnd]));
    assert!(matches!(JvmOperatorCode::IntegerBitOr.instructions(0).as_slice(), [JvmInstruction::IOr]));
    assert!(matches!(JvmOperatorCode::IntegerBitXor.instructions(0).as_slice(), [JvmInstruction::IXor]));
    assert!(matches!(JvmOperatorCode::IntegerShiftLeft.instructions(0).as_slice(), [JvmInstruction::IShl]));
    assert!(matches!(JvmOperatorCode::IntegerShiftRightUnsigned.instructions(0).as_slice(), [JvmInstruction::IUShr]));
    assert!(JvmOperatorCode::IntegerBitAnd.arithmetic_instruction().is_some(), "位运算码不得落入比较展开路径");
}

#[test]
fn comparison_operator_expansion_uses_unique_method_local_labels() {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("jvm.compare", BOOLEAN, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), INTEGER);
    fixture.value_types.insert(ValueRef(3), INTEGER);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(operator), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(operator), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(3)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "less".to_string());
    plan.testing_register_import(
        operator,
        operator_import(operator_plan(JvmOperatorCode::LongCompareLessThan, TargetOperandType::I64, TargetOperandType::I32)),
    );
    let submission = FragmentSubmission::from_backend_plan(plan);
    let class_file = lower_fragment_to_jvm_class(&submission).expect("comparison operator must lower");

    let instructions = method_code(&class_file, "less");
    let labels = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            JvmInstruction::Label(label) => Some(label.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    // 块标签之外，两次比较必须各自展开成两个方法内唯一标签。
    let comparison_labels =
        labels.iter().filter(|label| label.ends_with("comparison_false") || label.ends_with("comparison_end")).cloned().collect::<Vec<_>>();
    assert_eq!(comparison_labels.len(), 4, "two comparisons must each expand into two labels: {labels:?}");
    let unique = labels.iter().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), labels.len(), "labels must be unique: {labels:?}");
    assert!(
        labels.iter().all(|label| label != "comparison_false" && label != "comparison_end"),
        "placeholder labels must be relabeled: {labels:?}"
    );

    let compare_count = instructions.iter().filter(|instruction| matches!(instruction, JvmInstruction::LCmp)).count();
    assert_eq!(compare_count, 2);
    class_file.to_bytes().expect("comparison class encoding");
}

/// JVM 没有无符号 `if_icmp`：`usize`/`u32` 小于必须展开为「两侧翻转符号位后
/// 按有符号比较」的显式序列，且不得偷换成有符号 `IntegerCompareLessThan`。
#[test]
fn unsigned_integer_less_than_flips_sign_bit_before_signed_branch() {
    let expansion = JvmOperatorCode::IntegerCompareLessThanUnsigned.instructions(0);
    let xor_count = expansion.iter().filter(|instruction| matches!(instruction, JvmInstruction::IXor)).count();
    let swap_count = expansion.iter().filter(|instruction| matches!(instruction, JvmInstruction::Swap)).count();
    assert_eq!(xor_count, 2, "两侧各翻转一次符号位: {expansion:?}");
    assert_eq!(swap_count, 2, "翻转符号位需要两次交换以保持操作数顺序: {expansion:?}");
    assert!(
        expansion.iter().any(|instruction| matches!(instruction, JvmInstruction::IConst(value) if *value == i32::MIN)),
        "翻转必须使用 `i32::MIN` 作为掩码: {expansion:?}"
    );
    assert!(
        expansion.iter().any(|instruction| matches!(instruction, JvmInstruction::IfICmpGe(_))),
        "翻转后按有符号 `>=` 分支到 false: {expansion:?}"
    );
}

/// JVM 没有无符号 `lcmp` 也没有 `swap2`：`u64` 小于必须展开为「两侧翻转符号位后
/// 按有符号 `lcmp` 比较」，翻转中途经 scratch `long` 槽周转，且槽必须随
/// `max_locals` 预留、随 class 字节 roundtrip 保留。
#[test]
fn unsigned_long_less_than_flips_sign_bits_through_scratch_local() {
    let scratch = 7u16;
    let expansion = JvmOperatorCode::LongCompareLessThanUnsigned.instructions(scratch);
    let xor_count = expansion.iter().filter(|instruction| matches!(instruction, JvmInstruction::LXor)).count();
    assert_eq!(xor_count, 2, "两侧各翻转一次符号位: {expansion:?}");
    assert_eq!(
        expansion.iter().filter(|instruction| matches!(instruction, JvmInstruction::LdcLong(value) if *value == i64::MIN)).count(),
        2,
        "翻转必须使用 `i64::MIN` 作为掩码: {expansion:?}"
    );
    assert!(
        expansion.iter().any(|instruction| matches!(instruction, JvmInstruction::LStore(slot) if *slot == scratch)),
        "中途结果必须存入 scratch 槽: {expansion:?}"
    );
    assert!(
        expansion.iter().any(|instruction| matches!(instruction, JvmInstruction::LLoad(slot) if *slot == scratch)),
        "翻转后必须从 scratch 槽取回: {expansion:?}"
    );
    assert!(
        expansion.iter().any(|instruction| matches!(instruction, JvmInstruction::LCmp))
            && expansion.iter().any(|instruction| matches!(instruction, JvmInstruction::IfGe(_))),
        "翻转后按 `lcmp` + 有符号 `>=` 分支到 false: {expansion:?}"
    );

    let class_file = lower_single_operator_call(JvmOperatorCode::LongCompareLessThanUnsigned, TargetOperandType::I32, BOOLEAN);
    let instructions = method_code(&class_file, "add");
    // scratch 槽是方法级预留的第 5 槽：2 个 `long` 形参占 0..4，结果 `int` 占 4。
    let lstore = instructions.iter().position(|instruction| matches!(instruction, JvmInstruction::LStore(5))).expect("必须发射 scratch lstore");
    let lload = instructions.iter().position(|instruction| matches!(instruction, JvmInstruction::LLoad(5))).expect("必须发射 scratch lload");
    let lcmp = instructions.iter().position(|instruction| matches!(instruction, JvmInstruction::LCmp)).expect("必须发射 lcmp");
    assert!(lstore < lload && lload < lcmp, "scratch 周转必须先存后取再比较: {instructions:?}");

    // 方法级 `max_locals` 必须覆盖 scratch 槽：2 个 `long` 形参（4 槽）+ 结果 `int`（1 槽）+ scratch（2 槽）。
    let method = class_file.methods.iter().find(|method| method.name == "add").expect("lowered method");
    assert_eq!(method.code.as_ref().expect("method code").max_locals, 7, "scratch long 槽必须计入 max_locals");

    let bytes = class_file.to_bytes().expect("class encoding");
    let decoded_class = JvmClassFile::from_bytes(&bytes).expect("class decoding");
    let index = decoded_class.methods.iter().position(|method| method.name == "add").expect("decoded method");
    let code = decoded_class.raw_method_code[index].as_ref().expect("decoded method code");
    let decoded = decode_instructions(code, &decoded_class.constant_pool);
    let mnemonics = decoded.iter().map(|instruction| instruction.mnemonic.as_str()).collect::<Vec<_>>();
    assert!(mnemonics.contains(&"ldc2_w"), "scratch 翻转常量必须随字节码保留: {mnemonics:?}");
    assert!(mnemonics.contains(&"lstore"), "scratch 槽必须随字节码保留: {mnemonics:?}");
    assert!(mnemonics.contains(&"lcmp"), "signed `lcmp` 必须随字节码保留: {mnemonics:?}");
}

#[test]
fn unfrozen_operator_binding_is_rejected_structurally() {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("jvm.fixture", LONG, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), LONG);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(operator), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());
    let mut import = operator_import(operator_plan(JvmOperatorCode::LongAdd, TargetOperandType::I64, TargetOperandType::I64));
    import.target_operator = Some(TargetOperatorBinding::Unfrozen);
    plan.testing_register_import(operator, import);

    let error = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect_err("unfrozen binding must fail");
    assert!(error.contains("缺少可编码助记符"), "{error}");
}

/// 构造两分支控制流，验证条件分支落到 `ifeq` + `goto`，两块各自返回。
#[test]
fn conditional_branch_lowers_to_compare_and_jump() {
    let mut fixture = function("jvm.branch", LONG, vec![LONG, BOOLEAN]);
    fixture.blocks.push(Block {
        id: BlockRef(1),
        label: "then_branch".to_string(),
        parameters: Vec::new(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    });
    fixture.blocks.push(Block {
        id: BlockRef(2),
        label: "else_branch".to_string(),
        parameters: Vec::new(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    });
    fixture.blocks[0].terminator =
        Terminator::Branch { condition: Operand::Value(ValueRef(1)), then_target: BlockRef(1), else_target: BlockRef(2) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "branch".to_string());
    let class_file = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect("conditional branch must lower");

    let instructions = method_code(&class_file, "branch");
    assert!(instructions.iter().any(|instruction| matches!(instruction, JvmInstruction::IfEq(_))), "{instructions:?}");
    assert!(instructions.iter().any(|instruction| matches!(instruction, JvmInstruction::Goto(_))), "{instructions:?}");
    class_file.to_bytes().expect("branch class encoding");
}

#[test]
fn unsupported_instruction_is_rejected_structurally() {
    let mut fixture = function("jvm.fixture", LONG, vec![LONG]);
    fixture.value_types.insert(ValueRef(1), LONG);
    fixture.blocks[0].instructions.push(instr(InstructionKind::Cast { source: Operand::Value(ValueRef(0)), ty: LONG }, vec![ValueRef(1)]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());

    let error = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect_err("Cast must fail");
    assert!(error.contains("尚不支持指令 `Cast`"), "{error}");
}

/// `Option<char>` 的实例级布局：discriminator + payload 槽，引用存储。
/// 布局身份（`LayoutId` 0）由计划注册表持有，类型只携带身份不携带拼写匹配。
fn option_char_layout() -> AggregateLayout {
    AggregateLayout {
        id: 0,
        name: "Option".to_string(),
        namespace: "core".to_string(),
        key: None,
        storage: StorageKind::Reference,
        size: 8,
        align: 4,
        fields: vec![
            FieldLayout { name: "discriminant".to_string(), ty: NyarType::Integer32 { signed: false }, offset: 0, size: 4, align: 4 },
            FieldLayout { name: "payload".to_string(), ty: NyarType::Character, offset: 4, size: 4, align: 4 },
        ],
    }
}

/// 泛型实例 `Option<char>`：基身份携带已冻结布局，实参代入 `char`。
fn option_char_type() -> NyarType {
    NyarType::Apply(Box::new(NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") }), vec![NyarType::Character])
}

/// 构造签名为 `(Option<char>, char) -> Option<char>` 的入口操作，返回第一个形参。
fn option_flow_plan(parameters: Vec<NyarType>, return_type: NyarType) -> FragmentSubmission {
    let mut fixture = function("jvm.option_flow", return_type, parameters);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![option_char_layout()], ..AggregateLayoutPlan::default() });
    FragmentSubmission::from_backend_plan(plan)
}

/// 泛型实例 `Option<char>` 的 JVM 载体合同：引用存储形状擦除为对象引用，
/// 形参 / 局部 / 返回槽按引用载体搬运，class 字节可编码并 roundtrip。
#[test]
fn option_char_flow_lowers_with_erased_reference_carrier() {
    let option = option_char_type();
    let submission = option_flow_plan(vec![option.clone(), NyarType::Character], option.clone());
    let class_file = lower_fragment_to_jvm_class(&submission).expect("Option<char> reference carrier must lower");

    let method = class_file.methods.iter().find(|method| method.name == "main").expect("entry method");
    assert_eq!(
        method.descriptor,
        JvmMethodDescriptor::new(
            vec![JvmTypeDescriptor::Object("java/lang/Object".to_string()), JvmTypeDescriptor::Char],
            JvmTypeDescriptor::Object("java/lang/Object".to_string())
        )
    );
    let instructions = method_code(&class_file, "main");
    assert!(instructions.iter().any(|instruction| matches!(instruction, JvmInstruction::ALoad(0))), "{instructions:?}");
    assert!(instructions.iter().any(|instruction| matches!(instruction, JvmInstruction::AReturn)), "{instructions:?}");

    let bytes = class_file.to_bytes().expect("class encoding");
    let decoded_class = JvmClassFile::from_bytes(&bytes).expect("class decoding");
    let index = decoded_class.methods.iter().position(|method| method.name == "main").expect("decoded entry method");
    let code = decoded_class.raw_method_code[index].as_ref().expect("decoded method code");
    let decoded = decode_instructions(code, &decoded_class.constant_pool);
    let mnemonics = decoded.iter().map(|instruction| instruction.mnemonic.as_str()).collect::<Vec<_>>();
    assert!(mnemonics.contains(&"aload_0") || mnemonics.contains(&"aload"), "decoded={mnemonics:?}");
    assert_eq!(mnemonics.last().copied(), Some("areturn"), "decoded={mnemonics:?}");
}

/// 携带身份的名义类型引用存储时同样走擦除引用载体（不经泛型实参形状）。
#[test]
fn nominal_with_registered_reference_layout_lowers_to_reference_carrier() {
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let submission = option_flow_plan(vec![nominal.clone()], nominal);
    let class_file = lower_fragment_to_jvm_class(&submission).expect("reference nominal must lower");
    let method = class_file.methods.iter().find(|method| method.name == "main").expect("entry method");
    assert_eq!(
        method.descriptor,
        JvmMethodDescriptor::new(
            vec![JvmTypeDescriptor::Object("nyar/layout/Option$0".to_string())],
            JvmTypeDescriptor::Object("nyar/layout/Option$0".to_string())
        )
    );
    class_file.to_bytes().expect("class encoding");
}

/// `Nominal` 携带的布局身份未在计划中注册：合同破坏，必须结构化失败。
#[test]
fn nominal_with_unregistered_layout_is_rejected_structurally() {
    let nominal = NyarType::Nominal { layout: 7, spelling: Identifier::new("Option") };
    let submission = option_flow_plan(vec![nominal.clone()], nominal);
    let error = lower_fragment_to_jvm_class(&submission).expect_err("unregistered layout identity must fail");
    assert!(error.contains("未在计划中注册"), "{error}");
}

/// 值存储聚合同样指向 companion 布局类：JVM 没有内联结构体，逐字段拷贝由
/// `AggregateCopy` 承担，是唯一允许的复制路径。
#[test]
fn value_storage_nominal_uses_layout_class_name() {
    let mut layout = option_char_layout();
    layout.storage = StorageKind::Value;
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let mut fixture = function("jvm.option_flow", nominal.clone(), vec![nominal.clone()]);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![layout], ..AggregateLayoutPlan::default() });

    let class_file = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect("value aggregate must lower");
    let method = class_file.methods.iter().find(|method| method.name == "main").expect("entry method");
    assert_eq!(
        method.descriptor,
        JvmMethodDescriptor::new(
            vec![JvmTypeDescriptor::Object("nyar/layout/Option$0".to_string())],
            JvmTypeDescriptor::Object("nyar/layout/Option$0".to_string())
        )
    );
}

/// 零字段值存储聚合与 wasm 标量语义一致：没有单槽 JVM 载体，必须结构化失败。
#[test]
fn zero_field_value_storage_nominal_is_rejected_structurally() {
    let mut layout = option_char_layout();
    layout.storage = StorageKind::Value;
    layout.fields.clear();
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let mut fixture = function("jvm.option_flow", nominal.clone(), vec![nominal.clone()]);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![layout], ..AggregateLayoutPlan::default() });

    let error = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect_err("zero-field value aggregate must fail");
    assert!(error.contains("值存储聚合"), "{error}");
}

/// 裸 `Named` 短名残差不携带身份，仍必须结构化失败。
#[test]
fn bare_named_residual_is_rejected_structurally() {
    let residual = NyarType::Named(Identifier::new("Option"));
    let submission = option_flow_plan(vec![residual.clone()], residual);
    let error = lower_fragment_to_jvm_class(&submission).expect_err("bare `Named` residual must fail");
    assert!(error.contains("缺少该形状的 JVM 载体合同"), "{error}");
}
