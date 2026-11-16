//! Native-bound PE residual (Futamura 1st projection product for `legend-vm`).
//!
//! Guests residualize into [`nyar_analyzer::residual::ResidualSink`]; this crate's **product**
//! sink records ops for the **native / x64** target lane, not NyarVM bytecode.
//!
//! 产品路径只有一条：specialization 产出 [`NativeResidualModule`]，多函数 residual lowering
//! 提交本 lane 的文本 LIR，由 native 平台编码器产出共享库 / 宿主扩展镜像（`PE32+` DLL、
//! `ELF64 ET_DYN`）。不存在「纯度检查 → 求值 → 固定整数返回」的独立 exe 出口；常量折叠
//! 属于优化/参考求值阶段（[`eval_native_residual`]），不得冒充产品。无法真实表示的残差
//! 一律结构化失败，不产出成功形状的空体或固定答案。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use miette::Result;
use nyar_analyzer::residual::ResidualSink;
pub use nyar_emitter::nyar_backend_native::{NativeHostModule, NativePlatform, NativeProductTarget};
use nyar_emitter::nyar_backend_native::{
    HostMethod, HostProperty, HostValueKind, NATIVE_TEXT_ARENA_CAPACITY, NativeDataTarget, NativeTextInstruction, NativeTextProgramFunction,
    NativeTextProgramImage, any_returns_bool, any_returns_utf8, append_text_arena, compile_shared_container,
    encode_native_text_program_with_dependencies, is_c_identifier, node_addon_registration, node_method_wrapper, node_module_data,
    python_method_wrapper, python_module_data, python_module_exec, python_module_registration,
};

/// One residual operation (language-neutral; mirrors [`ResidualSink`]).
#[derive(Debug, Clone, PartialEq)]
pub enum NativeResidualOp {
    /// Push i64.
    PushI64(i64),
    /// Push bool.
    PushBool(bool),
    /// Push string.
    PushString(String),
    /// Push nil.
    PushNil,
    /// Load local.
    Load(String),
    /// Store local.
    Store(String),
    /// Pop.
    Pop,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// String concat.
    Concat,
    /// Logical not.
    Not,
    /// Print `argc` stack values.
    Print(usize),
    /// Label.
    Label(String),
    /// Jump.
    Jump(String),
    /// Jump if falsey.
    JumpIfFalse(String),
    /// Jump if truthy.
    JumpIfTrue(String),
    /// Call residual function.
    Call {
        /// Callee name.
        name: String,
        /// Argument count.
        argc: usize,
    },
    /// Push a fresh empty table/object.
    TableNew,
    /// Stack `[table, value]` → append to the array part, push the table.
    TableArrayPush,
    /// Stack `[table, value]` → set string key, push the table.
    TableRecordSet(String),
    /// Stack `[value, table, key]` → dynamic index write, push the assigned value.
    TableIndexSet,
    /// Stack `[value, table]` → dynamic field write, push the assigned value.
    TableFieldSet(String),
    /// Stack `[table]` → read a string field.
    TableFieldGet(String),
    /// Stack `[table, key]` → dynamic index read.
    TableIndexGet,
    /// Stack `[value]` → guest length (table entries / string chars).
    Length,
    /// Call resolved at runtime (guest builtin / global function / nil).
    DynamicCall {
        /// Callee name.
        name: String,
        /// Argument count.
        argc: usize,
    },
    /// Return.
    Ret,
}

/// One residual function body.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeResidualFunction {
    /// Function name.
    pub name: String,
    /// Parameter names.
    pub parameters: Vec<String>,
    /// Residual ops.
    pub ops: Vec<NativeResidualOp>,
}

/// Native-bound residual module (specialize product).
#[derive(Debug, Clone, PartialEq)]
pub struct NativeResidualModule {
    /// Module name.
    pub name: String,
    /// Residual functions (includes `main`).
    pub functions: Vec<NativeResidualFunction>,
}

impl NativeResidualModule {
    /// Total residual ops across all functions (proves specialize ran).
    pub fn op_count(&self) -> usize {
        self.functions.iter().map(|function| function.ops.len()).sum()
    }

    /// Look up a function by name.
    pub fn function(&self, name: &str) -> Option<&NativeResidualFunction> {
        self.functions.iter().find(|function| function.name == name)
    }
}

/// [`ResidualSink`] that records ops for native / PE emission.
#[derive(Debug)]
pub struct NativeResidualSink {
    module_name: String,
    functions: Vec<NativeResidualFunction>,
    current: Option<NativeResidualFunction>,
}

impl NativeResidualSink {
    /// Create a sink for `module_name`.
    pub fn new(module_name: impl Into<String>) -> Self {
        Self { module_name: module_name.into(), functions: Vec::new(), current: None }
    }

    /// Finish specialization into a native-bound residual module.
    pub fn finish(mut self) -> NativeResidualModule {
        if self.current.is_some() {
            self.end_function();
        }
        NativeResidualModule { name: self.module_name, functions: self.functions }
    }

    fn push_op(&mut self, op: NativeResidualOp) {
        let function = self.current.as_mut().expect("begin_function first");
        function.ops.push(op);
    }
}

impl ResidualSink for NativeResidualSink {
    fn push_i64(&mut self, value: i64) {
        self.push_op(NativeResidualOp::PushI64(value));
    }

    fn push_bool(&mut self, value: bool) {
        self.push_op(NativeResidualOp::PushBool(value));
    }

    fn push_string(&mut self, value: &str) {
        self.push_op(NativeResidualOp::PushString(value.to_string()));
    }

    fn push_nil(&mut self) {
        self.push_op(NativeResidualOp::PushNil);
    }

    fn load(&mut self, name: &str) {
        self.push_op(NativeResidualOp::Load(name.to_string()));
    }

    fn store(&mut self, name: &str) {
        self.push_op(NativeResidualOp::Store(name.to_string()));
    }

    fn pop(&mut self) {
        self.push_op(NativeResidualOp::Pop);
    }

    fn add(&mut self) {
        self.push_op(NativeResidualOp::Add);
    }

    fn sub(&mut self) {
        self.push_op(NativeResidualOp::Sub);
    }

    fn mul(&mut self) {
        self.push_op(NativeResidualOp::Mul);
    }

    fn div(&mut self) {
        self.push_op(NativeResidualOp::Div);
    }

    fn rem(&mut self) {
        self.push_op(NativeResidualOp::Rem);
    }

    fn eq(&mut self) {
        self.push_op(NativeResidualOp::Eq);
    }

    fn ne(&mut self) {
        self.push_op(NativeResidualOp::Ne);
    }

    fn lt(&mut self) {
        self.push_op(NativeResidualOp::Lt);
    }

    fn le(&mut self) {
        self.push_op(NativeResidualOp::Le);
    }

    fn gt(&mut self) {
        self.push_op(NativeResidualOp::Gt);
    }

    fn ge(&mut self) {
        self.push_op(NativeResidualOp::Ge);
    }

    fn concat(&mut self) {
        self.push_op(NativeResidualOp::Concat);
    }

    fn not(&mut self) {
        self.push_op(NativeResidualOp::Not);
    }

    fn print(&mut self, argc: usize) {
        self.push_op(NativeResidualOp::Print(argc));
    }

    fn label(&mut self, name: &str) {
        self.push_op(NativeResidualOp::Label(name.to_string()));
    }

    fn jump(&mut self, label: &str) {
        self.push_op(NativeResidualOp::Jump(label.to_string()));
    }

    fn jump_if_false(&mut self, label: &str) {
        self.push_op(NativeResidualOp::JumpIfFalse(label.to_string()));
    }

    fn jump_if_true(&mut self, label: &str) {
        self.push_op(NativeResidualOp::JumpIfTrue(label.to_string()));
    }

    fn begin_function(&mut self, name: &str) {
        if self.current.is_some() {
            self.end_function();
        }
        self.current = Some(NativeResidualFunction { name: name.to_string(), parameters: Vec::new(), ops: Vec::new() });
    }

    fn add_parameter(&mut self, name: &str) {
        let function = self.current.as_mut().expect("begin_function first");
        function.parameters.push(name.to_string());
    }

    fn end_function(&mut self) {
        let function = self.current.take().expect("begin_function first");
        self.functions.push(function);
    }

    fn call(&mut self, name: &str, argc: usize) {
        self.push_op(NativeResidualOp::Call { name: name.to_string(), argc });
    }

    fn ret(&mut self) {
        self.push_op(NativeResidualOp::Ret);
    }

    fn table_new(&mut self) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableNew);
        Ok(())
    }

    fn table_array_push(&mut self) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableArrayPush);
        Ok(())
    }

    fn table_record_set(&mut self, key: &str) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableRecordSet(key.to_string()));
        Ok(())
    }

    fn table_index_set(&mut self) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableIndexSet);
        Ok(())
    }

    fn table_field_set(&mut self, name: &str) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableFieldSet(name.to_string()));
        Ok(())
    }

    fn table_field_get(&mut self, name: &str) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableFieldGet(name.to_string()));
        Ok(())
    }

    fn table_index_get(&mut self) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::TableIndexGet);
        Ok(())
    }

    fn length(&mut self) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::Length);
        Ok(())
    }

    fn dynamic_call(&mut self, name: &str, argc: usize) -> std::result::Result<(), String> {
        self.push_op(NativeResidualOp::DynamicCall { name: name.to_string(), argc });
        Ok(())
    }
}

/// Specialize `source` for `language` into a **native-bound** residual module.
pub fn specialize_language(language: &str, source: &str, module_name: &str) -> Result<NativeResidualModule, String> {
    match language {
        "lua" => {
            let mut sink = NativeResidualSink::new(module_name);
            rusty_lua::specialize_lua_into(source, &mut sink)?;
            Ok(sink.finish())
        }
        "bash" | "sh" | "powershell" | "ps1" | "pwsh" | "tcl" | "c" => {
            Err(format!("PE specialization for '{language}' is stubbed; Lua → native residual is the first projection"))
        }
        other => Err(format!("no PE specialization hook for '{other}'")),
    }
}

/// Runtime table/object value of a guest residual (array part + string-keyed map).
///
/// 对象形状属于 guest 语言运行时；残差只保留 `TableNew` / 读写操作，不把静态
/// 形状折叠进产物。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResidualTable {
    /// 1-based array part stored densely from index 1.
    pub array: Vec<ResidualValue>,
    /// String-keyed map part.
    pub map: HashMap<String, ResidualValue>,
}

impl ResidualTable {
    fn length(&self) -> i64 {
        self.array.len() as i64
    }

    fn get(&self, key: &ResidualValue) -> ResidualValue {
        if let Some(index) = residual_integer_key(key) {
            return self.array.get((index - 1) as usize).cloned().unwrap_or(ResidualValue::Nil);
        }
        self.map.get(&key.to_key()).cloned().unwrap_or(ResidualValue::Nil)
    }

    fn set(&mut self, key: &ResidualValue, value: ResidualValue) {
        if let Some(index) = residual_integer_key(key) {
            let idx = (index - 1) as usize;
            if matches!(value, ResidualValue::Nil) {
                if idx < self.array.len() {
                    self.array[idx] = ResidualValue::Nil;
                    while self.array.last().map(|value| matches!(value, ResidualValue::Nil)).unwrap_or(false) {
                        self.array.pop();
                    }
                }
                return;
            }
            if idx >= self.array.len() {
                self.array.resize(idx + 1, ResidualValue::Nil);
            }
            self.array[idx] = value;
            return;
        }
        let map_key = key.to_key();
        if matches!(value, ResidualValue::Nil) {
            self.map.remove(&map_key);
        }
        else {
            self.map.insert(map_key, value);
        }
    }
}

fn residual_integer_key(key: &ResidualValue) -> Option<i64> {
    match key {
        ResidualValue::Int(value) if *value >= 1 => Some(*value),
        ResidualValue::Float(value) if value.fract() == 0.0 && *value >= 1.0 && value.is_finite() => Some(*value as i64),
        _ => None,
    }
}

