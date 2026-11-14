//! Native-bound PE residual (Futamura 1st projection product for `legend-vm`).
//!
//! Guests residualize into [`nyar_analyzer::residual::ResidualSink`]; this crate's **product**
//! sink records ops for **native / x64 → Windows PE**, not NyarVM bytecode.
//!
//! Full x64 lowering is still incomplete; specialization always produces a
//! [`NativeResidualModule`], and [`emit_native_pe`] wraps a minimal PE whose exit
//! code comes from a residual `main` that evaluates to an integer. Anything the
//! lowering cannot honestly represent — evaluation failure or a non-integer
//! result — is an explicit error, never a success-shaped empty body.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use miette::{Result, miette};
use nyar_analyzer::residual::ResidualSink;
use nyar_emitter::nyar_backend_native::{
    NativeAbi, NativeTextInstruction, NativeTextProgramFunction, NativeTextProgramImage, encode_native_text_function,
    encode_native_text_program_with_imports,
};

use super::PeCompiler;

/// Target lane for host-script native residuals (product path).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeResidualTarget {
    /// x86-64 native frames (legend-vm endpoint).
    #[default]
    Native,
}

/// 原生产品目标：决定容器、调用约定、宿主导入面与 effect 通道来源。
///
/// 目标身份来自规范四元组，不从宿主 `cfg` 猜；产品入口必须显式选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProductTarget {
    /// Windows `PE` DLL：Win64 + `kernel32!WriteFile` 经 IAT，effect 通道是隐藏首参（文件句柄）。
    Windows,
    /// Linux `ELF` `.so`：SysV64 + `write(2)` 系统调用，effect 通道是隐藏首参（fd）。
    Linux,
    /// Linux `ELF` `.node`：SysV64，effect 通道固定 stdout（fd 1），导出 Node-API 注册函数。
    Node,
    /// CPython 扩展 `.so`：SysV64，effect 通道固定 stdout（fd 1），导出 `PyInit_<name>`。
    Python,
}

impl NativeProductTarget {
    /// 对应的文本 LIR 调用约定。
    fn abi(self) -> NativeAbi {
        match self {
            Self::Windows => NativeAbi::Win64,
            Self::Linux | Self::Node | Self::Python => NativeAbi::SystemV64,
        }
    }

    /// 该目标是否用宿主导入（IAT）实现 effect 写入。
    fn uses_host_imports(self) -> bool {
        matches!(self, Self::Windows)
    }

    /// effect 通道是否由宿主作为隐藏首个整数形参传入。
    ///
    /// Node 扩展没有宿主传入的句柄：产物自己固定写 stdout（fd 1）。
    fn sink_from_parameter(self) -> bool {
        !matches!(self, Self::Node | Self::Python)
    }

    /// 目标名称，用于结构化错误。
    fn name(self) -> &'static str {
        match self {
            Self::Windows => "Windows PE",
            Self::Linux => "Linux ELF",
            Self::Node => "Linux Node addon",
            Self::Python => "CPython extension",
        }
    }
}

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
    /// Product target (always [`NativeResidualTarget::Native`] on this path).
    pub target: NativeResidualTarget,
    /// Residual functions (includes `main`).
    pub functions: Vec<NativeResidualFunction>,
}

impl NativeResidualModule {
    /// Total residual ops across all functions (proves specialize ran).
    pub fn op_count(&self) -> usize {
        self.functions.iter().map(|function| function.ops.len()).sum()
    }

