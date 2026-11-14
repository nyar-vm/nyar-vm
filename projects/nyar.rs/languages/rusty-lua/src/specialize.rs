//! PE / Futamura 1st projection for Lua: specialize interpret w.r.t. a concrete program.
//!
//! Residualizes the same match arms as [`super::interpret`] into a [`ResidualSink`],
//! rather than a hand-written divergent Lua compiler.

use std::collections::{BTreeSet, HashMap};

use crate::script::{LuaExpr, LuaLValue, LuaScript, LuaStmt};

use nyar_analyzer::residual::ResidualSink;

/// Specialize Lua `source` into `sink` (Futamura projection 1).
pub fn specialize_lua_into(source: &str, sink: &mut dyn ResidualSink) -> Result<(), String> {
    let script = LuaScript::parse(source).map_err(|error| format!("lua parse: {error}"))?;
    specialize_lua_script(&script, sink)
}

/// Specialize a parsed Lua script into `sink`.
pub fn specialize_lua_script(script: &LuaScript, sink: &mut dyn ResidualSink) -> Result<(), String> {
    let mut functions: HashMap<String, (Vec<String>, Vec<LuaStmt>)> = HashMap::new();
    let mut top_level = Vec::new();

    for stmt in &script.statements {
        if let LuaStmt::FunctionDef { name, params, body } = stmt {
            functions.insert(name.clone(), (params.clone(), body.clone()));
        }
        else {
            top_level.push(stmt.clone());
        }
    }

    // 残化函数只绑定自身形参；guest 解释器按调用时的动态作用域读取外层变量。
    // 两者对自由变量的语义不同，必须在特化期失败闭合，不能静默读成 `nil`。
    for (name, (params, body)) in &functions {
        let mut bound: BTreeSet<String> = params.iter().cloned().collect();
        let mut free = Vec::new();
        let mut seen = BTreeSet::new();
        scan_free_block(body, &functions, &mut bound, &mut free, &mut seen);
        if let Some(variable) = free.first() {
            return sink.unsupported(&format!(
                "free variable `{variable}` in function `{name}` (guest functions read outer bindings dynamically)"
            ));
        }
    }

    // 顶层自由变量是编译时未知输入：解释器把它们当作初始环境，残差的 `main`
    // 必须以同名形参接收，而不是折叠成常量或静默读成 `nil`。
    let main_inputs = {
        let mut bound = BTreeSet::new();
        let mut free = Vec::new();
        let mut seen = BTreeSet::new();
        scan_free_block(&top_level, &functions, &mut bound, &mut free, &mut seen);
        free
    };

    for (name, (params, body)) in &functions {
        sink.begin_function(name);
        for param in params {
            sink.add_parameter(param);
        }
        residualize_block(body, sink, &functions, None)?;
        sink.push_nil();
        sink.ret();
        sink.end_function();
    }

    sink.begin_function("main");
    for input in &main_inputs {
        sink.add_parameter(input);
    }
    sink.push_nil();
    sink.store("__pe_last");
    residualize_block(&top_level, sink, &functions, None)?;
    sink.load("__pe_last");
    sink.ret();
    sink.end_function();
    Ok(())
}

/// 按首次读取顺序收集一段残化代码里的自由变量。
///
/// `bound` 是当前可见的绑定集合（形参、`local`、先前赋值）；扫描会就地扩展它。
/// 该扫描只描述绑定关系，不特化操作——真正拒绝未知构造仍由
/// [`residualize_stmt`] / [`residualize_expr`] 决定。
fn scan_free_block(
    statements: &[LuaStmt],
    functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
    bound: &mut BTreeSet<String>,
    free: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    for stmt in statements {
        scan_free_stmt(stmt, functions, bound, free, seen);
    }
}

fn note_free(free: &mut Vec<String>, seen: &mut BTreeSet<String>, bound: &BTreeSet<String>, name: &str) {
    if bound.contains(name) || !seen.insert(name.to_string()) {
        return;
    }
    free.push(name.to_string());
}

