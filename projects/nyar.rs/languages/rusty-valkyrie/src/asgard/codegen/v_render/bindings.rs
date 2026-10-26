use std::fmt::Write as _;

use crate::asgard::awsl::{BindingKind, ScriptBinding, SignalValueType, expr_deps::reactive_deps};

use super::{aux_exports::AuxExports, expr::rewrite_utf8_expr_body, util::sanitize_export};

pub(crate) fn emit_memoized_bindings(bindings: &[ScriptBinding], body: &mut String, route: &str, aux: &mut AuxExports) {
    let mut memo_id = 0i32;
    for binding in bindings {
        if binding.kind != BindingKind::Memoized {
            continue;
        }
        memo_id += 1;
        let export = format!("awsl_memo_{}_{}", sanitize_export(route), sanitize_export(&binding.name));
        aux.push(format!(
            "micro {export}(): {} {{\n    return {}\n}}",
            match binding.value_type {
                SignalValueType::I32 => "i32",
                SignalValueType::Bool => "bool",
                SignalValueType::Utf8 => "utf8",
            },
            rewrite_utf8_expr_body(&binding.init_expr, bindings, &[])
        ));
        let deps = reactive_deps(&binding.init_expr, bindings);
        let dep1 =
            deps.first().and_then(|name| bindings.iter().find(|b| b.name == *name)).map(|b| b.sig_id_expr()).unwrap_or_else(|| "-1".into());
        let dep2 =
            deps.get(1).and_then(|name| bindings.iter().find(|b| b.name == *name)).map(|b| b.sig_id_expr()).unwrap_or_else(|| "-1".into());
        let call = match binding.value_type {
            SignalValueType::I32 => format!("rx_memo_i32({memo_id}, \"{export}\", {dep1}, {dep2})"),
            SignalValueType::Utf8 => format!("rx_memo_utf8({memo_id}, \"{export}\", {dep1}, {dep2})"),
            SignalValueType::Bool => format!("rx_memo_utf8({memo_id}, \"{export}\", {dep1}, {dep2})"),
        };
        writeln!(body, "    let {} = {call}", binding.name).unwrap();
    }
}