    /// Whether this residual is bound to the native product lane.
    pub fn is_native_bound(&self) -> bool {
        self.target == NativeResidualTarget::Native
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
        NativeResidualModule { name: self.module_name, target: NativeResidualTarget::Native, functions: self.functions }
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
    /// 静态字符串描述符地址；携带字面量以支持无分配的 effect 写入。
    Str(String),
}

/// 把残差模块降低为多函数文本程序（导出函数 ABI），平台显式给出。
///
/// 未知输入通过该平台的整数形参进入（Win64 4 个 / SysV64 6 个），残差函数按名导出。
///
/// 产品载体：整数、布尔与**静态字符串描述符**（镜像内只读 `{rel_ptr,len}` + UTF-8，
/// 由宿主读取、永不释放）。含 `print` 的模块额外声明一个隐藏的首个整数形参作为
/// effect 通道（宿主提供的可写句柄/fd，作为**每个**残差函数的首个隐藏整数形参），
/// `print` 按目标 OS 的写入原语输出：Windows 经 `kernel32!WriteFile`（IAT），
/// Linux 经 `write(2)` 系统调用；两者都保持 guest 顺序。字符串走静态描述符，
/// 整数走十进制格式化（负数带 `-`），布尔写 `true`/`false`，`nil` 写空串。
/// 对象、运行时解析调用、除法、字符串比较/拼接与字符串作为实参都显式失败
/// 闭合，不在产品路径里静默吞掉语义。
pub fn lower_native_residual_to_program(
    module: &NativeResidualModule,
    target: NativeProductTarget,
) -> Result<NativeTextProgramImage, String> {
    lower_native_residual_to_program_with_entry(module, target, None)
}

/// 同上；`python_module` 只对 [`NativeProductTarget::Python`] 生效，决定
/// `PyInit_<name>` 导出名与模块名。
fn lower_native_residual_to_program_with_entry(
    module: &NativeResidualModule,
    target: NativeProductTarget,
    python_module: Option<&str>,
) -> Result<NativeTextProgramImage, String> {
    if !module.is_native_bound() {
        return Err("residual is not native-bound".into());
    }
    // 产品级 effect：`print` 可出现在任意残差函数；含 effect 的模块给每个函数
    // 增加同一个隐藏的首个整数形参（宿主 effect 通道），调用时逐层传递。
    let has_print = module.functions.iter().any(|function| function.ops.iter().any(|op| matches!(op, NativeResidualOp::Print(_))));
    // 导入面按目标冻结：Windows 的 effect 写入是 IAT 导入；Node 扩展要调用
    // Node-API 构造返回值；Linux 的 effect 是 syscall，没有导入。
    let imports: Vec<(String, String)> = match target {
        NativeProductTarget::Windows if has_print => vec![("kernel32".to_string(), "WriteFile".to_string())],
        NativeProductTarget::Node => vec![(String::new(), "napi_create_int32".to_string())],
        NativeProductTarget::Python => {
            vec![(String::new(), "PyModule_New".to_string()), (String::new(), "PyModule_AddIntConstant".to_string())]
        }
        NativeProductTarget::Windows | NativeProductTarget::Linux => Vec::new(),
    };
    // Node 扩展的 effect 通道固定 stdout，不占宿主形参；其余目标由宿主传入句柄/fd。
    let sink_from_parameter = has_print && target.sink_from_parameter();

    let mut lowered = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        lowered.push(lower_residual_function(function, has_print, sink_from_parameter, target)?);
    }
    // 调用结果当前按整数载体传播；字符串返回值的残差函数不能作为调用目标，
    // 否则调用方会按错误载体继续使用 `rax`。
    let string_returners: Vec<&str> = lowered
        .iter()
        .filter(|(_, return_kind)| matches!(return_kind, Some(ProductValueKind::Str(_))))
        .map(|(function, _)| function.name.as_str())
        .collect();
    for (function, _) in &lowered {
        for op in &function.instructions {
            if let NativeTextInstruction::Call(name) = op
                && string_returners.contains(&name.as_str())
            {
                return Err(format!(
                    "residual function `{}` calls `{name}` which returns a string descriptor; string-valued calls are not implemented at the product boundary",
                    function.name
                ));
            }
        }
    }
    let mut functions: Vec<NativeTextProgramFunction> = lowered.iter().map(|(function, _)| function.clone()).collect();
    match target {
        NativeProductTarget::Node => {
            require_integer_main(module, &lowered, "Node addon")?;
            functions.push(node_addon_registration());
        }
        NativeProductTarget::Python => {
            let name = python_module.ok_or_else(|| "CPython 扩展必须提供模块名（`PyInit_<name>`）".to_string())?;
            if !is_c_module_name(name) {
                return Err(format!("CPython 模块名 `{name}` 不是合法的 C 标识符"));
            }
            require_integer_main(module, &lowered, "CPython extension")?;
            functions.push(python_module_init(name));
        }
        NativeProductTarget::Windows | NativeProductTarget::Linux => {}
    }
    // 调用约定与容器都由平台显式给出，编码器不提供默认 ABI。
    encode_native_text_program_with_imports(&functions, &imports, target.abi())
}