fn scan_free_stmt(
    stmt: &LuaStmt,
    functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
    bound: &mut BTreeSet<String>,
    free: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    match stmt {
        LuaStmt::LocalDecl { names, values } => {
            for value in values {
                scan_free_expr(value, functions, bound, free, seen);
            }
            for name in names {
                bound.insert(name.clone());
            }
        }
        LuaStmt::Assign { targets, values } => {
            for value in values {
                scan_free_expr(value, functions, bound, free, seen);
            }
            for target in targets {
                match target {
                    LuaLValue::Name(name) => {
                        bound.insert(name.clone());
                    }
                    LuaLValue::Field { table, .. } | LuaLValue::Index { table, .. } => {
                        scan_free_expr(table, functions, bound, free, seen);
                    }
                }
            }
        }
        LuaStmt::If { condition, then_block, else_block } => {
            scan_free_expr(condition, functions, bound, free, seen);
            // 分支内声明不泄漏到分支外。
            scan_free_block(then_block, functions, &mut bound.clone(), free, seen);
            scan_free_block(else_block, functions, &mut bound.clone(), free, seen);
        }
        LuaStmt::While { condition, body } => {
            scan_free_expr(condition, functions, bound, free, seen);
            scan_free_block(body, functions, &mut bound.clone(), free, seen);
        }
        LuaStmt::Repeat { body, condition } => {
            scan_free_block(body, functions, &mut bound.clone(), free, seen);
            scan_free_expr(condition, functions, bound, free, seen);
        }
        LuaStmt::ForNumeric { name, start, limit, step, body } => {
            scan_free_expr(start, functions, bound, free, seen);
            scan_free_expr(limit, functions, bound, free, seen);
            if let Some(step) = step {
                scan_free_expr(step, functions, bound, free, seen);
            }
            let mut inner = bound.clone();
            inner.insert(name.clone());
            scan_free_block(body, functions, &mut inner, free, seen);
        }
        LuaStmt::Return(value) => {
            if let Some(expr) = value {
                scan_free_expr(expr, functions, bound, free, seen);
            }
        }
        LuaStmt::ExprStmt(expr) => scan_free_expr(expr, functions, bound, free, seen),
        LuaStmt::Break | LuaStmt::FunctionDef { .. } => {}
        LuaStmt::Block(statements) => scan_free_block(statements, functions, &mut bound.clone(), free, seen),
    }
}

fn scan_free_expr(
    expr: &LuaExpr,
    functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
    bound: &BTreeSet<String>,
    free: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    match expr {
        LuaExpr::Number(_) | LuaExpr::String(_) | LuaExpr::Bool(_) | LuaExpr::Nil => {}
        LuaExpr::Ident(name) => note_free(free, seen, bound, name),
        LuaExpr::Table { fields } => {
            for field in fields {
                use crate::script::LuaTableField;
                match field {
                    LuaTableField::Array(value) => scan_free_expr(value, functions, bound, free, seen),
                    LuaTableField::Record { value, .. } => scan_free_expr(value, functions, bound, free, seen),
                    LuaTableField::Indexed { key, value } => {
                        scan_free_expr(key, functions, bound, free, seen);
                        scan_free_expr(value, functions, bound, free, seen);
                    }
                }
            }
        }
        LuaExpr::Index { table, key } => {
            scan_free_expr(table, functions, bound, free, seen);
            scan_free_expr(key, functions, bound, free, seen);
        }
        LuaExpr::Field { table, .. } => scan_free_expr(table, functions, bound, free, seen),
        LuaExpr::Binary { left, right, .. } => {
            scan_free_expr(left, functions, bound, free, seen);
            scan_free_expr(right, functions, bound, free, seen);
        }
        LuaExpr::Unary { operand, .. } => scan_free_expr(operand, functions, bound, free, seen),
        // 被调用名不是值读取：残化按函数表/`print` 分派，未知调用由残化本身拒绝。
        LuaExpr::Call { args, .. } => {
            for arg in args {
                scan_free_expr(arg, functions, bound, free, seen);
            }
        }
    }
}

fn residualize_block(
    statements: &[LuaStmt],
    sink: &mut dyn ResidualSink,
    functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
    break_label: Option<&str>,
) -> Result<(), String> {
    for stmt in statements {
        residualize_stmt(stmt, sink, functions, break_label)?;
    }
    Ok(())
}

