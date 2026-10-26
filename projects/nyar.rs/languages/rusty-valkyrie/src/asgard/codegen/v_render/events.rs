use super::util::is_simple_identifier;

pub(crate) enum EventArgKind {
    Utf8(String),
    I32(String),
}

pub(crate) enum EventHandler {
    Simple(String),
    Call { fn_name: String, arg: String },
    Unknown,
}

pub(crate) fn strip_handler_wrapper(handler: &str) -> String {
    let mut handler = handler.trim().trim_matches('"').trim_matches('\'').to_string();
    if let Some(idx) = handler.find("=>") {
        let left = handler[..idx].trim();
        if left == "()" || left == "( )" {
            handler = handler[idx + 2..].trim().to_string();
        }
    }
    handler
}

pub(crate) fn parse_event_handler(handler: &str) -> EventHandler {
    let handler = handler.trim();
    if is_simple_identifier(handler) {
        return EventHandler::Simple(handler.to_string());
    }
    if let Some(open) = handler.find('(') {
        if handler.ends_with(')') {
            let fn_name = handler[..open].trim();
            let arg = handler[open + 1..handler.len() - 1].trim();
            if is_simple_identifier(fn_name) && !arg.is_empty() && !arg.contains(',') {
                return EventHandler::Call { fn_name: fn_name.to_string(), arg: arg.to_string() };
            }
        }
    }
    EventHandler::Unknown
}

pub(crate) fn split_field_access(expr: &str) -> Option<(String, String)> {
    let (head, tail) = expr.split_once('.')?;
    if is_simple_identifier(head) && is_simple_identifier(tail) {
        return Some((head.to_string(), tail.to_string()));
    }
    None
}
