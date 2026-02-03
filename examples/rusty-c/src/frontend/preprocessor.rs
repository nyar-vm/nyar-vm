use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

pub struct Preprocessor {
    macros: HashMap<String, MacroDef>,
    include_paths: Vec<PathBuf>,
    included_files: Vec<PathBuf>,
    current_file: PathBuf,
    current_line: usize,
}

#[derive(Clone)]
enum MacroDef {
    Simple(String),
    Function {
        params: Vec<String>,
        has_varargs: bool,
        body: String,
    },
}

impl Preprocessor {
    pub fn new() -> Self {
        let mut macros = HashMap::new();
        // Predefined macros
        macros.insert("__RUSTY_C__".to_string(), MacroDef::Simple("1".to_string()));
        
        Self {
            macros,
            include_paths: Vec::new(),
            included_files: Vec::new(),
            current_file: PathBuf::from("<stdin>"),
            current_line: 0,
        }
    }

    pub fn add_include_path<P: AsRef<Path>>(&mut self, path: P) {
        self.include_paths.push(path.as_ref().to_path_buf());
    }

    pub fn define<S: Into<String>, V: Into<String>>(&mut self, name: S, value: V) {
        self.macros.insert(name.into(), MacroDef::Simple(value.into()));
    }

    pub fn process(&mut self, source: &str, current_dir: &Path) -> Result<String, String> {
        let mut output = String::new();
        let mut lines = source.lines().enumerate();
        let mut skip_stack = Vec::new();

        while let Some((line_idx, mut line)) = lines.next() {
            self.current_line = line_idx + 1;
            
            // Handle line continuation
            let mut full_line = line.to_string();
            while full_line.ends_with('\\') {
                full_line.pop();
                if let Some((_, next_line)) = lines.next() {
                    full_line.push_str(next_line);
                    self.current_line += 1;
                } else {
                    break;
                }
            }
            line = &full_line;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                output.push('\n');
                continue;
            }

            // Handle directives
            if trimmed.starts_with('#') {
                let directive_line = trimmed[1..].trim();
                
                if directive_line.starts_with("ifdef") {
                    let name = directive_line[5..].trim();
                    skip_stack.push(!self.macros.contains_key(name));
                    continue;
                } else if directive_line.starts_with("ifndef") {
                    let name = directive_line[6..].trim();
                    skip_stack.push(self.macros.contains_key(name));
                    continue;
                } else if directive_line.starts_with("if") {
                    // Basic support for #if - currently just checks if macro exists or is non-zero
                    let expr = directive_line[2..].trim();
                    let val = self.evaluate_condition(expr);
                    skip_stack.push(!val);
                    continue;
                } else if directive_line.starts_with("elif") {
                    if let Some(skip) = skip_stack.pop() {
                        if skip {
                            let expr = directive_line[4..].trim();
                            let val = self.evaluate_condition(expr);
                            skip_stack.push(!val);
                        } else {
                            skip_stack.push(true); // Already executed a branch
                        }
                    } else {
                        return Err(format!("Line {}: Unexpected #elif", self.current_line));
                    }
                    continue;
                } else if directive_line == "else" {
                    if let Some(skip) = skip_stack.pop() {
                        skip_stack.push(!skip);
                    } else {
                        return Err(format!("Line {}: Unexpected #else", self.current_line));
                    }
                    continue;
                } else if directive_line == "endif" {
                    if skip_stack.pop().is_none() {
                        return Err(format!("Line {}: Unexpected #endif", self.current_line));
                    }
                    continue;
                }

                // Skip other directives if in a false conditional branch
                if skip_stack.iter().any(|&skip| skip) {
                    continue;
                }

                if directive_line.starts_with("define") {
                    let rest = directive_line[6..].trim();
                    if let Some(paren_idx) = rest.find('(') {
                        // Function-like macro
                        let name = rest[..paren_idx].trim().to_string();
                        if let Some(close_paren_idx) = rest.find(')') {
                            let params_str = &rest[paren_idx + 1..close_paren_idx];
                            let mut params: Vec<String> = params_str.split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            
                            let mut has_varargs = false;
                            if let Some(last) = params.last() {
                                if last == "..." {
                                    has_varargs = true;
                                    params.pop();
                                }
                            }
                            
                            let body = rest[close_paren_idx + 1..].trim().to_string();
                            self.macros.insert(name, MacroDef::Function { params, has_varargs, body });
                        } else {
                            return Err(format!("Line {}: Invalid function-like macro definition", self.current_line));
                        }
                    } else {
                        // Simple macro
                        let parts: Vec<&str> = rest.splitn(2, |c: char| c.is_whitespace()).collect();
                        if parts.is_empty() {
                            return Err(format!("Line {}: Invalid #define", self.current_line));
                        }
                        let name = parts[0].to_string();
                        let value = if parts.len() > 1 { parts[1].trim().to_string() } else { "1".to_string() };
                        self.macros.insert(name, MacroDef::Simple(value));
                    }
                } else if directive_line.starts_with("undef") {
                    let name = directive_line[5..].trim();
                    self.macros.remove(name);
                } else if directive_line.starts_with("include") {
                    let include_spec = directive_line[7..].trim();
                    if include_spec.starts_with('"') && include_spec.ends_with('"') {
                        let file_name = &include_spec[1..include_spec.len() - 1];
                        let included_content = self.resolve_include(file_name, current_dir, true)?;
                        output.push_str(&included_content);
                    } else if include_spec.starts_with('<') && include_spec.ends_with('>') {
                        let file_name = &include_spec[1..include_spec.len() - 1];
                        let included_content = self.resolve_include(file_name, current_dir, false)?;
                        output.push_str(&included_content);
                    } else {
                        return Err(format!("Line {}: Unsupported include format: {}", self.current_line, include_spec));
                    }
                } else if directive_line.starts_with("error") {
                    let msg = directive_line[5..].trim();
                    return Err(format!("Line {}: #error: {}", self.current_line, msg));
                } else if directive_line == "pragma once" {
                    // Handled in resolve_include
                }
                continue;
            }

            // Skip regular lines if in a false conditional branch
            if skip_stack.iter().any(|&skip| skip) {
                continue;
            }

            // Regular line, perform macro expansion
            let expanded_line = self.expand_macros_in_line(line);
            output.push_str(&expanded_line);
            output.push('\n');
        }