/// 嵌入型产物（Node / Python）的入口只能暴露残差 `main` 的整数/布尔值。
fn require_integer_main(
    module: &NativeResidualModule,
    lowered: &[(NativeTextProgramFunction, Option<ProductValueKind>)],
    product: &str,
) -> Result<(), String> {
    let kind = lowered
        .iter()
        .find(|(function, _)| function.name == "main")
        .and_then(|(_, kind)| kind.clone())
        .ok_or_else(|| format!("{product} residual has no `main` return value"))?;
    if !matches!(kind, ProductValueKind::Int | ProductValueKind::Bool) {
        return Err(format!("{product} residual `main` returns `{kind:?}`; only integer/boolean results can be exposed"));
    }
    let main = module.function("main").ok_or_else(|| format!("{product} residual has no `main`"))?;
    if !main.parameters.is_empty() {
        return Err(format!("{product} residual `main` must not take parameters: the embedder entry receives no guest inputs"));
    }
    Ok(())
}

/// CPython 模块名必须是合法 C 标识符（`PyInit_<name>` 是导出符号）。
fn is_c_module_name(name: &str) -> bool {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// `PyInit_<name>()` 包装体：创建模块并把残差 `main` 的值写为 `value` 属性。
fn python_module_init(module_name: &str) -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    NativeTextProgramFunction {
        name: format!("PyInit_{module_name}"),
        slot_count: 4,
        instructions: vec![
            // 先建模块，再求残差值：模块对象必须在返回前保持有效引用。
            Lir::LoadStringPtr(module_name.to_string()),
            Lir::StoreSlot(0),
            Lir::LoadSlotToArg { slot: 0, index: 0 },
            Lir::CallImport { module: String::new(), field: "PyModule_New".to_string() },
            Lir::StoreSlot(1),
            Lir::Call("main".to_string()),
            Lir::StoreSlot(3),
            Lir::LoadSlotToArg { slot: 1, index: 0 },
            Lir::LoadStringPtr("value".to_string()),
            Lir::StoreSlot(2),
            Lir::LoadSlotToArg { slot: 2, index: 1 },
            Lir::LoadSlotToArg { slot: 3, index: 2 },
            Lir::CallImport { module: String::new(), field: "PyModule_AddIntConstant".to_string() },
            Lir::LoadSlot(1),
            Lir::Return,
        ],
    }
}

/// `napi_register_module_v1(env, exports)` 包装体：把残差 `main` 的整数值包成
/// Node number 并作为 `module.exports` 返回。
fn node_addon_registration() -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    NativeTextProgramFunction {
        name: "napi_register_module_v1".to_string(),
        slot_count: 4,
        instructions: vec![
            Lir::LoadParam { index: 0, slot: 0 },
            Lir::Call("main".to_string()),
            Lir::StoreSlot(1),
            Lir::LoadSlotToArg { slot: 0, index: 0 },
            Lir::LoadSlotToArg { slot: 1, index: 1 },
            Lir::LoadSlotAddress(2),
            Lir::StoreSlot(3),
            Lir::LoadSlotToArg { slot: 3, index: 2 },
            Lir::CallImport { module: String::new(), field: "napi_create_int32".to_string() },
            Lir::LoadSlot(2),
            Lir::Return,
        ],
    }
}

/// 以导出函数 ABI 发射 `PE` DLL（宿主按 Win64 调用约定调用残差函数）。
pub fn emit_native_pe_library(module: &NativeResidualModule) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program(module, NativeProductTarget::Windows)?;
    PeCompiler.compile_library(&image).map_err(|error| error.to_string())
}