/// Values used while interpreting a native residual (goldens / exit-code fold).
#[derive(Debug, Clone, PartialEq)]
pub enum ResidualValue {
    /// Nil.
    Nil,
    /// Bool.
    Bool(bool),
    /// Integer.
    Int(i64),
    /// Float（`/` 在非整数/零除时产生，与 guest 解释器一致）。
    Float(f64),
    /// String.
    String(String),
    /// Runtime table/object.
    Table(Rc<RefCell<ResidualTable>>),
}

impl ResidualValue {
    fn is_truthy(&self) -> bool {
        !matches!(self, ResidualValue::Nil | ResidualValue::Bool(false))
    }

    fn as_int(&self) -> Option<i64> {
        match self {
            ResidualValue::Int(value) => Some(*value),
            ResidualValue::Float(value) => Some(*value as i64),
            ResidualValue::Bool(true) => Some(1),
            ResidualValue::Bool(false) | ResidualValue::Nil => Some(0),
            // 与 guest 语义一致：字符串按数值解析，失败取 0；残差不再拒绝 guest
            // 已定义的动态转换。
            ResidualValue::String(value) => Some(value.parse().unwrap_or(0)),
            // guest 解释器把表按 0 参与算术。
            ResidualValue::Table(_) => Some(0),
        }
    }

    fn as_f64(&self) -> f64 {
        match self {
            ResidualValue::Int(value) => *value as f64,
            ResidualValue::Float(value) => *value,
            ResidualValue::Bool(true) => 1.0,
            ResidualValue::Bool(false) | ResidualValue::Nil => 0.0,
            ResidualValue::String(value) => value.parse().unwrap_or(0.0),
            ResidualValue::Table(_) => 0.0,
        }
    }

    fn is_integral(&self) -> bool {
        matches!(self, ResidualValue::Int(_))
    }

    /// Stable string key for golden comparison.
    pub fn to_key(&self) -> String {
        match self {
            ResidualValue::Nil => String::new(),
            ResidualValue::Bool(value) => value.to_string(),
            ResidualValue::Int(value) => value.to_string(),
            ResidualValue::Float(value) => {
                if value.fract() == 0.0 && value.is_finite() { format!("{value:.0}") } else { value.to_string() }
            }
            ResidualValue::String(value) => value.clone(),
            ResidualValue::Table(table) => format!("table:#{}", table.borrow().length()),
        }
    }
}

/// Interpret a native residual's `main` (semantic check without nyar-vm).
///
/// 该解释器是残差语义的**诊断/对照**工具，不是产品路径：编译期 lowering 只允许
/// 在残差已证明无副作用时折叠常量，并且受步数预算约束，不能把原程序完整执行后
/// 丢掉 effect。`main` 的形参是编译时未知输入，由调用方按名提供。
pub fn eval_native_residual(module: &NativeResidualModule) -> Result<ResidualValue, String> {
    eval_native_residual_with_inputs(module, &HashMap::new())
}

/// 以显式未知输入求值残差 `main`；缺省输入为 `nil`，与 guest 解释器读取
/// 未绑定环境变量的语义一致。
pub fn eval_native_residual_with_inputs(
    module: &NativeResidualModule,
    inputs: &HashMap<String, ResidualValue>,
) -> Result<ResidualValue, String> {
    eval_native_residual_with_trace(module, inputs).map(|(value, _)| value)
}

/// 以显式未知输入求值残差，并按发生顺序记录每次 `print` 的显示值。
///
/// 副作用顺序是 guest 语义的一部分：解释器与残差运行时必须产生同一序列。
pub fn eval_native_residual_with_trace(
    module: &NativeResidualModule,
    inputs: &HashMap<String, ResidualValue>,
) -> Result<(ResidualValue, Vec<Vec<String>>), String> {
    let main = module.function("main").ok_or_else(|| "native residual has no `main` function".to_string())?;
    let args: Vec<ResidualValue> =
        main.parameters.iter().map(|parameter| inputs.get(parameter).cloned().unwrap_or(ResidualValue::Nil)).collect();
    let mut frames = Vec::new();
    let mut steps = 0usize;
    let mut effects = Vec::new();
    let value = call_function(module, "main", args, &mut frames, &mut steps, &mut effects)?;
    Ok((value, effects))
}

/// 编译期常量折叠的执行步数上限；防止残差中的无限循环把编译器挂死。
const MAX_EVAL_STEPS: usize = 1_000_000;

fn call_function(
    module: &NativeResidualModule,
    name: &str,
    args: Vec<ResidualValue>,
    frames: &mut Vec<()>,
    steps: &mut usize,
    effects: &mut Vec<Vec<String>>,
) -> Result<ResidualValue, String> {
    if frames.len() > 64 {
        return Err("native residual call depth exceeded".into());
    }
    frames.push(());
    let function = module.function(name).ok_or_else(|| format!("missing residual function '{name}'"))?;
    let mut locals: HashMap<String, ResidualValue> = HashMap::new();
    for (index, param) in function.parameters.iter().enumerate() {
        locals.insert(param.clone(), args.get(index).cloned().unwrap_or(ResidualValue::Nil));
    }
    let labels = build_labels(&function.ops);
    let mut stack: Vec<ResidualValue> = Vec::new();
    let mut pc = 0usize;
    let result = loop {
        if pc >= function.ops.len() {
            break stack.pop().unwrap_or(ResidualValue::Nil);
        }
        *steps = steps.saturating_add(1);
        if *steps > MAX_EVAL_STEPS {
            return Err(format!("native residual evaluation exceeded {MAX_EVAL_STEPS} steps"));
        }
        match &function.ops[pc] {
            NativeResidualOp::PushI64(value) => stack.push(ResidualValue::Int(*value)),
            NativeResidualOp::PushBool(value) => stack.push(ResidualValue::Bool(*value)),
            NativeResidualOp::PushString(value) => stack.push(ResidualValue::String(value.clone())),
            NativeResidualOp::PushNil => stack.push(ResidualValue::Nil),
            NativeResidualOp::Load(name) => {
                stack.push(locals.get(name).cloned().unwrap_or(ResidualValue::Nil));
            }
            NativeResidualOp::Store(name) => {
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                locals.insert(name.clone(), value);
            }
            NativeResidualOp::Pop => {
                stack.pop();
            }
            NativeResidualOp::Add => residual_add(&mut stack)?,
            NativeResidualOp::Sub => bin_numeric(&mut stack, |a, b| a - b)?,
            NativeResidualOp::Mul => bin_numeric(&mut stack, |a, b| a * b)?,
            NativeResidualOp::Div => {
                let right = stack.pop().unwrap_or(ResidualValue::Nil);
                let left = stack.pop().unwrap_or(ResidualValue::Nil);
                let divisor = right.as_f64();
                if divisor == 0.0 {
                    stack.push(ResidualValue::Float(f64::NAN));
                }
                else {
                    stack.push(residual_numeric_result(left.as_f64() / divisor, &left, &right));
                }
            }
            NativeResidualOp::Rem => {
                let right = stack.pop().unwrap_or(ResidualValue::Nil);
                let left = stack.pop().unwrap_or(ResidualValue::Nil);
                let divisor = right.as_int().unwrap_or(0);
                stack.push(ResidualValue::Int(if divisor == 0 { 0 } else { left.as_int().unwrap_or(0) % divisor }));
            }
            NativeResidualOp::Eq => bin_cmp(&mut stack, |a, b| a == b)?,
            NativeResidualOp::Ne => bin_cmp(&mut stack, |a, b| a != b)?,
            NativeResidualOp::Lt => bin_num_cmp(&mut stack, |a, b| a < b)?,
            NativeResidualOp::Le => bin_num_cmp(&mut stack, |a, b| a <= b)?,
            NativeResidualOp::Gt => bin_num_cmp(&mut stack, |a, b| a > b)?,
            NativeResidualOp::Ge => bin_num_cmp(&mut stack, |a, b| a >= b)?,
            NativeResidualOp::Concat => {
                let right = stack.pop().unwrap_or(ResidualValue::Nil);
                let left = stack.pop().unwrap_or(ResidualValue::Nil);
                stack.push(ResidualValue::String(format!("{}{}", display_value(&left), display_value(&right))));
            }
            NativeResidualOp::Not => {
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                stack.push(ResidualValue::Bool(!value.is_truthy()));
            }
            NativeResidualOp::Print(argc) => {
                let mut args = Vec::with_capacity(*argc);
                for _ in 0..*argc {
                    args.push(stack.pop().unwrap_or(ResidualValue::Nil));
                }
                args.reverse();
                effects.push(args.iter().map(display_value).collect());
                let text = args.iter().map(display_value).collect::<Vec<_>>().join("\t");
                println!("{text}");
                if let Some(last) = args.last() {
                    stack.push(last.clone());
                }
                else {
                    stack.push(ResidualValue::Nil);
                }
            }
            NativeResidualOp::TableNew => stack.push(ResidualValue::Table(Rc::new(RefCell::new(ResidualTable::default())))),
            NativeResidualOp::TableArrayPush => {
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                let table = stack.pop().unwrap_or(ResidualValue::Nil);
                if let ResidualValue::Table(table) = &table {
                    table.borrow_mut().array.push(value);
                }
                stack.push(table);
            }
            NativeResidualOp::TableRecordSet(key) => {
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                let table = stack.pop().unwrap_or(ResidualValue::Nil);
                if let ResidualValue::Table(table) = &table {
                    table.borrow_mut().map.insert(key.clone(), value);
                }
                stack.push(table);
            }
            NativeResidualOp::TableIndexSet => {
                let key = stack.pop().unwrap_or(ResidualValue::Nil);
                let table = stack.pop().unwrap_or(ResidualValue::Nil);
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                if let ResidualValue::Table(table) = &table {
                    table.borrow_mut().set(&key, value.clone());
                }
                stack.push(value);
            }
            NativeResidualOp::TableFieldSet(name) => {
                let table = stack.pop().unwrap_or(ResidualValue::Nil);
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                if let ResidualValue::Table(table) = &table {
                    table.borrow_mut().map.insert(name.clone(), value.clone());
                }
                stack.push(value);
            }
            NativeResidualOp::TableFieldGet(name) => {
                let table = stack.pop().unwrap_or(ResidualValue::Nil);
                let value = match &table {
                    ResidualValue::Table(table) => table.borrow().map.get(name).cloned().unwrap_or(ResidualValue::Nil),
                    _ => ResidualValue::Nil,
                };
                stack.push(value);
            }
            NativeResidualOp::TableIndexGet => {
                let key = stack.pop().unwrap_or(ResidualValue::Nil);
                let table = stack.pop().unwrap_or(ResidualValue::Nil);
                let value = match &table {
                    ResidualValue::Table(table) => table.borrow().get(&key),
                    // guest 解释器允许对字符串做 1-based 字符索引。
                    ResidualValue::String(text) => {
                        residual_string_index(text, key.as_int().unwrap_or(0))
                    }
                    _ => ResidualValue::Nil,
                };
                stack.push(value);
            }
            NativeResidualOp::Length => {
                let value = stack.pop().unwrap_or(ResidualValue::Nil);
                let length = match &value {
                    ResidualValue::Table(table) => table.borrow().length(),
                    ResidualValue::String(text) => text.chars().count() as i64,
                    other => other.to_key().len() as i64,
                };
                stack.push(ResidualValue::Int(length));
            }
            NativeResidualOp::DynamicCall { name, argc } => {
                let mut args = Vec::with_capacity(*argc);
                for _ in 0..*argc {
                    args.push(stack.pop().unwrap_or(ResidualValue::Nil));
                }
                args.reverse();
                stack.push(dynamic_call(module, name, args, frames, steps, effects)?);
            }
            NativeResidualOp::Label(_) => {}
            NativeResidualOp::Jump(label) => {
                pc = *labels.get(label).ok_or_else(|| format!("unknown label '{label}'"))?;
                continue;
            }
            NativeResidualOp::JumpIfFalse(label) => {
                let cond = stack.pop().unwrap_or(ResidualValue::Nil);
                if !cond.is_truthy() {
                    pc = *labels.get(label).ok_or_else(|| format!("unknown label '{label}'"))?;
                    continue;
                }
            }
            NativeResidualOp::JumpIfTrue(label) => {
                let cond = stack.pop().unwrap_or(ResidualValue::Nil);
                if cond.is_truthy() {
                    pc = *labels.get(label).ok_or_else(|| format!("unknown label '{label}'"))?;
                    continue;
                }
            }
            NativeResidualOp::Call { name, argc } => {
                let mut args = Vec::with_capacity(*argc);
                for _ in 0..*argc {
                    args.push(stack.pop().unwrap_or(ResidualValue::Nil));
                }
                args.reverse();
                let value = call_function(module, name, args, frames, steps, effects)?;
                stack.push(value);
            }
            NativeResidualOp::Ret => {
                break stack.pop().unwrap_or(ResidualValue::Nil);
            }
        }
        pc += 1;
    };
    frames.pop();
    Ok(result)
}