fn residualize_stmt(
    stmt: &LuaStmt,
    sink: &mut dyn ResidualSink,
    functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
    break_label: Option<&str>,
) -> Result<(), String> {
    match stmt {
        LuaStmt::LocalDecl { names, values } => {
            for (index, name) in names.iter().enumerate() {
                if let Some(expr) = values.get(index) {
                    residualize_expr(expr, sink, functions)?;
                }
                else {
                    sink.push_nil();
                }
                sink.store(name);
            }
            Ok(())
        }
        LuaStmt::Assign { targets, values } => {
            let mut evaluated = Vec::with_capacity(values.len());
            for expr in values {
                residualize_expr(expr, sink, functions)?;
                // stash into temps so multi-assign left-to-right matches interpret
                let temp = format!("__pe_assign_{}", evaluated.len());
                sink.store(&temp);
                evaluated.push(temp);
            }
            for (index, target) in targets.iter().enumerate() {
                if let Some(temp) = evaluated.get(index) {
                    sink.load(temp);
                }
                else {
                    sink.push_nil();
                }
                residualize_assign(target, sink, functions)?;
            }
            Ok(())
        }
        LuaStmt::If { condition, then_block, else_block } => {
            let else_l = unique_label("else");
            let end_l = unique_label("endif");
            residualize_expr(condition, sink, functions)?;
            sink.jump_if_false(&else_l);
            residualize_block(then_block, sink, functions, break_label)?;
            sink.jump(&end_l);
            sink.label(&else_l);
            residualize_block(else_block, sink, functions, break_label)?;
            sink.label(&end_l);
            Ok(())
        }
        LuaStmt::While { condition, body } => {
            let head = unique_label("while_head");
            let end = unique_label("while_end");
            sink.label(&head);
            residualize_expr(condition, sink, functions)?;
            sink.jump_if_false(&end);
            residualize_block(body, sink, functions, Some(&end))?;
            sink.jump(&head);
            sink.label(&end);
            Ok(())
        }
        LuaStmt::Repeat { body, condition } => {
            let head = unique_label("repeat_head");
            let end = unique_label("repeat_end");
            sink.label(&head);
            residualize_block(body, sink, functions, Some(&end))?;
            residualize_expr(condition, sink, functions)?;
            sink.jump_if_false(&head);
            sink.label(&end);
            Ok(())
        }
        LuaStmt::ForNumeric { name, start, limit, step, body } => {
            let head = unique_label("for_head");
            let end = unique_label("for_end");
            let limit_slot = unique_label("for_limit");
            let step_slot = unique_label("for_step");
            residualize_expr(start, sink, functions)?;
            sink.store(name);
            residualize_expr(limit, sink, functions)?;
            sink.store(&limit_slot);
            if let Some(step_expr) = step {
                residualize_expr(step_expr, sink, functions)?;
            }
            else {
                sink.push_i64(1);
            }
            sink.store(&step_slot);
            sink.label(&head);
            // continue when (step>0 && i<=limit) || (step<0 && i>=limit)
            sink.load(&step_slot);
            sink.push_i64(0);
            sink.gt();
            let pos = unique_label("for_pos");
            let neg = unique_label("for_neg");
            let cont = unique_label("for_cont");
            sink.jump_if_true(&pos);
            sink.jump(&neg);
            sink.label(&pos);
            sink.load(name);
            sink.load(&limit_slot);
            sink.le();
            sink.jump_if_false(&end);
            sink.jump(&cont);
            sink.label(&neg);
            sink.load(name);
            sink.load(&limit_slot);
            sink.ge();
            sink.jump_if_false(&end);
            sink.label(&cont);
            residualize_block(body, sink, functions, Some(&end))?;
            sink.load(name);
            sink.load(&step_slot);
            sink.add();
            sink.store(name);
            sink.jump(&head);
            sink.label(&end);
            Ok(())
        }
        LuaStmt::Break => {
            let Some(label) = break_label
            else {
                return sink.unsupported("break outside loop");
            };
            sink.jump(label);
            Ok(())
        }
        LuaStmt::Return(value) => {
            if let Some(expr) = value {
                residualize_expr(expr, sink, functions)?;
            }
            else {
                sink.push_nil();
            }
            sink.store("__pe_last");
            sink.load("__pe_last");
            sink.ret();
            Ok(())
        }
        LuaStmt::ExprStmt(expr) => {
            residualize_expr(expr, sink, functions)?;
            // keep last expr result in `__pe_last` for main fallthrough (interpret returns it)
            sink.store("__pe_last");
            Ok(())
        }
        LuaStmt::FunctionDef { .. } => Ok(()), // already hoisted
        LuaStmt::Block(statements) => residualize_block(statements, sink, functions, break_label),
    }
}