/// 以导出函数 ABI 发射 x86-64 `ELF` 共享库 `.so`（宿主 `dlopen` + `dlsym`）。
///
/// 与 `PE` DLL 对称：残差函数按名导出，未知输入走 SysV64 整数形参；含 `print`
/// 的模块用隐藏的首个整数形参作为 fd，调用 `write(2)`。
pub fn emit_native_elf_library(module: &NativeResidualModule) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program(module, NativeProductTarget::Linux)?;
    super::ElfCompiler.compile_library(&image).map_err(|error| error.to_string())
}

/// 发射 Node-API `.node` 扩展（x86-64 `ELF` 共享库 + `napi_register_module_v1`）。
///
/// 宿主 `require()` 时由 Node 的 Node-API 装载器调用注册函数；`main` 的整数值
/// 通过 `napi_create_int32` 包成 JS number 作为 `module.exports`。`print` 写 stdout。
pub fn emit_native_node_addon(module: &NativeResidualModule) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program(module, NativeProductTarget::Node)?;
    super::ElfCompiler.compile_library(&image).map_err(|error| error.to_string())
}

/// 发射 CPython 扩展 `.so`（x86-64 `ELF` + `PyInit_<module_name>`）。
///
/// 宿主 `import` 时由 CPython 调用模块初始化函数：`PyModule_New` 建模块，
/// `PyModule_AddIntConstant` 把残差 `main` 的整数值写为 `value` 属性；`print`
/// 写 stdout。导入符号按解释器全局符号作用域解析（与 Node-API 同一 GOT 合同）。
pub fn emit_native_python_extension(module: &NativeResidualModule, module_name: &str) -> Result<Vec<u8>, String> {
    let image = lower_native_residual_to_program_with_entry(module, NativeProductTarget::Python, Some(module_name))?;
    super::ElfCompiler.compile_library(&image).map_err(|error| error.to_string())
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
}

impl<'a> ResidualFunctionLowering<'a> {
    fn new(function: &'a NativeResidualFunction, target: NativeProductTarget) -> Self {
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

    /// 用 `(pointer, length)` 两个帧槽按目标 OS 的写入原语写 effect。
    ///
    /// 两个平台的通道都是隐藏的首个整数形参（Windows 文件句柄 / Linux fd）；
    /// 差别只在写入编码：Windows 走 `kernel32!WriteFile`（IAT + shadow space），
    /// Linux 走 `write(2)` 系统调用（rdi/rsi/rdx）。
    fn write_bytes(&mut self, pointer: u16, length: u16) -> Result<(), String> {
        let handle = self.effect_sink.ok_or_else(|| {
            format!("residual function `{}` writes effects without an effect sink", self.function.name)
        })?;
        self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: handle, index: 0 });
        self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: pointer, index: 1 });
        self.instructions.push(NativeTextInstruction::LoadSlotToArg { slot: length, index: 2 });
        match self.target {
            NativeProductTarget::Windows => {
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
            NativeProductTarget::Linux | NativeProductTarget::Node | NativeProductTarget::Python => {
                self.instructions.push(NativeTextInstruction::SyscallWrite);
            }
        }
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
}