fn build_labels(ops: &[NativeResidualOp]) -> HashMap<String, usize> {
    let mut labels = HashMap::new();
    for (index, op) in ops.iter().enumerate() {
        if let NativeResidualOp::Label(name) = op {
            labels.insert(name.clone(), index);
        }
    }
    labels
}

fn residual_add(stack: &mut Vec<ResidualValue>) -> Result<(), String> {
    let right = stack.pop().unwrap_or(ResidualValue::Nil);
    let left = stack.pop().unwrap_or(ResidualValue::Nil);
    // guest 的 `+` 在两侧都是字符串时是拼接，其余走数值转换。
    if let (ResidualValue::String(left), ResidualValue::String(right)) = (&left, &right) {
        stack.push(ResidualValue::String(format!("{left}{right}")));
        return Ok(());
    }
    stack.push(residual_numeric_result(left.as_f64() + right.as_f64(), &left, &right));
    Ok(())
}

fn bin_numeric(stack: &mut Vec<ResidualValue>, op: impl FnOnce(f64, f64) -> f64) -> Result<(), String> {
    let right = stack.pop().unwrap_or(ResidualValue::Nil);
    let left = stack.pop().unwrap_or(ResidualValue::Nil);
    stack.push(residual_numeric_result(op(left.as_f64(), right.as_f64()), &left, &right));
    Ok(())
}

fn residual_numeric_result(value: f64, left: &ResidualValue, right: &ResidualValue) -> ResidualValue {
    if left.is_integral() && right.is_integral() { ResidualValue::Int(value as i64) } else { ResidualValue::Float(value) }
}

fn residual_string_index(text: &str, index: i64) -> ResidualValue {
    if index >= 1 { text.chars().nth((index - 1) as usize).map(|ch| ResidualValue::String(ch.to_string())).unwrap_or(ResidualValue::Nil) }
    else { ResidualValue::Nil }
}

/// 运行时解析的 guest 调用：已残差化函数、`print`、`table_insert`，其余按
/// 未定义全局函数返回 nil（与解释器一致）。
fn dynamic_call(
    module: &NativeResidualModule,
    name: &str,
    args: Vec<ResidualValue>,
    frames: &mut Vec<()>,
    steps: &mut usize,
    effects: &mut Vec<Vec<String>>,
) -> Result<ResidualValue, String> {
    if module.function(name).is_some() {
        return call_function(module, name, args, frames, steps, effects);
    }
    match name {
        "print" => {
            effects.push(args.iter().map(display_value).collect());
            let text = args.iter().map(display_value).collect::<Vec<_>>().join("\t");
            println!("{text}");
            Ok(args.last().cloned().unwrap_or(ResidualValue::Nil))
        }
        "table_insert" => {
            if let (Some(ResidualValue::Table(table)), Some(value)) = (args.first(), args.get(1)) {
                table.borrow_mut().array.push(value.clone());
            }
            Ok(ResidualValue::Nil)
        }
        _ => Ok(ResidualValue::Nil),
    }
}

fn bin_cmp(stack: &mut Vec<ResidualValue>, op: impl FnOnce(&ResidualValue, &ResidualValue) -> bool) -> Result<(), String> {
    let right = stack.pop().unwrap_or(ResidualValue::Nil);
    let left = stack.pop().unwrap_or(ResidualValue::Nil);
    stack.push(ResidualValue::Bool(op(&left, &right)));
    Ok(())
}

fn bin_num_cmp(stack: &mut Vec<ResidualValue>, op: impl FnOnce(f64, f64) -> bool) -> Result<(), String> {
    let right = stack.pop().unwrap_or(ResidualValue::Nil);
    let left = stack.pop().unwrap_or(ResidualValue::Nil);
    stack.push(ResidualValue::Bool(op(left.as_f64(), right.as_f64())));
    Ok(())
}

fn display_value(value: &ResidualValue) -> String {
    value.to_key()
}

/// 残差产品边界上的值类别（导出函数 ABI 以 `i64` 承载）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ProductValueKind {
    Int,
    Bool,
    Nil,
    /// 字符串描述符地址。
    ///
    /// `Some(text)` 是编译期已知的字面量（描述符在 `.text`），`None` 是运行期在
    /// 镜像 arena 里构造的值；两者布局相同，宿主读取方式不区分来源。
    Str(Option<String>),
}

