use nyar_language::python::{PythonLintOptions, lint_python_source};

#[test]
fn flags_star_import() {
    let issues = lint_python_source("from os import *\n", &PythonLintOptions::default());
    assert_eq!(issues.len(), 1);
}