fn lower_residual_function(
    function: &NativeResidualFunction,
    with_effect_sink: bool,
    sink_from_parameter: bool,
    target: NativeProductTarget,
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
    let mut lowering = ResidualFunctionLowering::new(function, target);
    lowering.sink_from_parameter = sink_from_parameter;
    let mut return_kind: Option<ProductValueKind> = None;
    let parameter_shift = if sink_from_parameter {
        let sink = lowering.alloc_slot();
        lowering.instructions.push(NativeTextInstruction::LoadParam { index: 0, slot: sink });
        lowering.effect_sink = Some(sink);
        1
    }
    else if with_effect_sink {
        // Node 扩展：effect 通道固定 stdout，不占宿主形参。
        let sink = lowering.alloc_slot();
        lowering.instructions.push(NativeTextInstruction::LoadI64(1));
        lowering.instructions.push(NativeTextInstruction::StoreSlot(sink));
        lowering.effect_sink = Some(sink);
        0
    }
    else {
        0
    };
    for (index, parameter) in function.parameters.iter().enumerate() {
        let slot = lowering.alloc_slot();
        lowering.instructions.push(NativeTextInstruction::LoadParam { index: (index + parameter_shift) as u8, slot });
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
                lowering.push_stack_value(ProductValueKind::Str(text.clone()));
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
                let destination = lowering.alloc_slot();
                lowering.instructions.push(NativeTextInstruction::LoadSlot(slot));
                lowering.instructions.push(NativeTextInstruction::StoreSlot(destination));
                lowering.locals.insert(name.clone(), (destination, kind));
            }
            NativeResidualOp::Pop => {
                lowering.pop()?;
            }
            NativeResidualOp::Add => lowering.binary_numeric(NativeTextInstruction::Add, ProductValueKind::Int)?,
            NativeResidualOp::Sub => lowering.binary_numeric(NativeTextInstruction::Sub, ProductValueKind::Int)?,
            NativeResidualOp::Mul => lowering.binary_numeric(NativeTextInstruction::Mul, ProductValueKind::Int)?,
            NativeResidualOp::Eq | NativeResidualOp::Ne => {
                let (right, right_kind) = lowering.pop()?;
                let (left, left_kind) = lowering.pop()?;
                // guest 的相等是结构化比较；不同载体不能按数值比较。
                if matches!(left_kind, ProductValueKind::Str(_)) || matches!(right_kind, ProductValueKind::Str(_)) {
                    return Err(format!(
                        "residual function `{}` compares string values; byte-wise comparison is not implemented at the product boundary",
                        function.name
                    ));
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
                if matches!(left_kind, ProductValueKind::Str(_)) || matches!(right_kind, ProductValueKind::Str(_)) {
                    return Err(format!(
                        "residual function `{}` orders strings; byte-wise ordering is not implemented at the product boundary",
                        function.name
                    ));
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
                // 导出函数 ABI 以整数返回；布尔按 0/1 载体。
                lowering.push_stack_value(ProductValueKind::Int);
            }
            NativeResidualOp::Ret => {
                let (slot, kind) = lowering.pop()?;
                if matches!(kind, ProductValueKind::Nil) {
                    return Err(format!(
                        "residual function `{}` returns `nil`; the product ABI carries integers, booleans and static strings",
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
                        ProductValueKind::Str(text) => lowering.write_static(text)?,
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
            NativeResidualOp::Concat
            | NativeResidualOp::Div
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

/// Lower a native residual to raw x64 bytes suitable for [`PeCompiler`].
///
/// 常量折叠只允许用于**已证明无副作用**的残差：任何 `Print` 或未知调用都会
/// 失败闭合，不得先执行原程序再把结果替成 `mov rax, imm`，否则运行期 effect
/// 会被编译器静默吞掉。纯度门通过后才在步数预算内折叠常量；非整数结果仍是
/// 明确的 not-implemented 错误，空函数体不会伪装成可运行的 exit-0 产品。
///
/// 机器码不在这里手写：residual 适配器只提交本 lane 的文本 LIR，帧序言/尾声、
/// 指令编码与重定位统一由 native 平台服务产出。
pub fn lower_native_residual_to_x64(module: &NativeResidualModule) -> Result<Vec<u8>, String> {
    if !module.is_native_bound() {
        return Err("residual is not native-bound".into());
    }
    // 编译时未知输入无法常量折叠：参数化残差必须走真实代码生成，该 lowering
    // 尚未接入，禁止用任何固定答案或空函数体冒充产品。
    let main = module.function("main").ok_or_else(|| "native residual has no `main` function".to_string())?;
    if !main.parameters.is_empty() {
        return Err(format!(
            "native x64 lowering for parameterized residuals is not implemented (unknown inputs: `{}`)",
            main.parameters.join("`, `")
        ));
    }
    ensure_effect_free_for_constant_folding(module)?;
    match eval_native_residual(module) {
        Ok(ResidualValue::Int(value)) => {
            let encoded = encode_native_text_function(&[NativeTextInstruction::LoadI64(value), NativeTextInstruction::Return])
                .map_err(|error| format!("native 平台 LIR 编码失败: {error}"))?;
            if !encoded.relocations.is_empty() {
                return Err("常量折叠残差不应产生直接调用重定位".into());
            }
            Ok(encoded.text)
        }
        Ok(other) => Err(format!("full x64 lowering is not implemented for non-integer residual results (got {other:?})")),
        Err(error) => Err(format!("native residual evaluation failed: {error}")),
    }
}

/// 残差纯度门：只允许无运行时副作用的常量折叠。
fn ensure_effect_free_for_constant_folding(module: &NativeResidualModule) -> Result<(), String> {
    for function in &module.functions {
        for op in &function.ops {
            match op {
                NativeResidualOp::PushString(text) => {
                    return Err(format!(
                        "residual function `{}` pushes string `{text}`; non-integer constant folding is not implemented",
                        function.name
                    ));
                }
                NativeResidualOp::Print(_) => {
                    return Err(format!(
                        "residual function `{}` contains `Print`; effects must be preserved at runtime, not consumed by compile-time folding",
                        function.name
                    ));
                }
                NativeResidualOp::Call { name, .. } if module.function(name).is_none() => {
                    return Err(format!("residual function `{}` calls unknown `{name}`", function.name));
                }
                // 运行时对象与运行时解析的调用属于保留的动态操作：不能把程序在
                // 编译期执行完再输出常量。
                NativeResidualOp::DynamicCall { name, .. } => {
                    return Err(format!(
                        "residual function `{}` contains runtime-resolved call `{name}`; dynamic calls must stay in the product",
                        function.name
                    ));
                }
                NativeResidualOp::TableNew
                | NativeResidualOp::TableArrayPush
                | NativeResidualOp::TableRecordSet(_)
                | NativeResidualOp::TableIndexSet
                | NativeResidualOp::TableFieldSet(_)
                | NativeResidualOp::TableFieldGet(_)
                | NativeResidualOp::TableIndexGet => {
                    return Err(format!(
                        "residual function `{}` builds or reads runtime objects; object shape must be preserved at runtime",
                        function.name
                    ));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Specialize → lower → Windows PE bytes (and optionally write to `output_path`).
pub fn emit_native_pe(module: &NativeResidualModule, output_path: Option<&std::path::Path>) -> Result<Vec<u8>> {
    let x64 = lower_native_residual_to_x64(module).map_err(|error| miette!("{error}"))?;
    let compiler = PeCompiler;
    match output_path {
        Some(path) => compiler.compile("main", &x64, path),
        None => compiler.compile_to_bytes(&x64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn residual_main(ops: Vec<NativeResidualOp>) -> NativeResidualModule {
        NativeResidualModule {
            name: "t".into(),
            target: NativeResidualTarget::Native,
            functions: vec![NativeResidualFunction { name: "main".into(), parameters: Vec::new(), ops }],
        }
    }

    /// 整数残差经 native 平台 LIR 编码：包含 `mov rax, imm64` 与完整返回尾声，
    /// residual 适配器不再手写任何机器码。
    #[test]
    fn integer_result_lowers_through_native_platform_lir() {
        let module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        let text = lower_native_residual_to_x64(&module).expect("integer residual must lower");
        let mut expected_load = vec![0x48, 0xB8];
        expected_load.extend_from_slice(&42u64.to_le_bytes());
        assert!(text.windows(expected_load.len()).any(|window| window == expected_load.as_slice()), "编码必须包含 mov rax, 42: {text:02x?}");
        assert!(text.ends_with(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]), "编码必须以帧还原 + ret 结束: {text:02x?}");
    }

    #[test]
    fn non_integer_result_fails_closed_instead_of_empty_body() {
        let module = residual_main(vec![NativeResidualOp::PushString("n:5".into()), NativeResidualOp::Ret]);
        let error = lower_native_residual_to_x64(&module).expect_err("non-integer residual must not report success");
        assert!(error.contains("not implemented"), "unexpected error: {error}");
    }

    #[test]
    fn evaluation_failure_propagates_instead_of_empty_body() {
        // `main` calls a missing function → 纯度门明确拒绝未知调用。
        let mut module = residual_main(vec![NativeResidualOp::Call { name: "absent".into(), argc: 0 }, NativeResidualOp::Ret]);
        module.functions.push(NativeResidualFunction {
            name: "other".into(),
            parameters: Vec::new(),
            ops: vec![NativeResidualOp::PushI64(0), NativeResidualOp::Ret],
        });
        let error = lower_native_residual_to_x64(&module).expect_err("evaluation failure must not report success");
        assert!(error.contains("unknown"), "unexpected error: {error}");
        assert!(error.contains("absent"), "evaluation message must be carried: {error}");
    }

    #[test]
    fn print_residual_fails_closed_instead_of_being_executed_at_compile_time() {
        let module =
            residual_main(vec![NativeResidualOp::PushI64(7), NativeResidualOp::Print(1), NativeResidualOp::PushI64(9), NativeResidualOp::Ret]);
        let error = lower_native_residual_to_x64(&module).expect_err("含 Print 的残差不得编译期折叠");
        assert!(error.contains("Print"), "unexpected error: {error}");
        assert!(error.contains("effects must be preserved at runtime"), "unexpected error: {error}");
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

    /// Node 扩展产物：Node-API 注册导出 + `napi_create_int32` 动态导入。
    #[test]
    fn node_addon_exposes_registration_and_napi_import() {
        use nyar_emitter::nyar_backend_native::{NativeExecutableKind, classify_native_image};

        let module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        let bytes = emit_native_node_addon(&module).expect("emit node addon");
        let facts = classify_native_image(&bytes).expect("addon must be a parseable image");
        assert_eq!(facts.kind, NativeExecutableKind::ElfShared, "Node 扩展是 ET_DYN");
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("napi_register_module_v1"), "必须导出 Node-API 注册函数");
        assert!(haystack.contains("napi_create_int32"), "必须声明 N-API 动态导入");
    }

    /// CPython 扩展产物：`PyInit_<name>` 导出 + CPython 动态导入。
    #[test]
    fn python_extension_exposes_module_init_and_cpython_imports() {
        use nyar_emitter::nyar_backend_native::{NativeExecutableKind, classify_native_image};

        let module = residual_main(vec![NativeResidualOp::PushI64(42), NativeResidualOp::Ret]);
        let bytes = emit_native_python_extension(&module, "demo").expect("emit python extension");
        let facts = classify_native_image(&bytes).expect("extension must be a parseable image");
        assert_eq!(facts.kind, NativeExecutableKind::ElfShared, "CPython 扩展是 ET_DYN");
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("PyInit_demo"), "必须导出 `PyInit_<module>`");
        assert!(haystack.contains("PyModule_New"), "必须声明 `PyModule_New` 导入");
        assert!(haystack.contains("PyModule_AddIntConstant"), "必须声明 `PyModule_AddIntConstant` 导入");
    }

    #[test]
    fn python_extension_requires_a_c_module_name() {
        let module = residual_main(vec![NativeResidualOp::PushI64(1), NativeResidualOp::Ret]);
        let error = emit_native_python_extension(&module, "bad-name").expect_err("非法模块名必须失败");
        assert!(error.contains("C 标识符"), "unexpected error: {error}");
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
        let image = lower_native_residual_to_program(&module, NativeProductTarget::Linux).expect("lower linux print residual");
        assert!(image.imports.is_empty(), "Linux 产品不得声明 PE 导入");
        assert!(image.idata.is_empty(), "Linux 产品不得生成 `.idata`");
        assert!(
            image.text.windows(7).any(|window| window == [0xB8, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x05]),
            "effect 写入必须是 `write(2)` 系统调用"
        );
    }
}