/// 把残差模块降低为多函数文本程序（导出函数 ABI），平台显式给出。
///
/// 未知输入通过该平台的整数形参进入（Win64 4 个 / SysV64 6 个），残差函数按名导出。
///
/// 产品载体：整数、布尔与**字符串描述符**（`{i32 self_relative_ptr; u32 len}` + UTF-8）。
/// 字面量描述符内联在 `.text`；运行期构造的字符串分配在 `.data` 的镜像文本 arena，
/// 两者都由镜像拥有、永不单独释放。扩展模块的入口包装体在每次宿主调用开始时把
/// arena 游标归零，宿主在同一次调用内复制所需内容，因此所有权不跨 CRT 转移、
/// 也不需要跨边界释放。arena 预算耗尽按陷入失败闭合。
///
/// 含 `print` 的模块额外声明一个隐藏的首个整数形参作为
/// effect 通道（宿主提供的可写句柄/fd，作为**每个**残差函数的首个隐藏整数形参），
/// `print` 按目标 OS 的写入原语输出：Windows 经 `kernel32!WriteFile`（IAT），
/// Linux 经 `write(2)` 系统调用；两者都保持 guest 顺序。字符串走描述符，
/// 整数走十进制格式化（负数带 `-`），布尔写 `true`/`false`，`nil` 写空串。
/// 混合载体拼接（字符串 + 数字）、字符串作为调用实参、对象与运行时解析调用
/// 都显式失败闭合，不在产品路径里静默吞掉语义。字符串的相等与排序按 UTF-8
/// 字节序（lexicographic）比较内容，与描述符地址无关。
pub fn lower_native_residual_to_program(
    module: &NativeResidualModule,
    target: NativeProductTarget,
) -> Result<NativeTextProgramImage, String> {
    // 产品级 effect：`print` 可出现在任意残差函数；含 effect 的模块给每个函数
    // 增加同一个隐藏的首个整数形参（宿主 effect 通道），调用时逐层传递。
    let has_print = module.functions.iter().any(|function| function.ops.iter().any(|op| matches!(op, NativeResidualOp::Print(_))));
    let host = target.host_module();
    let has_methods = host.is_some() && module.functions.iter().any(|function| function.name != "main");
    // Windows 的 IAT 必须指明提供符号的宿主模块：Node 的符号由 `node.exe` 导出，
    // CPython 的在版本化的 `pythonNNN.dll` 里（由构建环境提供，不能猜）。
    let host_library = match (&target, host) {
        (_, Some(NativeHostModule::Node)) if target.is_windows() => "node.exe".to_string(),
        (NativeProductTarget::Extension { library, .. }, Some(NativeHostModule::Python)) if target.is_windows() => {
            library.clone().ok_or_else(|| {
                "Windows CPython 扩展必须提供宿主导入 DLL 名（例如 `python312.dll`）：版本相关名不能由编译器猜".to_string()
            })?
        }
        _ => String::new(),
    };
    // `ELF` 没有 IAT：宿主库以 `DT_NEEDED` 显式声明，避免依赖「碰巧在全局作用域」。
    let native_dependencies: Vec<String> = match (&target, host) {
        (NativeProductTarget::Extension { library: Some(library), .. }, Some(NativeHostModule::Python)) if !target.is_windows() => {
            vec![library.clone()]
        }
        _ => Vec::new(),
    };
    // 普通库的 effect 通道由宿主传入；扩展模块自己写 stdout。
    let sink_from_parameter = has_print && target.sink_from_parameter();

    // 调用结果的载体必须与已降低的被调用函数一致；先做不动点迭代收敛返回载体，
    // 让跨函数的前向引用（先调用、后定义）也能得到正确类别，而不是默认整数。
    //
    // 运行期文本 arena 的偏移取决于适配器数据（模块定义/符号名）的实际长度，
    // 而方法集合又由函数降低结果决定；因此先用占位偏移收敛返回载体与方法集合，
    // 数据布局确定后再按真实偏移重新降低函数体。
    let probe_arena = if host.is_some() { Some(0u32) } else { None };
    let mut known_returns: HashMap<String, ProductValueKind> = HashMap::new();
    let mut probe_lowered = Vec::with_capacity(module.functions.len());
    for _ in 0..=module.functions.len() {
        probe_lowered = module
            .functions
            .iter()
            .map(|function| lower_residual_function(function, has_print, sink_from_parameter, &target, &known_returns, probe_arena))
            .collect::<Result<Vec<_>, String>>()?;
        let mut changed = false;
        for (function, return_kind) in &probe_lowered {
            if let Some(kind) = return_kind {
                // 运行期内容未知：调用方只能把它当作描述符地址。
                let normalized = match kind {
                    ProductValueKind::Str(_) => ProductValueKind::Str(None),
                    other => other.clone(),
                };
                if known_returns.get(&function.name) != Some(&normalized) {
                    known_returns.insert(function.name.clone(), normalized);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // 数据段属于适配器：CPython 的 `PyModuleDef` 与符号名必须随镜像布局，其他
    // 目标的产物不需要可写数据。
    let mut data = Vec::new();
    let mut data_relocations: Vec<(u32, NativeDataTarget)> = Vec::new();
    let mut utf8_exports = false;
    let mut bool_exports = false;
    let mut functions: Vec<NativeTextProgramFunction>;
    match host {
        Some(NativeHostModule::Node) => {
            // 残差语言约定：把 `main` 的整数/布尔结果暴露成属性 `value`；
            // 承载服务本身不要求任何特定符号名。
            let properties = extension_value_property("Node addon", module, &probe_lowered)?;
            let methods = extension_methods(
                module,
                &probe_lowered,
                "Node 扩展",
                "Node-API 回调的 `argv` 窗口最多解析整数实参",
                target.method_parameter_limit(),
            )?;
            let (node_data, property_name_offsets, method_name_offsets, error_message_offset) = node_module_data(&properties, &methods);
            data = node_data;
            let arena_offset = Some(append_text_arena(&mut data));
            utf8_exports = any_returns_utf8(&methods);
            bool_exports = any_returns_bool(&properties, &methods);
            let lowered = lower_all(module, has_print, sink_from_parameter, &target, &known_returns, arena_offset)?;
            functions = lowered.iter().map(|(function, _)| function.clone()).collect();
            for method in &methods {
                functions.push(node_method_wrapper(method, error_message_offset, target.platform(), &host_library, arena_offset));
            }
            functions.push(node_addon_registration(
                &properties,
                &property_name_offsets,
                &methods,
                &method_name_offsets,
                target.platform(),
                &host_library,
                arena_offset,
            ));
        }
        Some(NativeHostModule::Python) => {
            let properties = extension_value_property("CPython extension", module, &probe_lowered)?;
            if !is_c_identifier(&module.name) {
                return Err(format!(
                    "CPython 扩展模块名 `{}` 不是合法 C 标识符：`PyInit_<module>` 必须能在 C 中声明",
                    module.name
                ));
            }
            let methods = extension_methods(
                module,
                &probe_lowered,
                "CPython 扩展",
                "`PyArg_ParseTuple` 包装最多闭合整数实参",
                target.method_parameter_limit(),
            )?;
            let (definition, relocations, property_name_offsets, format_offsets) = python_module_data(&module.name, &properties, &methods);
            data = definition;
            data_relocations = relocations;
            let arena_offset = Some(append_text_arena(&mut data));
            utf8_exports = any_returns_utf8(&methods);
            bool_exports = any_returns_bool(&properties, &methods);
            let lowered = lower_all(module, has_print, sink_from_parameter, &target, &known_returns, arena_offset)?;
            functions = lowered.iter().map(|(function, _)| function.clone()).collect();
            for (method, format_offset) in methods.iter().zip(format_offsets) {
                functions.push(python_method_wrapper(method, format_offset, &host_library, arena_offset));
            }
            functions.push(python_module_exec(&properties, &property_name_offsets, &host_library, arena_offset));
            functions.push(python_module_registration(&module.name, &host_library));
        }
        None => {
            functions = probe_lowered.iter().map(|(function, _)| function.clone()).collect();
        }
    }
    // 导入面：OS 决定 effect 原语与导入 DLL，模块类型决定宿主 API。
    let mut imports: Vec<(String, String)> = Vec::new();
    if has_print && target.is_windows() {
        imports.push(("kernel32".to_string(), "WriteFile".to_string()));
        if host.is_some() {
            // 扩展模块没有宿主传入的句柄：自己取 stdout 句柄。
            imports.push(("kernel32".to_string(), "GetStdHandle".to_string()));
        }
    }
    match host {
        Some(NativeHostModule::Node) => {
            imports.push((host_library.clone(), "napi_create_int64".to_string()));
            imports.push((host_library.clone(), "napi_set_named_property".to_string()));
            if has_methods {
                for symbol in ["napi_create_function", "napi_get_cb_info", "napi_get_value_int64", "napi_throw_type_error"] {
                    imports.push((host_library.clone(), symbol.to_string()));
                }
            }
            if utf8_exports {
                imports.push((host_library.clone(), "napi_create_string_utf8".to_string()));
            }
            if bool_exports {
                imports.push((host_library.clone(), "napi_get_boolean".to_string()));
            }
        }
        Some(NativeHostModule::Python) => {
            // 多阶段初始化：宿主负责创建模块，扩展只提供已初始化的定义与执行钩子。
            imports.push((host_library.clone(), "PyModuleDef_Init".to_string()));
            // 模块常量与方法返回值都用 64 位整数装箱：Windows 的 C `long` 只有
            // 32 位，`PyModule_AddIntConstant` / `PyLong_FromLong` 会静默截断 i64。
            imports.push((host_library.clone(), "PyLong_FromLongLong".to_string()));
            imports.push((host_library.clone(), "PyModule_AddObject".to_string()));
            if has_methods {
                imports.push((host_library.clone(), "PyArg_ParseTuple".to_string()));
            }
            if utf8_exports {
                imports.push((host_library.clone(), "PyUnicode_FromStringAndSize".to_string()));
            }
            if bool_exports {
                imports.push((host_library.clone(), "PyBool_FromLong".to_string()));
            }
        }
        None => {}
    }
    // 调用约定与容器都由平台显式给出，编码器不提供默认 ABI。
    encode_native_text_program_with_dependencies(
        &functions,
        &imports,
        target.abi(),
        &data,
        &data_relocations,
        &native_dependencies,
    )
}

/// 残差语言约定：把 `main` 的整数/布尔结果暴露成模块属性 `value`。
///
/// `main` 必须存在、无宿主输入、返回可暴露的整数/布尔；属性事实交给目标侧
/// 承载服务，服务本身不认识 `main`/`value` 这两个名字。
fn extension_value_property(
    target: &str,
    module: &NativeResidualModule,
    lowered: &[(NativeTextProgramFunction, Option<ProductValueKind>)],
) -> Result<Vec<HostProperty>, String> {
    let main_return = lowered
        .iter()
        .find(|(function, _)| function.name == "main")
        .and_then(|(_, kind)| kind.clone())
        .ok_or_else(|| format!("{target} residual has no `main` return value"))?;
    let returns = match main_return {
        ProductValueKind::Int => HostValueKind::Int64,
        ProductValueKind::Bool => HostValueKind::Bool,
        other => {
            return Err(format!(
                "{target} residual `main` returns `{other:?}`; only integer/boolean results can be exposed to the host"
            ));
        }
    };
    let main = module.function("main").ok_or_else(|| format!("{target} residual has no `main`"))?;
    if !main.parameters.is_empty() {
        return Err(format!("{target} residual `main` must not take parameters: the extension entry receives no guest inputs"));
    }
    Ok(vec![HostProperty { name: "value".to_string(), symbol: "main".to_string(), returns }])
}

/// 收集可暴露方法（语言侧策略）：除 `main` 外的残差函数，返回整数/布尔或
/// 静态字符串描述符，且形参不超过该宿主的参数窗口。不支持的形状显式失败。
///
/// 产出的 [`HostMethod`] 集合交给目标侧承载服务；残差语言在这里决定哪些函数
/// 可以暴露，包装体构造不属于本 crate。
fn extension_methods(
    module: &NativeResidualModule,
    lowered: &[(NativeTextProgramFunction, Option<ProductValueKind>)],
    host: &str,
    window: &str,
    max_parameters: usize,
) -> Result<Vec<HostMethod>, String> {
    let mut methods = Vec::new();
    for (function, return_kind) in lowered {
        if function.name == "main" {
            continue;
        }
        let returns = match return_kind {
            Some(ProductValueKind::Int) => HostValueKind::Int64,
            Some(ProductValueKind::Bool) => HostValueKind::Bool,
            // 字符串描述符由宿主侧复制（`PyUnicode_FromStringAndSize` /
            // `napi_create_string_utf8`）；描述符属于镜像，不需要释放。
            Some(ProductValueKind::Str(_)) => HostValueKind::Utf8,
            other => {
                return Err(format!(
                    "{host}方法 `{}` 的返回载体是 `{other:?}`：只支持整数/布尔或静态字符串",
                    function.name
                ));
            }
        };
        let parameters = module.function(&function.name).map(|function| function.parameters.len()).unwrap_or(0);
        if parameters > max_parameters {
            return Err(format!(
                "{host}方法 `{}` 有 {parameters} 个形参；{window}（上限 {max_parameters}）",
                function.name
            ));
        }
        methods.push(HostMethod { name: function.name.clone(), symbol: function.name.clone(), parameters, returns });
    }
    Ok(methods)
}

/// 以导出函数 ABI 发射 `PE` DLL（宿主按 Win64 调用约定调用残差函数）。
pub fn emit_native_pe_library(module: &NativeResidualModule) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program(module, NativeProductTarget::Library(NativePlatform::windows_x64()))?;
    compile_shared_container(NativePlatform::windows_x64(), &image).map_err(|error| error.to_string())
}

/// 以导出函数 ABI 发射 x86-64 `ELF` 共享库 `.so`（宿主 `dlopen` + `dlsym`）。
///
/// 与 `PE` DLL 对称：残差函数按名导出，未知输入走 SysV64 整数形参；含 `print`
/// 的模块用隐藏的首个整数形参作为 fd，调用 `write(2)`。
pub fn emit_native_elf_library(module: &NativeResidualModule) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program(module, NativeProductTarget::Library(NativePlatform::linux_x64()))?;
    compile_shared_container(NativePlatform::linux_x64(), &image).map_err(|error| error.to_string())
}

/// 发射 Node-API 扩展（`.node` / `.dll`）。
///
/// 宿主 `require()` 时由 Node 的 Node-API 装载器调用注册函数；`main` 的整数值
/// 通过 `napi_create_int64` 写成 `exports.value`，残差函数注册为同名回调。
/// Linux 产物是 `ET_DYN`，Windows 产物是 `PE32+` DLL（`.data` + 基址重定位）。
pub fn emit_native_node_addon(module: &NativeResidualModule, platform: NativePlatform) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program(module, NativeProductTarget::extension(platform, NativeHostModule::Node))?;
    compile_extension_container(platform, &image)
}

/// 发射 CPython 扩展模块（Linux `.so` / Windows `.pyd`）。
///
/// 入口为 `PyInit_<module>`：返回多阶段 `PyModuleDef`（`m_slots` 指向
/// `Py_mod_exec`），模块由宿主创建，`value` 在 exec 阶段安装。Windows 的导入
/// DLL 名（例如 `python312.dll`）必须由调用方提供——版本相关的名字不能由编译器猜。
pub fn emit_native_python_extension(
    module: &NativeResidualModule,
    platform: NativePlatform,
    python_library: Option<&str>,
) -> Result<Vec<u8>, String> {
    let target = NativeProductTarget::Extension {
        platform,
        module: NativeHostModule::Python,
        library: python_library.map(str::to_string),
    };
    let image = lower_native_residual_to_program(module, target)?;
    compile_extension_container(platform, &image)
}

/// 按已冻结的平台身份选择容器：Windows 写 `PE32+` DLL，Linux 写 `ET_DYN`。
///
/// 容器装配归 native 目标 lane 的共享服务；残差产品与正式编译 lane 复用同一条链。
fn compile_extension_container(platform: NativePlatform, image: &NativeTextProgramImage) -> Result<Vec<u8>, String> {
    compile_shared_container(platform, image).map_err(|error| error.to_string())
}

struct ResidualFunctionLowering<'a> {
    function: &'a NativeResidualFunction,
    locals: HashMap<String, (u16, ProductValueKind)>,
    labels: HashMap<String, u32>,
    stack: Vec<(u16, ProductValueKind)>,
    next_slot: u16,
    next_label: u32,
    instructions: Vec<NativeTextInstruction>,
    /// 线性残差里的 `ret`/`jump` 之后是死代码；只允许标签重新开启可达区。
    returned: bool,
    /// `main` 的隐藏 effect 通道（第 1 个整数形参：Windows 文件句柄 / Linux fd）。
    effect_sink: Option<u16>,
    /// effect 通道是否来自宿主形参（Node 扩展固定 stdout，不占用形参）。
    sink_from_parameter: bool,
    /// 目标 OS：决定 effect 写入原语与导入面。
    target: NativeProductTarget,
    /// 运行期文本 arena 的游标偏移；`None` 表示该产物没有入口重置合同（普通库）。
    text_arena: Option<u32>,
}

impl<'a> ResidualFunctionLowering<'a> {
    fn new(function: &'a NativeResidualFunction, target: NativeProductTarget, text_arena: Option<u32>) -> Self {
        Self {
            function,
            locals: HashMap::new(),
            labels: HashMap::new(),
            stack: Vec::new(),
            next_slot: 0,
            next_label: 0,
            instructions: Vec::new(),
            returned: false,
            effect_sink: None,
            sink_from_parameter: false,
            target,
            text_arena,
        }
    }

    fn alloc_slot(&mut self) -> u16 {
        let slot = self.next_slot;
        self.next_slot = self.next_slot.saturating_add(1);
        slot
    }

    fn label_id(&mut self, name: &str) -> u32 {
        if let Some(id) = self.labels.get(name) {
            return *id;
        }
        let id = self.next_label;
        self.next_label += 1;
        self.labels.insert(name.to_string(), id);
        id
    }

    fn fresh_label(&mut self) -> u32 {
        let id = self.next_label;
        self.next_label += 1;
        id
    }