        if !skip_stack.is_empty() {
            return Err(format!("File {:?}: Missing #endif", self.current_file));
        }

        Ok(output)
    }

    fn evaluate_condition(&self, expr: &str) -> bool {
        let trimmed = expr.trim();
        if trimmed.is_empty() { return false; }

        // Handle logical OR
        if trimmed.contains("||") {
            return trimmed.split("||").any(|part| self.evaluate_condition(part));
        }
        // Handle logical AND
        if trimmed.contains("&&") {
            return trimmed.split("&&").all(|part| self.evaluate_condition(part));
        }
        // Handle logical NOT
        if trimmed.starts_with('!') {
            return !self.evaluate_condition(&trimmed[1..]);
        }

        // Handle defined(NAME) or defined NAME
        if trimmed.starts_with("defined") {
            let rest = trimmed[7..].trim();
            let name = if rest.starts_with('(') && rest.ends_with(')') {
                &rest[1..rest.len()-1]
            } else {
                rest
            };
            return self.macros.contains_key(name.trim());
        }

        // Try to evaluate as a macro or number
        let expanded = self.expand_macros_in_line(trimmed);
        let val = expanded.trim();
        
        if val == "0" || val == "false" || val.is_empty() {
            false
        } else if val == "1" || val == "true" {
            true
        } else {
            // Check if it's a numeric literal
            val.parse::<i64>().map(|v| v != 0).unwrap_or(false)
        }
    }

    fn resolve_include(&mut self, file_name: &str, current_dir: &Path, search_current: bool) -> Result<String, String> {
        let mut paths_to_check = Vec::new();
        if search_current {
            paths_to_check.push(current_dir.to_path_buf());
        }
        paths_to_check.extend(self.include_paths.clone());

        for path in paths_to_check {
            let full_path = path.join(file_name);
            if full_path.exists() {
                // Handle #pragma once
                if self.included_files.contains(&full_path) {
                    return Ok(String::new());
                }
                
                let content = fs::read_to_string(&full_path)
                    .map_err(|e| format!("Failed to read included file {:?}: {}", full_path, e))?;
                
                if content.contains("#pragma once") {
                    self.included_files.push(full_path.clone());
                }

                // Recursively process the included file
                let result = self.process(&content, full_path.parent().unwrap_or(Path::new(".")))?;
                
                return Ok(result);
            }
        }

        Err(format!("Could not find included file: {}", file_name))
    }

    fn expand_macros_in_line(&self, line: &str) -> String {
        let mut result = String::new();
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '"' {
                // String literal - don't expand macros inside
                result.push(c);
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    result.push(nc);
                    if nc == '"' {
                        break;
                    }
                    if nc == '\\' {
                        if let Some(esc) = chars.next() {
                            result.push(esc);
                        }
                    }
                }
            } else if c.is_alphabetic() || c == '_' {
                // Potential macro name
                let mut name = String::new();
                name.push(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        name.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                
                if let Some(def) = self.macros.get(&name) {
                    match def {
                        MacroDef::Simple(value) => {
                            result.push_str(value);
                        }
                        _ => { /* Handled below */ }
                    }
                } else {
                    // Handle special predefined macros
                    match name.as_str() {
                        "__FILE__" => {
                            result.push_str(&format!("\"{}\"", self.current_file.to_string_lossy().replace("\\", "\\\\")));
                            continue;
                        }
                        "__LINE__" => {
                            result.push_str(&self.current_line.to_string());
                            continue;
                        }
                        "__DATE__" => {
                            result.push_str("\"Feb  3 2026\""); // Today's date
                            continue;
                        }
                        "__TIME__" => {
                            result.push_str("\"12:00:00\""); // Placeholder
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(def) = self.macros.get(&name) {
                    match def {
                        MacroDef::Simple(_) => { /* Already handled */ }
                        MacroDef::Function { params, has_varargs, body } => {
                            // Try to parse arguments
                            if let Some(&'(') = chars.peek() {
                                chars.next(); // skip '('
                                let mut args = Vec::new();
                                let mut current_arg = String::new();
                                let mut paren_depth = 0;
                                while let Some(ac) = chars.next() {
                                    if ac == '(' { paren_depth += 1; }
                                    if ac == ')' {
                                        if paren_depth == 0 {
                                            args.push(current_arg.trim().to_string());
                                            break;
                                        } else {
                                            paren_depth -= 1;
                                        }
                                    }
                                    if ac == ',' && paren_depth == 0 {
                                        args.push(current_arg.trim().to_string());
                                        current_arg.clear();
                                    } else {
                                        current_arg.push(ac);
                                    }
                                }
                                
                                let mut expanded_body = body.clone();
                                
                                // Handle __VA_ARGS__
                                if *has_varargs {
                                    let va_args = if args.len() >= params.len() {
                                        args[params.len()..].join(", ")
                                    } else {
                                        String::new()
                                    };
                                    expanded_body = expanded_body.replace("__VA_ARGS__", &va_args);
                                }

                                // Handle stringification (#param)
                                for (p, a) in params.iter().zip(args.iter()) {
                                    let pattern = format!("#{}", p);
                                    let stringified = format!("\"{}\"", a.replace("\"", "\\\""));
                                    expanded_body = expanded_body.replace(&pattern, &stringified);
                                }

                                // Handle concatenation (##)
                                let mut final_body = String::new();
                                let mut i = 0;
                                let body_chars: Vec<char> = expanded_body.chars().collect();
                                while i < body_chars.len() {
                                    if i + 1 < body_chars.len() && body_chars[i] == '#' && body_chars[i+1] == '#' {
                                        // Skip ##
                                        i += 2;
                                        // Skip whitespace after ##
                                        while i < body_chars.len() && body_chars[i].is_whitespace() {
                                            i += 1;
                                        }
                                        // Remove trailing whitespace from final_body
                                        while let Some(c) = final_body.pop() {
                                            if !c.is_whitespace() {
                                                final_body.push(c);
                                                break;
                                            }
                                        }
                                    } else {
                                        final_body.push(body_chars[i]);
                                        i += 1;
                                    }
                                }
                                expanded_body = final_body;

                                for (p, a) in params.iter().zip(args.iter()) {
                                    // Simple replacement for arguments
                                    expanded_body = expanded_body.replace(p, a);
                                }
                                
                                // Recursively expand the result
                                let final_expansion = self.expand_macros_in_line(&expanded_body);
                                result.push_str(&final_expansion);
                            } else {
                                result.push_str(&name);
                            }
                        }
                    }
                } else {
                    result.push_str(&name);
                }
            } else {
                result.push(c);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_simple_macro() {
        let mut pp = Preprocessor::new();
        pp.define("PI", "3.14");
        let result = pp.process("float area = PI * r * r;", Path::new(".")).unwrap();
        assert_eq!(result, "float area = 3.14 * r * r;\n");
    }

    #[test]
    fn test_function_macro() {
        let mut pp = Preprocessor::new();
        pp.process("#define SQUARE(x) ((x)*(x))", Path::new(".")).unwrap();
        let result = pp.process("int y = SQUARE(5);", Path::new(".")).unwrap();
        assert_eq!(result, "int y = ((5)*(5));\n");
    }

    #[test]
    fn test_ifdef() {
        let mut pp = Preprocessor::new();
        pp.define("DEBUG", "1");
        let source = "#ifdef DEBUG\nint x = 1;\n#else\nint x = 0;\n#endif";
        let result = pp.process(source, Path::new(".")).unwrap();
        assert!(result.contains("int x = 1;"));
        assert!(!result.contains("int x = 0;"));
    }

    #[test]
    fn test_va_args() {
        let mut pp = Preprocessor::new();
        pp.process("#define LOG(fmt, ...) printf(fmt, __VA_ARGS__)", Path::new(".")).unwrap();
        let result = pp.process("LOG(\"%d %d\", 1, 2);", Path::new(".")).unwrap();
        assert_eq!(result, "printf(\"%d %d\", 1, 2);\n");
    }

    #[test]
    fn test_stringification() {
        let mut pp = Preprocessor::new();
        pp.process("#define STR(x) #x", Path::new(".")).unwrap();
        let result = pp.process("STR(hello);", Path::new(".")).unwrap();
        assert_eq!(result, "\"hello\";\n");
    }

    #[test]
    fn test_predefined_macros() {
        let mut pp = Preprocessor::new();
        let result = pp.process("int line = __LINE__;", Path::new(".")).unwrap();
        assert_eq!(result, "int line = 1;\n");
    }

    #[test]
    fn test_line_continuation() {
        let mut pp = Preprocessor::new();
        pp.process("#define MULTI \\\nline \\\nmacro", Path::new(".")).unwrap();
        let result = pp.process("MULTI", Path::new(".")).unwrap();
        assert_eq!(result, "line macro\n");
    }

    #[test]
    fn test_concatenation() {
        let mut pp = Preprocessor::new();
        pp.process("#define CONCAT(a, b) a ## b", Path::new(".")).unwrap();
        let result = pp.process("int CONCAT(x, 1) = 42;", Path::new(".")).unwrap();
        assert_eq!(result, "int x1 = 42;\n");
    }

    #[test]
    fn test_if_defined() {
        let mut pp = Preprocessor::new();
        pp.define("VERSION", "2");
        let source = "#if defined(VERSION)\nint v = VERSION;\n#endif";
        let result = pp.process(source, Path::new(".")).unwrap();
        assert_eq!(result, "int v = 2;\n");
    }

    #[test]
    fn test_if_logic() {
        let mut pp = Preprocessor::new();
        pp.define("A", "1");
        pp.define("B", "0");
        let source = "#if defined(A) && !defined(B)\nint x = 1;\n#endif";
        let result = pp.process(source, Path::new(".")).unwrap();
        assert_eq!(result, "int x = 1;\n");
    }

    #[test]
    fn test_include() {
        let dir = tempdir().unwrap();
        let header_path = dir.path().join("test.h");
        let mut header_file = fs::File::create(&header_path).unwrap();
        writeln!(header_file, "#define VAL 42").unwrap();

        let mut pp = Preprocessor::new();
        let source = "#include \"test.h\"\nint x = VAL;";
        let result = pp.process(source, dir.path()).unwrap();
        assert!(result.contains("int x = 42;"));
    }
}