fn residualize_assign(
    target: &LuaLValue,
    sink: &mut dyn ResidualSink,
    _functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
) -> Result<(), String> {
    match target {
        LuaLValue::Name(name) => {
            sink.store(name);
            Ok(())
        }
        LuaLValue::Field { .. } | LuaLValue::Index { .. } => sink.unsupported("table assignment"),
    }
}

fn residualize_expr(
    expr: &LuaExpr,
    sink: &mut dyn ResidualSink,
    functions: &HashMap<String, (Vec<String>, Vec<LuaStmt>)>,
) -> Result<(), String> {
    match expr {
        LuaExpr::Number(value) => {
            if value.fract() == 0.0 && value.is_finite() {
                sink.push_i64(*value as i64);
            }
            else {
                return sink.unsupported("float literal");
            }
            Ok(())
        }
        LuaExpr::String(value) => {
            sink.push_string(value);
            Ok(())
        }
        LuaExpr::Bool(value) => {
            sink.push_bool(*value);
            Ok(())
        }
        LuaExpr::Nil => {
            sink.push_nil();
            Ok(())
        }
        LuaExpr::Ident(name) => {
            sink.load(name);
            Ok(())
        }
        LuaExpr::Table { .. } | LuaExpr::Index { .. } | LuaExpr::Field { .. } => sink.unsupported("tables"),
        LuaExpr::Binary { op, left, right } => match op.as_str() {
            // Lua 的 `and`/`or` 返回**操作数**，不是布尔值（`7 or 9 == 7`）。
            // 残差必须保留左侧值：用临时槽保存并只在短路分支重新载入，
            // 不得把结果塌缩成 `true`/`false`。
            "and" => {
                let temp = unique_label("and_operand");
                let false_l = unique_label("and_false");
                let end_l = unique_label("and_end");
                residualize_expr(left, sink, functions)?;
                sink.store(&temp);
                sink.load(&temp);
                sink.jump_if_false(&false_l);
                residualize_expr(right, sink, functions)?;
                sink.jump(&end_l);
                sink.label(&false_l);
                sink.load(&temp);
                sink.label(&end_l);
                Ok(())
            }
            "or" => {
                let temp = unique_label("or_operand");
                let true_l = unique_label("or_true");
                let end_l = unique_label("or_end");
                residualize_expr(left, sink, functions)?;
                sink.store(&temp);
                sink.load(&temp);
                sink.jump_if_true(&true_l);
                residualize_expr(right, sink, functions)?;
                sink.jump(&end_l);
                sink.label(&true_l);
                sink.load(&temp);
                sink.label(&end_l);
                Ok(())
            }
            _ => {
                residualize_expr(left, sink, functions)?;
                residualize_expr(right, sink, functions)?;
                match op.as_str() {
                    "+" => sink.add(),
                    "-" => sink.sub(),
                    "*" => sink.mul(),
                    "/" => sink.div(),
                    "%" => sink.rem(),
                    ".." => sink.concat(),
                    "==" => sink.eq(),
                    "~=" => sink.ne(),
                    "<" => sink.lt(),
                    "<=" => sink.le(),
                    ">" => sink.gt(),
                    ">=" => sink.ge(),
                    other => return sink.unsupported(&format!("binary op '{other}'")),
                }
                Ok(())
            }
        },
        LuaExpr::Unary { op, operand } => {
            residualize_expr(operand, sink, functions)?;
            match op.as_str() {
                "not" => {
                    sink.not();
                    Ok(())
                }
                "-" => {
                    // operand already on stack → 0 - operand
                    sink.store("__pe_neg");
                    sink.push_i64(0);
                    sink.load("__pe_neg");
                    sink.sub();
                    Ok(())
                }
                "#" => sink.unsupported("length operator"),
                other => sink.unsupported(&format!("unary op '{other}'")),
            }
        }
        LuaExpr::Call { name, args } => {
            for arg in args {
                residualize_expr(arg, sink, functions)?;
            }
            if name == "print" {
                sink.print(args.len());
                sink.store("__pe_last");
                sink.load("__pe_last");
                return Ok(());
            }
            if functions.contains_key(name) {
                sink.call(name, args.len());
                return Ok(());
            }
            sink.unsupported(&format!("call to '{name}'"))
        }
    }
}

fn unique_label(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("__{prefix}_{id}")
}