    fn push_stack_value(&mut self, kind: ProductValueKind) -> u16 {
        let slot = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::StoreSlot(slot));
        self.stack.push((slot, kind));
        slot
    }

    fn pop(&mut self) -> Result<(u16, ProductValueKind), String> {
        self.stack.pop().ok_or_else(|| format!("residual function `{}` underflows the operand stack", self.function.name))
    }

    fn binary_numeric(&mut self, instruction: NativeTextInstruction, result: ProductValueKind) -> Result<(), String> {
        let (right, right_kind) = self.pop()?;
        let (left, left_kind) = self.pop()?;
        if left_kind != ProductValueKind::Int || right_kind != ProductValueKind::Int {
            return Err(format!(
                "residual function `{}` uses `{instruction:?}` on non-integer operands at the product boundary",
                self.function.name
            ));
        }
        self.instructions.push(NativeTextInstruction::LoadSlot(left));
        self.instructions.push(NativeTextInstruction::LoadSlotToRcx(right));
        self.instructions.push(instruction);
        self.push_stack_value(result);
        Ok(())
    }

    /// 通过 effect 通道写入一段静态字节（`WriteFile(handle, ptr, len, 0, 0)`）。
    fn write_static(&mut self, text: &str) -> Result<(), String> {
        self.instructions.push(NativeTextInstruction::LoadStringPtr(text.to_string()));
        let pointer = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::StoreSlot(pointer));
        self.instructions.push(NativeTextInstruction::LoadStringLen(text.to_string()));
        let length = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::StoreSlot(length));
        self.write_bytes(pointer, length)
    }

    /// 用 `(pointer, length)` 两个帧槽按产物 OS 的写入原语写 effect。
    ///
    /// Windows 走 `kernel32!WriteFile`（IAT + shadow space）；普通库的句柄来自宿主
    /// 形参，扩展模块由 [`Self::effect_channel`] 经 `GetStdHandle` 取得。Linux 走
    /// `write(2)` 系统调用（rdi/rsi/rdx）。
    fn write_bytes(&mut self, pointer: u16, length: u16) -> Result<(), String> {
        let handle = self.effect_sink.ok_or_else(|| {
            format!("residual function `{}` writes effects without an effect sink", self.function.name)
        })?;
        self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: handle, index: 0 });
        self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: pointer, index: 1 });
        self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: length, index: 2 });
        if self.target.is_windows() {
            self.instructions.push(NativeTextInstruction::LoadI64(0));
            let written_out = self.alloc_slot();
            self.instructions.push(NativeTextInstruction::StoreSlot(written_out));
            self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: written_out, index: 3 });
            self.instructions.push(NativeTextInstruction::ZeroStackArg(0));
            self.instructions.push(NativeTextInstruction::CallImport {
                module: "kernel32".to_string(),
                field: "WriteFile".to_string(),
            });
        }
        else {
            self.instructions.push(NativeTextInstruction::SyscallWrite);
        }
        Ok(())
    }

    /// effect 通道的初始值：扩展模块固定 stdout，普通库由宿主形参传入。
    ///
    /// Windows 的 stdout 不是 fd 1：扩展模块经 `kernel32!GetStdHandle(-11)` 取句柄；
    /// Linux 直接用 fd 1。
    fn effect_channel(&mut self) -> Result<(), String> {
        let slot = self.alloc_slot();
        if self.target.sink_from_parameter() {
            self.instructions.push(NativeTextInstruction::LoadParam { index: 0, slot });
            self.effect_sink = Some(slot);
            return Ok(());
        }
        if self.target.is_windows() {
            // `STD_OUTPUT_HANDLE = -11`。
            self.instructions.push(NativeTextInstruction::LoadI64(-11));
            let constant = self.alloc_slot();
            self.instructions.push(NativeTextInstruction::StoreSlot(constant));
            self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: constant, index: 0 });
            self.instructions.push(NativeTextInstruction::CallImport {
                module: "kernel32".to_string(),
                field: "GetStdHandle".to_string(),
            });
        }
        else {
            self.instructions.push(NativeTextInstruction::LoadI64(1));
        }
        self.instructions.push(NativeTextInstruction::StoreSlot(slot));
        self.effect_sink = Some(slot);
        Ok(())
    }

    fn store_immediate(&mut self, value: i64) -> u16 {
        self.instructions.push(NativeTextInstruction::LoadI64(value));
        let slot = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::StoreSlot(slot));
        slot
    }

    /// 把整数槽按 guest 十进制格式写入 effect 通道（负数带 `-`）。
    fn write_integer(&mut self, value_slot: u16) -> Result<(), String> {
        let zero = self.store_immediate(0);
        let nonzero = self.fresh_label();
        let positive = self.fresh_label();
        let done = self.fresh_label();
        // `value == 0` 直接写 "0"。
        self.instructions.push(NativeTextInstruction::LoadSlot(value_slot));
        self.instructions.push(NativeTextInstruction::LoadSlotToRcx(zero));
        self.instructions.push(NativeTextInstruction::Eq);
        self.instructions.push(NativeTextInstruction::JumpIfZero(nonzero));
        self.write_static("0")?;
        self.instructions.push(NativeTextInstruction::Jump(done));
        self.instructions.push(NativeTextInstruction::Label(nonzero));
        // 负数：写 `-` 并取绝对值。
        self.instructions.push(NativeTextInstruction::LoadSlot(value_slot));
        self.instructions.push(NativeTextInstruction::LoadSlotToRcx(zero));
        self.instructions.push(NativeTextInstruction::Lt);
        self.instructions.push(NativeTextInstruction::JumpIfZero(positive));
        self.write_static("-")?;
        self.instructions.push(NativeTextInstruction::LoadSlot(value_slot));
        self.instructions.push(NativeTextInstruction::NegateRax);
        self.instructions.push(NativeTextInstruction::StoreSlot(value_slot));
        self.instructions.push(NativeTextInstruction::Label(positive));

        // 统计位数；用副本避免破坏原值。
        let digits_left = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::LoadSlot(value_slot));
        self.instructions.push(NativeTextInstruction::StoreSlot(digits_left));
        let count = self.store_immediate(0);
        let count_loop = self.fresh_label();
        let count_done = self.fresh_label();
        self.instructions.push(NativeTextInstruction::Label(count_loop));
        self.instructions.push(NativeTextInstruction::LoadSlot(digits_left));
        self.instructions.push(NativeTextInstruction::DivideByTen);
        self.instructions.push(NativeTextInstruction::StoreSlot(digits_left));
        self.instructions.push(NativeTextInstruction::LoadSlot(count));
        self.instructions.push(NativeTextInstruction::AddRaxImmediate(1));
        self.instructions.push(NativeTextInstruction::StoreSlot(count));
        self.instructions.push(NativeTextInstruction::LoadSlot(digits_left));
        self.instructions.push(NativeTextInstruction::JumpIfZero(count_done));
        self.instructions.push(NativeTextInstruction::Jump(count_loop));
        self.instructions.push(NativeTextInstruction::Label(count_done));

        // 24 字节连续缓冲区：槽 b0/b1/b2 覆盖 [b2 基址, b0 基址)。
        let _b0 = self.alloc_slot();
        let _b1 = self.alloc_slot();
        let buffer_start = self.alloc_slot();
        let length = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::LoadSlot(count));
        self.instructions.push(NativeTextInstruction::StoreSlot(length));
        let pointer = self.alloc_slot();
        // pointer = buffer_start + (count - 1)
        self.instructions.push(NativeTextInstruction::LoadSlot(count));
        self.instructions.push(NativeTextInstruction::SubRaxImmediate(1));
        let offset = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::StoreSlot(offset));
        self.instructions.push(NativeTextInstruction::LoadSlotAddress(buffer_start));
        self.instructions.push(NativeTextInstruction::LoadSlotToRcx(offset));
        self.instructions.push(NativeTextInstruction::Add);
        self.instructions.push(NativeTextInstruction::StoreSlot(pointer));

        // 从低位到高位，逐字节倒着填入缓冲区。
        let remaining = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::LoadSlot(value_slot));
        self.instructions.push(NativeTextInstruction::StoreSlot(remaining));
        let write_loop = self.fresh_label();
        let write_done = self.fresh_label();
        self.instructions.push(NativeTextInstruction::Label(write_loop));
        self.instructions.push(NativeTextInstruction::LoadSlot(remaining));
        self.instructions.push(NativeTextInstruction::DivideByTen);
        self.instructions.push(NativeTextInstruction::StoreSlot(remaining));
        self.instructions.push(NativeTextInstruction::AddRdxImmediate(i32::from(b'0')));
        self.instructions.push(NativeTextInstruction::LoadSlot(pointer));
        self.instructions.push(NativeTextInstruction::StoreByteAtRaxFromRdx);
        self.instructions.push(NativeTextInstruction::LoadSlot(pointer));
        self.instructions.push(NativeTextInstruction::SubRaxImmediate(1));
        self.instructions.push(NativeTextInstruction::StoreSlot(pointer));
        self.instructions.push(NativeTextInstruction::LoadSlot(count));
        self.instructions.push(NativeTextInstruction::SubRaxImmediate(1));
        self.instructions.push(NativeTextInstruction::StoreSlot(count));
        self.instructions.push(NativeTextInstruction::LoadSlot(count));
        self.instructions.push(NativeTextInstruction::JumpIfZero(write_done));
        self.instructions.push(NativeTextInstruction::Jump(write_loop));
        self.instructions.push(NativeTextInstruction::Label(write_done));

        self.instructions.push(NativeTextInstruction::LoadSlotAddress(buffer_start));
        let base = self.alloc_slot();
        self.instructions.push(NativeTextInstruction::StoreSlot(base));
        self.write_bytes(base, length)?;
        self.instructions.push(NativeTextInstruction::Label(done));
        Ok(())
    }

    /// 布尔按 guest 显示写 `true` / `false`。
    fn write_boolean(&mut self, value_slot: u16) -> Result<(), String> {
        let false_label = self.fresh_label();
        let done = self.fresh_label();
        self.instructions.push(NativeTextInstruction::LoadSlot(value_slot));
        self.instructions.push(NativeTextInstruction::JumpIfZero(false_label));
        self.write_static("true")?;
        self.instructions.push(NativeTextInstruction::Jump(done));
        self.instructions.push(NativeTextInstruction::Label(false_label));
        self.write_static("false")?;
        self.instructions.push(NativeTextInstruction::Label(done));
        Ok(())
    }

    /// 在镜像 arena 里分配一个 `{i32 self_relative_ptr = 8; u32 len}` 描述符，
    /// 返回描述符所在帧槽；描述符共 8 字节，载荷起点是描述符 + 8，由调用方写入。
    ///
    /// `total` 是载荷字节数的帧槽。分配按游标单调推进，越界以 `ud2` 陷入失败闭合；
    /// 扩展模块的入口包装体在每次宿主调用开始时把游标归零。
    fn allocate_text(&mut self, total: u16) -> Result<u16, String> {
        use NativeTextInstruction as Lir;
        let arena = self.text_arena.ok_or_else(|| {
            format!(
                "residual function `{}` constructs a runtime string, but this product has no entry-reset text arena contract",
                self.function.name
            )
        })?;
        // cursor_address = &[arena]
        self.instructions.push(Lir::LoadDataAddress(arena));
        let cursor_address = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(cursor_address));
        // old = [cursor_address]
        self.instructions.push(Lir::LoadSlot(cursor_address));
        self.instructions.push(Lir::LoadFromRax);
        let old = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(old));
        // need = total + 8（描述符本身 8 字节）
        self.instructions.push(Lir::LoadSlot(total));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let need = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(need));
        // new = old + need
        self.instructions.push(Lir::LoadSlot(old));
        self.instructions.push(Lir::LoadSlotToRcx(need));
        self.instructions.push(Lir::Add);
        let new = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(new));
        // new > capacity → trap（预算耗尽不能返回伪造的空字符串）。
        let capacity = self.store_immediate(i64::from(NATIVE_TEXT_ARENA_CAPACITY));
        let within_budget = self.fresh_label();
        self.instructions.push(Lir::LoadSlot(new));
        self.instructions.push(Lir::LoadSlotToRcx(capacity));
        self.instructions.push(Lir::Gt);
        self.instructions.push(Lir::JumpIfZero(within_budget));
        self.instructions.push(Lir::Trap);
        self.instructions.push(Lir::Label(within_budget));
        // [cursor_address] = new
        self.instructions.push(Lir::LoadSlot(cursor_address));
        self.instructions.push(Lir::LoadSlotToRcx(new));
        self.instructions.push(Lir::StoreRcxToRax);
        // descriptor = payload_base + old
        self.instructions.push(Lir::LoadDataAddress(arena.saturating_add(8)));
        self.instructions.push(Lir::LoadSlotToRcx(old));
        self.instructions.push(Lir::Add);
        let descriptor = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(descriptor));
        // 描述符字段：自相对指针常量 8，长度 `u32`。
        self.instructions.push(Lir::LoadSlot(descriptor));
        self.instructions.push(Lir::StoreDwordImmediateToRax(8));
        self.instructions.push(Lir::LoadSlot(descriptor));
        self.instructions.push(Lir::AddRaxImmediate(4));
        self.instructions.push(Lir::LoadSlotToRcx(total));
        self.instructions.push(Lir::StoreDwordRcxToRax);
        Ok(descriptor)
    }

    /// 字符串拼接：两个描述符的长度在运行期读取，结果写入 arena。
    fn concat_text(&mut self) -> Result<(), String> {
        use NativeTextInstruction as Lir;
        let (right, right_kind) = self.pop()?;
        let (left, left_kind) = self.pop()?;
        if !matches!(left_kind, ProductValueKind::Str(_)) || !matches!(right_kind, ProductValueKind::Str(_)) {
            return Err(format!(
                "residual function `{}` concatenates mixed carriers; guest number-to-string coercion is not implemented at the product boundary",
                self.function.name
            ));
        }
        self.instructions.push(Lir::LoadSlot(left));
        self.instructions.push(Lir::LoadDescriptorLength);
        let left_length = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(left_length));
        self.instructions.push(Lir::LoadSlot(right));
        self.instructions.push(Lir::LoadDescriptorLength);
        let right_length = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(right_length));
        self.instructions.push(Lir::LoadSlot(left_length));
        self.instructions.push(Lir::LoadSlotToRcx(right_length));
        self.instructions.push(Lir::Add);
        let total = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(total));
        let descriptor = self.allocate_text(total)?;
        // destination = descriptor + 8
        self.instructions.push(Lir::LoadSlot(descriptor));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let destination = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(destination));
        // 左字节区起点 = left + 8。
        self.instructions.push(Lir::LoadSlot(left));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let left_bytes = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(left_bytes));
        self.instructions.push(Lir::LoadSlot(destination));
        self.instructions.push(Lir::LoadSlotToRcx(left_bytes));
        self.instructions.push(Lir::LoadSlotToRdx(left_length));
        self.instructions.push(Lir::CopyBytes);
        // 右字节区起点 = right + 8，目标 = destination + left_length。
        self.instructions.push(Lir::LoadSlot(right));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let right_bytes = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(right_bytes));
        self.instructions.push(Lir::LoadSlot(destination));
        self.instructions.push(Lir::LoadSlotToRcx(left_length));
        self.instructions.push(Lir::Add);
        self.instructions.push(Lir::LoadSlotToRcx(right_bytes));
        self.instructions.push(Lir::LoadSlotToRdx(right_length));
        self.instructions.push(Lir::CopyBytes);
        self.instructions.push(Lir::LoadSlot(descriptor));
        self.push_stack_value(ProductValueKind::Str(None));
        Ok(())
    }

    /// 通过 effect 通道写入一个运行期文本描述符指向的字节。
    fn write_text(&mut self, slot: u16) -> Result<(), String> {
        use NativeTextInstruction as Lir;
        self.instructions.push(Lir::LoadSlot(slot));
        self.instructions.push(Lir::LoadDescriptorLength);
        let length = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(length));
        self.instructions.push(Lir::LoadSlot(slot));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let pointer = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(pointer));
        self.write_bytes(pointer, length)
    }

    /// 把字符串描述符的运行期长度读入新槽。
    fn text_length(&mut self, descriptor: u16) -> u16 {
        use NativeTextInstruction as Lir;
        self.instructions.push(Lir::LoadSlot(descriptor));
        self.instructions.push(Lir::LoadDescriptorLength);
        let length = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(length));
        length
    }

    /// 字符串按字节序（lexicographic）比较：先逐字节比较公共前缀，再按长度定序，
    /// 得到 `-1 / 0 / 1` 后应用谓词得到布尔。
    ///
    /// 描述符地址不参与比较；字面量与运行期 arena 文本使用同一布局与同一路径。
    fn compare_text_values(
        &mut self,
        left: u16,
        left_kind: ProductValueKind,
        right: u16,
        right_kind: ProductValueKind,
        predicate: NativeTextInstruction,
    ) -> Result<(), String> {
        use NativeTextInstruction as Lir;
        if !matches!(left_kind, ProductValueKind::Str(_)) || !matches!(right_kind, ProductValueKind::Str(_)) {
            return Err(format!(
                "residual function `{}` orders mixed carriers at the product boundary",
                self.function.name
            ));
        }
        let left_length = self.text_length(left);
        let right_length = self.text_length(right);
        // 字节区起点 = 描述符 + 8。
        self.instructions.push(Lir::LoadSlot(left));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let left_bytes = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(left_bytes));
        self.instructions.push(Lir::LoadSlot(right));
        self.instructions.push(Lir::AddRaxImmediate(8));
        let right_bytes = self.alloc_slot();
        self.instructions.push(Lir::StoreSlot(right_bytes));

        let index = self.store_immediate(0);
        let comparison = self.alloc_slot();
        let left_byte = self.alloc_slot();
        let right_byte = self.alloc_slot();
        let address = self.alloc_slot();
        let head = self.fresh_label();
        let tail = self.fresh_label();
        let bytes_differ = self.fresh_label();
        let greater_byte = self.fresh_label();
        let shorter = self.fresh_label();
        let done = self.fresh_label();

        self.instructions.push(Lir::Label(head));
        // index < len(left) && index < len(right)，否则进入尾部长度比较。
        self.instructions.push(Lir::LoadSlot(index));
        self.instructions.push(Lir::LoadSlotToRcx(left_length));
        self.instructions.push(Lir::Lt);
        self.instructions.push(Lir::JumpIfZero(tail));
        self.instructions.push(Lir::LoadSlot(index));
        self.instructions.push(Lir::LoadSlotToRcx(right_length));
        self.instructions.push(Lir::Lt);
        self.instructions.push(Lir::JumpIfZero(tail));
        // 取两侧同一下标的字节。
        self.instructions.push(Lir::LoadSlot(left_bytes));
        self.instructions.push(Lir::LoadSlotToRcx(index));
        self.instructions.push(Lir::Add);
        self.instructions.push(Lir::StoreSlot(address));
        self.instructions.push(Lir::LoadSlot(address));
        self.instructions.push(Lir::LoadByteFromRax);
        self.instructions.push(Lir::StoreSlot(left_byte));
        self.instructions.push(Lir::LoadSlot(right_bytes));
        self.instructions.push(Lir::LoadSlotToRcx(index));
        self.instructions.push(Lir::Add);
        self.instructions.push(Lir::StoreSlot(address));
        self.instructions.push(Lir::LoadSlot(address));
        self.instructions.push(Lir::LoadByteFromRax);
        self.instructions.push(Lir::StoreSlot(right_byte));
        // 相同则推进下标；不同则按字节定序。
        self.instructions.push(Lir::LoadSlot(left_byte));
        self.instructions.push(Lir::LoadSlotToRcx(right_byte));
        self.instructions.push(Lir::Eq);
        self.instructions.push(Lir::JumpIfZero(bytes_differ));
        self.instructions.push(Lir::LoadSlot(index));
        self.instructions.push(Lir::AddRaxImmediate(1));
        self.instructions.push(Lir::StoreSlot(index));
        self.instructions.push(Lir::Jump(head));
        self.instructions.push(Lir::Label(bytes_differ));
        self.instructions.push(Lir::LoadSlot(left_byte));
        self.instructions.push(Lir::LoadSlotToRcx(right_byte));
        self.instructions.push(Lir::Lt);
        self.instructions.push(Lir::JumpIfZero(greater_byte));
        self.instructions.push(Lir::LoadI64(-1));
        self.instructions.push(Lir::StoreSlot(comparison));
        self.instructions.push(Lir::Jump(done));
        self.instructions.push(Lir::Label(greater_byte));
        self.instructions.push(Lir::LoadI64(1));
        self.instructions.push(Lir::StoreSlot(comparison));
        self.instructions.push(Lir::Jump(done));
        // 公共前缀相同：短者在前。
        self.instructions.push(Lir::Label(tail));
        self.instructions.push(Lir::LoadI64(0));
        self.instructions.push(Lir::StoreSlot(comparison));
        self.instructions.push(Lir::LoadSlot(left_length));
        self.instructions.push(Lir::LoadSlotToRcx(right_length));
        self.instructions.push(Lir::Lt);
        self.instructions.push(Lir::JumpIfZero(shorter));
        self.instructions.push(Lir::LoadI64(-1));
        self.instructions.push(Lir::StoreSlot(comparison));
        self.instructions.push(Lir::Jump(done));
        self.instructions.push(Lir::Label(shorter));
        self.instructions.push(Lir::LoadSlot(left_length));
        self.instructions.push(Lir::LoadSlotToRcx(right_length));
        self.instructions.push(Lir::Gt);
        self.instructions.push(Lir::JumpIfZero(done));
        self.instructions.push(Lir::LoadI64(1));
        self.instructions.push(Lir::StoreSlot(comparison));
        // 谓词应用到 `-1 / 0 / 1`。
        self.instructions.push(Lir::Label(done));
        let zero = self.store_immediate(0);
        self.instructions.push(Lir::LoadSlot(comparison));
        self.instructions.push(Lir::LoadSlotToRcx(zero));
        self.instructions.push(predicate);
        self.push_stack_value(ProductValueKind::Bool);
        Ok(())
    }
}

