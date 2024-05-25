use rusty_c::*;
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
fn test_recursion_protection() {
    let mut pp = Preprocessor::new();
    pp.process("#define A B\n#define B A", Path::new(".")).unwrap();
    let result = pp.process("A", Path::new(".")).unwrap();
    // Should not infinite loop, A expands to B, B expands to A, but A is now protected
    assert_eq!(result, "A\n");
}

#[test]
fn test_arithmetic_if() {
    let mut pp = Preprocessor::new();
    pp.define("X", "10");
    pp.define("Y", "20");
    let source = "#if X + 10 == Y\nint matched = 1;\n#endif";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert_eq!(result, "int matched = 1;\n");
}

#[test]
fn test_comments() {
    let mut pp = Preprocessor::new();
    let source = "// line comment\n/* block\n   comment */\nint x = 1;";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert_eq!(result, "\n\nint x = 1;\n");
}

#[test]
fn test_line_directive() {
    let mut pp = Preprocessor::new();
    let source = "#line 100 \"test.c\"\nint line = __LINE__;";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert!(result.contains("int line = 100;"));
}

#[test]
fn test_elifdef() {
    let mut pp = Preprocessor::new();
    pp.define("A", "1");
    let source = "#ifdef B\nint x = 0;\n#elifdef A\nint x = 1;\n#endif";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert!(result.contains("int x = 1;"));
}

#[test]
fn test_has_include() {
    let mut pp = Preprocessor::new();
    let source = "#if __has_include(\"stdio.h\")\n#define HAS_STDIO 1\n#endif";
    // stdio.h doesn't exist in current dir, so it should be false
    let result = pp.process(source, Path::new(".")).unwrap();
    assert!(!result.contains("#define HAS_STDIO 1"));
}

#[test]
fn test_va_opt() {
    let mut pp = Preprocessor::new();
    pp.process("#define LOG(fmt, ...) printf(fmt __VA_OPT__(,) __VA_ARGS__)", Path::new(".")).unwrap();
    let result1 = pp.process("LOG(\"hello\");", Path::new(".")).unwrap();
    assert_eq!(result1, "printf(\"hello\");\n");
    let result2 = pp.process("LOG(\"num: %d\", 42);", Path::new(".")).unwrap();
    assert_eq!(result2, "printf(\"num: %d\", 42);\n");
}

#[test]
fn test_va_args_comma_elision() {
    let mut pp = Preprocessor::new();
    pp.process("#define LOG(fmt, ...) printf(fmt, ##__VA_ARGS__)", Path::new(".")).unwrap();
    let result1 = pp.process("LOG(\"hello\");", Path::new(".")).unwrap();
    assert_eq!(result1, "printf(\"hello\");\n");
}

#[test]
fn test_full_arithmetic() {
    let mut pp = Preprocessor::new();
    pp.define("X", "10");
    let source = "#if X * 2 == 20\nint ok = 1;\n#endif";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert!(result.contains("int ok = 1;"));
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
    // B is not defined
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

#[test]
fn test_embed() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.bin");
    fs::write(&file_path, vec![1, 2, 3]).unwrap();

    let mut pp = Preprocessor::new();
    let source = format!("#embed \"{}\" prefix(pre ) suffix( post)", file_path.to_str().unwrap());
    let result = pp.process(&source, Path::new(".")).unwrap();
    assert_eq!(result, "pre 1, 2, 3 post\n");
}

#[test]
fn test_has_c_attribute() {
    let mut pp = Preprocessor::new();
    let source = "#if __has_c_attribute(nodiscard)\nint x = 1;\n#endif";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert_eq!(result, "int x = 1;\n");
}

#[test]
fn test_bitwise_if() {
    let mut pp = Preprocessor::new();
    let source = "#if (0x0F & 0xF0) == 0 && (1 << 3) == 8\nint ok = 1;\n#endif";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert_eq!(result, "int ok = 1;\n");
}

#[test]
fn test_has_builtin() {
    let mut pp = Preprocessor::new();
    let source = "#if __has_builtin(__builtin_expect)\nint ok = 1;\n#endif";
    let result = pp.process(source, Path::new(".")).unwrap();
    assert_eq!(result, "int ok = 1;\n");
}

#[test]
fn test_c23_integrated() {
    let mut pp = Preprocessor::new();
    let source = r#"
#define VERSION 202311L
#if __STDC_VERSION__ >= VERSION
    #elifdef UNDEFINED
        #error "Should not happen"
    #elifndef DEFINED
        #if __has_c_attribute(nodiscard) && __has_builtin(__builtin_memcpy)
            int c23_ok = 1;
        #endif
#endif
"#;
    let result = pp.process(source, Path::new(".")).unwrap();
    assert!(result.contains("int c23_ok = 1;"));
}