fn lower_all(
    module: &NativeResidualModule,
    with_effect_sink: bool,
    sink_from_parameter: bool,
    target: &NativeProductTarget,
    known_returns: &HashMap<String, ProductValueKind>,
    text_arena: Option<u32>,
) -> Result<Vec<(NativeTextProgramFunction, Option<ProductValueKind>)>, String> {
    module
        .functions
        .iter()
        .map(|function| lower_residual_function(function, with_effect_sink, sink_from_parameter, target, known_returns, text_arena))
        .collect()
}

fn lower_residual_function(
    function: &NativeResidualFunction,
    with_effect_sink: bool,
    sink_from_parameter: bool,
    target: &NativeProductTarget,
    known_returns: &HashMap<String, ProductValueKind>,
    text_arena: Option<u32>,
) -> Result<(NativeTextProgramFunction, Option<ProductValueKind>), String> {
    // 该平台寄存器实参上限，减去隐藏 effect 通道占用（仅当通道来自宿主形参）。
    let total = target.abi().integer_argument_limit();
    let available = total - usize::from(sink_from_parameter);
    if function.parameters.len() > available {
        return Err(format!(
            "residual function `{}` has {} parameters; the {} text ABI supports at most {available} integer inputs{}",
            function.name,
            function.parameters.len(),
            target.abi().name(),
            if sink_from_parameter { " alongside the effect sink" } else { "" }
        ));
    }
    let mut lowering = ResidualFunctionLowering::new(function, target.clone(), text_arena);
    lowering.sink_from_parameter = sink_from_parameter;
    let mut return_kind: Option<ProductValueKind> = None;
    if with_effect_sink {
        // 普通库从宿主形参取通道（占一个实参位）；扩展模块自己取 stdout。
        lowering.effect_channel()?;
    }
    let parameter_shift = u8::from(sink_from_parameter && with_effect_sink);
    for (index, parameter) in function.parameters.iter().enumerate() {
        let slot = lowering.alloc_slot();
        lowering.instructions.push(NativeTextInstruction::LoadParam { index: index as u8 + parameter_shift, slot });
        lowering.locals.insert(parameter.clone(), (slot, ProductValueKind::Int));
    }

    for op in &function.ops {
        // `ret`/`jump` 后的线性残差不可达；标签是唯一重新可达的入口。
        if lowering.returned {
            if matches!(op, NativeResidualOp::Label(_)) {
                lowering.returned = false;
            }
            else {
                continue;
            }
        }
        match op {
            NativeResidualOp::PushI64(value) => {
                lowering.instructions.push(NativeTextInstruction::LoadI64(*value));
                lowering.push_stack_value(ProductValueKind::Int);
            }
            NativeResidualOp::PushBool(value) => {
                lowering.instructions.push(NativeTextInstruction::LoadI64(i64::from(*value)));
                lowering.push_stack_value(ProductValueKind::Bool);
            }
            NativeResidualOp::PushString(text) => {
                lowering.instructions.push(NativeTextInstruction::LoadString(text.clone()));
                lowering.push_stack_value(ProductValueKind::Str(Some(text.clone())));
            }
            NativeResidualOp::PushNil => {
                lowering.instructions.push(NativeTextInstruction::LoadI64(0));
                lowering.push_stack_value(ProductValueKind::Nil);
            }
            NativeResidualOp::Load(name) => {
                let (slot, kind) = lowering.locals.get(name).cloned().ok_or_else(|| {
                    format!("residual function `{}` loads unbound local `{name}` at the product boundary", function.name)
                })?;
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.push_stack_value(kind);
            }
            NativeResidualOp::Store(name) => {
                let (slot, kind) = lowering.pop()?;
                // 变量槽是**绑定身份**，不是一次性 SSA 值：循环回边必须能看到
                // 上一轮写入的值，因此重绑定复用首个分配到的槽，只更新载体类别。
                let destination = match lowering.locals.get(name) {
                    Some((existing, _)) => *existing,
                    None => {
                        let allocated = lowering.alloc_slot();
                        lowering.locals.insert(name.clone(), (allocated, kind.clone()));
                        allocated
                    }
                };
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.instructions.push(NativeTextInstruction::StoreSlot(destination));
                if let Some((_, existing_kind)) = lowering.locals.get_mut(name) {
                    *existing_kind = kind;
                }
            }
            NativeResidualOp::Pop => {
                lowering.pop()?;
            }
            NativeResidualOp::Add => lowering.binary_numeric(NativeTextInstruction::Add, ProductValueKind::Int)?,
            NativeResidualOp::Sub => lowering.binary_numeric(NativeTextInstruction::Sub, ProductValueKind::Int)?,
            NativeResidualOp::Mul => lowering.binary_numeric(NativeTextInstruction::Mul, ProductValueKind::Int)?,
            NativeResidualOp::Concat => lowering.concat_text()?,
            NativeResidualOp::Eq | NativeResidualOp::Ne => {
                let (right, right_kind) = lowering.pop()?;
                let (left, left_kind) = lowering.pop()?;
                // guest 的字符串相等是内容比较；不同载体的结构化比较仍失败闭合。
                if matches!(left_kind, ProductValueKind::Str(_)) || matches!(right_kind, ProductValueKind::Str(_)) {
                    let predicate =
                        if matches!(op, NativeResidualOp::Eq) { NativeTextInstruction::Eq } else { NativeTextInstruction::Ne };
                    lowering.compare_text_values(left, left_kind, right, right_kind, predicate)?;
                    continue;
                }
                if left_kind != right_kind || left_kind == ProductValueKind::Nil {
                    return Err(format!(
                        "residual function `{}` compares values of different carriers at the product boundary",
                        function.name
                    ));
                }
                let instruction = if matches!(op, NativeResidualOp::Eq) {
                    NativeTextInstruction::Eq
                }
                else {
                    NativeTextInstruction::Ne
                };
                lowering.instructions.push(NativeTextInstruction::LoadSlot(left));
                lowering.instructions.push(NativeTextInstruction::LoadSlotToRcx(right));
                lowering.instructions.push(instruction);
                lowering.push_stack_value(ProductValueKind::Bool);
            }
            NativeResidualOp::Lt | NativeResidualOp::Le | NativeResidualOp::Gt | NativeResidualOp::Ge => {
                let (right, right_kind) = lowering.pop()?;
                let (left, left_kind) = lowering.pop()?;
                if matches!(left_kind, ProductValueKind::Nil) || matches!(right_kind, ProductValueKind::Nil) {
                    return Err(format!(
                        "residual function `{}` orders `nil` at the product boundary",
                        function.name
                    ));
                }
                // 字符串按字节序比较（与 guest 的字典序一致），其它载体按有符号整数比较。
                if matches!(left_kind, ProductValueKind::Str(_)) || matches!(right_kind, ProductValueKind::Str(_)) {
                    let predicate = match op {
                        NativeResidualOp::Lt => NativeTextInstruction::Lt,
                        NativeResidualOp::Le => NativeTextInstruction::Le,
                        NativeResidualOp::Gt => NativeTextInstruction::Gt,
                        _ => NativeTextInstruction::Ge,
                    };
                    lowering.compare_text_values(left, left_kind, right, right_kind, predicate)?;
                    continue;
                }
                let instruction = match op {
                    NativeResidualOp::Lt => NativeTextInstruction::Lt,
                    NativeResidualOp::Le => NativeTextInstruction::Le,
                    NativeResidualOp::Gt => NativeTextInstruction::Gt,
                    _ => NativeTextInstruction::Ge,
                };
                lowering.instructions.push(NativeTextInstruction::LoadSlot(left));
                lowering.instructions.push(NativeTextInstruction::LoadSlotToRcx(right));
                lowering.instructions.push(instruction);
                lowering.push_stack_value(ProductValueKind::Bool);
            }
            NativeResidualOp::Not => {
                let (slot, kind) = lowering.pop()?;
                if kind != ProductValueKind::Bool {
                    return Err(format!(
                        "residual function `{}` applies logical `not` to a non-boolean at the product boundary",
                        function.name
                    ));
                }
                let zero = lowering.alloc_slot();
                lowering.instructions.push(NativeTextInstruction::LoadI64(0));
                lowering.instructions.push(NativeTextInstruction::StoreSlot(zero));
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.instructions.push(NativeTextInstruction::LoadSlotToRcx(zero));
                lowering.instructions.push(NativeTextInstruction::Eq);
                lowering.push_stack_value(ProductValueKind::Bool);
            }
            NativeResidualOp::Label(name) => {
                let id = lowering.label_id(name);
                lowering.instructions.push(NativeTextInstruction::Label(id));
            }
            NativeResidualOp::Jump(name) => {
                let id = lowering.label_id(name);
                lowering.instructions.push(NativeTextInstruction::Jump(id));
                lowering.returned = true;
            }
            NativeResidualOp::JumpIfFalse(name) => {
                let id = lowering.label_id(name);
                let (slot, kind) = lowering.pop()?;
                if kind != ProductValueKind::Bool {
                    return Err(format!(
                        "residual function `{}` branches on a non-boolean at the product boundary",
                        function.name
                    ));
                }
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.instructions.push(NativeTextInstruction::JumpIfZero(id));
            }
            NativeResidualOp::JumpIfTrue(name) => {
                let id = lowering.label_id(name);
                let skip = lowering.fresh_label();
                let (slot, kind) = lowering.pop()?;
                if kind != ProductValueKind::Bool {
                    return Err(format!(
                        "residual function `{}` branches on a non-boolean at the product boundary",
                        function.name
                    ));
                }
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.instructions.push(NativeTextInstruction::JumpIfZero(skip));
                lowering.instructions.push(NativeTextInstruction::Jump(id));
                lowering.instructions.push(NativeTextInstruction::Label(skip));
            }
            NativeResidualOp::Call { name, argc } => {
                // 只有来自宿主形参的 effect 通道会占用一个实参位置。
                let sink = if lowering.sink_from_parameter { lowering.effect_sink } else { None };
                let available = target.abi().integer_argument_limit() - usize::from(sink.is_some());
                if *argc > available {
                    return Err(format!(
                        "residual function `{}` calls `{name}` with {argc} arguments; the {} text ABI supports at most {available}",
                        function.name,
                        target.abi().name()
                    ));
                }
                let mut arguments = Vec::with_capacity(*argc);
                for _ in 0..*argc {
                    arguments.push(lowering.pop()?);
                }
                arguments.reverse();
                if arguments.iter().any(|(_, kind)| matches!(kind, ProductValueKind::Str(_))) {
                    return Err(format!(
                        "residual function `{}` passes a string descriptor to `{name}`; string-valued calls are not implemented at the product boundary",
                        function.name
                    ));
                }
                if let Some(sink) = sink {
                    lowering.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: sink, index: 0 });
                }
                let shift = u8::from(sink.is_some());
                for (index, (slot, _)) in arguments.iter().enumerate() {
                    lowering.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: *slot, index: index as u8 + shift });
                }
                lowering.instructions.push(NativeTextInstruction::Call(name.clone()));
                // 调用结果的载体来自已降低的被调用函数（不动点收敛）。
                let result_kind = known_returns.get(name).cloned().unwrap_or(ProductValueKind::Int);
                lowering.push_stack_value(result_kind);
            }
            NativeResidualOp::Ret => {
                let (slot, kind) = lowering.pop()?;
                if matches!(kind, ProductValueKind::Nil) {
                    return Err(format!(
                        "residual function `{}` returns `nil`; the product ABI carries integers, booleans and strings",
                        function.name
                    ));
                }
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.instructions.push(NativeTextInstruction::Return);
                // `ret` 之后的线性残差不可达；保持栈形态确定。
                lowering.stack.clear();
                lowering.returned = true;
                return_kind = Some(kind.clone());
            }
            NativeResidualOp::Print(argc) => {
                // 产品级 effect：只支持静态字符串实参（整数格式化尚未实现），
                // 每个值之间写 `\t`，结束时写 `\n`，与 guest `print` 的可观察顺序一致。
                let mut arguments = Vec::with_capacity(*argc);
                for _ in 0..*argc {
                    arguments.push(lowering.pop()?);
                }
                // 栈顶是最后一个实参；恢复从左到右的可观察顺序。
                arguments.reverse();
                for (index, (slot, kind)) in arguments.iter().enumerate() {
                    if index > 0 {
                        lowering.write_static("\t")?;
                    }
                    match kind {
                        ProductValueKind::Str(Some(text)) => lowering.write_static(text)?,
                        ProductValueKind::Str(None) => lowering.write_text(*slot)?,
                        ProductValueKind::Int => lowering.write_integer(*slot)?,
                        ProductValueKind::Bool => lowering.write_boolean(*slot)?,
                        // guest 把 `nil` 显示为空串。
                        ProductValueKind::Nil => {}
                    }
                }
                lowering.write_static("\n")?;
                if let Some((slot, kind)) = arguments.last() {
                    lowering.instructions.push(NativeTextInstruction::LoadSlot(*slot));
                    lowering.push_stack_value(kind.clone());
                }
                else {
                    lowering.instructions.push(NativeTextInstruction::LoadI64(0));
                    lowering.push_stack_value(ProductValueKind::Nil);
                }
            }
            NativeResidualOp::Div
            | NativeResidualOp::Rem
            | NativeResidualOp::Length
            | NativeResidualOp::TableNew
            | NativeResidualOp::TableArrayPush
            | NativeResidualOp::TableRecordSet(_)
            | NativeResidualOp::TableIndexSet
            | NativeResidualOp::TableFieldSet(_)
            | NativeResidualOp::TableFieldGet(_)
            | NativeResidualOp::TableIndexGet
            | NativeResidualOp::DynamicCall { .. } => {
                return Err(format!(
                    "residual function `{}` contains `{op:?}`, which the current Win64 text ABI cannot carry; failing closed instead of shrinking semantics",
                    function.name
                ));
            }
        }
    }

    Ok((NativeTextProgramFunction { name: function.name.clone(), slot_count: lowering.next_slot, instructions: lowering.instructions }, return_kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn residual_main(ops: Vec<NativeResidualOp>) -> NativeResidualModule {
        NativeResidualModule {
            name: "t".into(),
            functions: vec![NativeResidualFunction { name: "main".into(), parameters: Vec::new(), ops }],
        }
    }

    /// Linux 产品 lane 产出 `ET_DYN` 共享库并按名导出残差函数。
    #[test]
    fn linux_product_emits_a_shared_object_with_exports() {
        use nyar_emitter::nyar_backend_native::{NativeExecutableKind, classify_native_image};

        let module = residual_main(vec![NativeResidualOp::PushI64(7), NativeResidualOp::Ret]);
        let bytes = emit_native_elf_library(&module).expect("emit linux residual .so");
        let facts = classify_native_image(&bytes).expect("product must be a parseable image");
        assert_eq!(facts.kind, NativeExecutableKind::ElfShared, "必须是 ET_DYN 而不是 ET_EXEC");
        assert!(facts.shared, "共享库身份");
        assert_eq!(format!("{:?}", facts.arch), "X64", "必须是 AMD64 共享库");
        assert!(String::from_utf8_lossy(&bytes).contains("main"), "残差函数必须出现在动态符号表");
    }

    /// Node 扩展产物：Node-API 注册导出 + 导出对象属性写入所需的动态导入。
    #[test]
    fn node_addon_exposes_registration_and_napi_import() {
        use nyar_emitter::nyar_backend_native::{NativeExecutableKind, classify_native_image};

        let module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        let bytes = emit_native_node_addon(&module, NativePlatform::linux_x64()).expect("emit node addon");
        let facts = classify_native_image(&bytes).expect("addon must be a parseable image");
        assert_eq!(facts.kind, NativeExecutableKind::ElfShared, "Node 扩展是 ET_DYN");
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("napi_register_module_v1"), "必须导出 Node-API 注册函数");
        assert!(haystack.contains("napi_create_int64"), "残差 i64 必须按 `napi_create_int64` 装箱");
        assert!(haystack.contains("napi_set_named_property"), "必须把 `value` 写成导出属性");
        assert!(!haystack.contains("napi_create_function"), "没有方法时不得声明方法原语");
    }

    /// Node 方法表：`napi_create_function` 注册同名回调，回调经 `napi_get_cb_info`
    /// 取实参、`napi_get_value_int64` 解析、`napi_create_int64` 装箱。
    #[test]
    fn node_addon_registers_a_method_table() {
        let module = residual_with_method(NativeResidualFunction {
            name: "add".into(),
            parameters: vec!["a".into(), "b".into()],
            ops: vec![
                NativeResidualOp::Load("a".into()),
                NativeResidualOp::Load("b".into()),
                NativeResidualOp::Add,
                NativeResidualOp::Ret,
            ],
        });
        let image = lower_native_residual_to_program(&module, NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Node)).expect("lower node methods");
        assert!(image.symbols.iter().any(|(name, _)| name == "__nyar_node_wrap_add"), "必须生成 Node 回调包装体");
        for symbol in ["napi_create_function", "napi_get_cb_info", "napi_get_value_int64", "napi_throw_type_error"] {
            assert!(image.elf_imports.iter().any(|import| import == symbol), "缺少方法原语导入 `{symbol}`");
        }
        assert!(image.data.windows(4).any(|window| window == b"add\0"), "方法名必须进入数据段");
        let bytes = emit_native_node_addon(&module, NativePlatform::linux_x64()).expect("emit node methods addon");
        assert!(String::from_utf8_lossy(&bytes).contains("__nyar_node_wrap_add"));
    }

    /// Linux 的 `print` 用 `write(2)` 系统调用，不生成任何 PE 导入表。
    #[test]
    fn linux_print_uses_the_write_syscall_without_host_imports() {
        let module = residual_main(vec![
            NativeResidualOp::PushString("a".into()),
            NativeResidualOp::Print(1),
            NativeResidualOp::PushI64(0),
            NativeResidualOp::Ret,
        ]);
        let image = lower_native_residual_to_program(&module, NativeProductTarget::Library(NativePlatform::linux_x64())).expect("lower linux print residual");
        assert!(image.imports.is_empty(), "Linux 产品不得声明 PE 导入");
        assert!(image.idata.is_empty(), "Linux 产品不得生成 `.idata`");
        assert!(
            image.text.windows(7).any(|window| window == [0xB8, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x05]),
            "effect 写入必须是 `write(2)` 系统调用"
        );
    }

    /// CPython 扩展产物：多阶段 `PyModuleDef`（`Py_mod_exec` 槽位）与
    /// `PyInit_<module>` / `__nyar_py_exec` 两个入口。
    #[test]
    fn python_extension_exposes_pyinit_and_module_def() {
        let mut module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        module.name = "demo".into();
        let image = lower_native_residual_to_program(&module, NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Python)).expect("lower python extension");
        assert!(image.symbols.iter().any(|(name, _)| name == "PyInit_demo"), "必须导出 `PyInit_demo`");
        assert!(image.symbols.iter().any(|(name, _)| name == "__nyar_py_exec"), "必须导出多阶段执行钩子");
        assert!(
            !image.elf_imports.iter().any(|symbol| symbol == "PyModule_Create2"),
            "多阶段初始化不调用 `PyModule_Create2`：模块由宿主创建"
        );
        assert!(image.elf_imports.iter().any(|symbol| symbol == "PyLong_FromLongLong"), "`value` 必须 64 位装箱");
        assert!(image.elf_imports.iter().any(|symbol| symbol == "PyModule_AddObject"), "模块常量经 `PyModule_AddObject` 安装");
        assert!(
            !image.elf_imports.iter().any(|symbol| symbol == "PyModule_AddIntConstant"),
            "`PyModule_AddIntConstant` 走 32 位 C `long`，不得用于 i64 `value`"
        );
        assert!(!image.data.is_empty(), "`PyModuleDef` 必须位于镜像数据段");
        // 模块名、`m_slots` 与槽位函数指针各一条自相对重定位。
        assert_eq!(image.data_relocations.len(), 3, "模块名 / m_slots / 槽位函数指针");
        assert!(image.data_references.iter().any(|(_, data_offset)| *data_offset == 0), "入口必须取 `PyModuleDef` 地址");
        let bytes = emit_native_python_extension(&module, NativePlatform::linux_x64(), None).expect("emit python extension");
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("PyInit_demo"));
        assert!(haystack.contains("__nyar_py_exec"));
    }

    #[test]
    fn python_extension_rejects_invalid_module_names() {
        let mut module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        module.name = "not-an-identifier".into();
        let error = lower_native_residual_to_program(&module, NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Python)).expect_err("非法模块名必须失败闭合");
        assert!(error.contains("C 标识符"), "unexpected error: {error}");
    }

    fn residual_with_method(method: NativeResidualFunction) -> NativeResidualModule {
        let mut module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        module.name = "demo".into();
        module.functions.push(method);
        module
    }

    /// 方法表：每个非 `main` 残差函数变成一个 `PyMethodDef`，`ml_meth` 指向
    /// `PyArg_ParseTuple` 包装体，`ml_name` 指向数据段里 NUL 结尾的方法名。
    #[test]
    fn python_extension_builds_a_method_table() {
        let module = residual_with_method(NativeResidualFunction {
            name: "add".into(),
            parameters: vec!["a".into(), "b".into()],
            ops: vec![
                NativeResidualOp::Load("a".into()),
                NativeResidualOp::Load("b".into()),
                NativeResidualOp::Add,
                NativeResidualOp::Ret,
            ],
        });
        let image = lower_native_residual_to_program(&module, NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Python)).expect("lower python methods");
        assert!(image.symbols.iter().any(|(name, _)| name == "__nyar_py_wrap_add"), "必须生成方法包装体");
        assert!(image.elf_imports.iter().any(|symbol| symbol == "PyArg_ParseTuple"), "方法实参必须经宿主解析");
        assert!(image.elf_imports.iter().any(|symbol| symbol == "PyLong_FromLongLong"), "返回值必须 64 位装箱成 Python int");
        // 方法名与格式串必须在数据段里以 NUL 结尾。
        assert!(image.data.windows(4).any(|window| window == b"add\0"), "方法名必须进入数据段");
        assert!(image.data.windows(6).any(|window| window == b"nn:add"), "格式串必须带方法名");
        // 编码器必须把命名函数解析成 `.text` 偏移；容器不应看到 `Function`。
        assert!(
            image.data_relocations.iter().all(|(_, target)| matches!(target, NativeDataTarget::Text(_) | NativeDataTarget::Data(_))),
            "命名函数重定位必须在编码阶段解析为节内偏移"
        );
        assert_eq!(image.data_relocations.len(), 6, "模块名 / m_methods / 方法名 / ml_meth / m_slots / 槽位函数指针");
        let bytes = emit_native_python_extension(&module, NativePlatform::linux_x64(), None).expect("emit python methods extension");
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("__nyar_py_wrap_add"));
        assert!(haystack.contains("PyArg_ParseTuple"));
    }

    #[test]
    fn python_methods_fail_closed_beyond_the_host_argument_window() {
        let module = residual_with_method(NativeResidualFunction {
            name: "many".into(),
            parameters: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            ops: vec![NativeResidualOp::Load("a".into()), NativeResidualOp::Ret],
        });
        let error = lower_native_residual_to_program(&module, NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Python)).expect_err("超出行参窗口必须失败闭合");
        assert!(error.contains("PyArg_ParseTuple"), "unexpected error: {error}");
        assert!(error.contains("many"), "错误必须指名方法: {error}");
    }

    /// 字符串返回的方法：镜像内描述符由宿主侧复制成原生字符串，不跨 CRT 释放。
    #[test]
    fn methods_can_return_static_strings() {
        let module = residual_with_method(NativeResidualFunction {
            name: "greet".into(),
            parameters: Vec::new(),
            ops: vec![NativeResidualOp::PushString("hi".into()), NativeResidualOp::Ret],
        });
        let image = lower_native_residual_to_program(
            &module,
            NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Python),
        )
        .expect("字符串方法必须可暴露");
        assert!(image.elf_imports.iter().any(|symbol| symbol == "PyUnicode_FromStringAndSize"));
        let node = lower_native_residual_to_program(
            &module,
            NativeProductTarget::extension(NativePlatform::linux_x64(), NativeHostModule::Node),
        )
        .expect("Node 也必须支持字符串方法");
        assert!(node.elf_imports.iter().any(|symbol| symbol == "napi_create_string_utf8"));
    }

    /// Windows 的 `PyArg_ParseTuple` 只剩 rdx/r8 两个寄存器给 out-指针，因此
    /// CPython 方法在 Win64 上最多 2 个整数形参（Linux 为 4）。
    #[test]
    fn windows_python_method_window_is_two_parameters() {
        let module = residual_with_method(NativeResidualFunction {
            name: "three".into(),
            parameters: vec!["a".into(), "b".into(), "c".into()],
            ops: vec![NativeResidualOp::Load("a".into()), NativeResidualOp::Ret],
        });
        let windows = NativeProductTarget::Extension {
            platform: NativePlatform::windows_x64(),
            module: NativeHostModule::Python,
            library: Some("python312.dll".into()),
        };
        let error = lower_native_residual_to_program(&module, windows).expect_err("Win64 行参窗口更窄");
        assert!(error.contains("three"), "unexpected error: {error}");
        assert!(error.contains("上限 2"), "错误必须说明该 OS 的窗口: {error}");
    }

    #[test]
    fn resolved_target_keeps_platform_and_contract() {
        let resolver = nyar::TargetResolver::with_host(NativePlatform::linux_x64().canonical());
        let resolved = resolver
            .resolve_with_platform(nyar::TargetRequest::Node, NativePlatform::windows_x64().canonical())
            .expect("node on windows resolves");
        let target = NativeProductTarget::from_resolved(&resolved).expect("product target");
        assert!(target.is_windows(), "显式目标必须覆盖宿主平台");
        assert_eq!(target.host_module(), Some(NativeHostModule::Node));
        assert_eq!(target.platform().canonical(), resolved.platform, "产物目标必须保留冻结的平台身份");
    }

    #[test]
    fn resolved_target_rejects_unimplemented_platform() {
        let resolver = nyar::TargetResolver::with_host(NativePlatform::linux_x64().canonical());
        let resolved = resolver.resolve_label("aarch64-apple-macos").expect("macos resolves as a platform");
        let error = NativeProductTarget::from_resolved(&resolved).expect_err("macOS 容器尚未实现");
        assert!(error.contains("尚不支持"), "unexpected error: {error}");
    }

    #[test]
    fn windows_python_extension_requires_the_host_library() {
        let mut module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        module.name = "demo".into();
        let target = NativeProductTarget::extension(NativePlatform::windows_x64(), NativeHostModule::Python);
        let error = lower_native_residual_to_program(&module, target).expect_err("缺 DLL 名必须失败闭合");
        assert!(error.contains("python312.dll"), "unexpected error: {error}");
    }
}
