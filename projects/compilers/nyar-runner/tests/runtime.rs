use std::path::Path;

use nyar_runner::runtime::{RuntimeContract, RuntimeFamily};

#[test]
fn jvm_defaults_to_classpath_without_contract() {
    let template = RuntimeFamily::Jvm.default_template(None);
    assert_eq!(template.command, "java");
    assert_eq!(template.args, vec!["-cp".to_string(), "{classpath}".to_string(), "{entry}".to_string()]);
}

#[test]
fn jvm_uses_jar_mode_with_contract() {
    let template =
        RuntimeFamily::Jvm.default_template(Some(RuntimeContract { logical_entry: Some("main"), physical_entry: Some("app"), wasi_p3: false }));
    assert_eq!(template.command, "java");
    assert_eq!(template.args, vec!["-jar".to_string(), "{artifact}".to_string()]);
}

#[test]
fn wasi_defaults_to_component_command_mode() {
    let invoke =
        RuntimeFamily::Wasi.default_template(Some(RuntimeContract { logical_entry: Some("main"), physical_entry: None, wasi_p3: false }));
    assert_eq!(
        invoke.args,
        vec![
            "run".to_string(),
            "-W".to_string(),
            "gc".to_string(),
            "-W".to_string(),
            "wmemcheck".to_string(),
            "-W".to_string(),
            "max-memory-size=16777216".to_string(),
            "{artifact}".to_string(),
        ]
    );

    let start =
        RuntimeFamily::Wasi.default_template(Some(RuntimeContract { logical_entry: Some("_start"), physical_entry: None, wasi_p3: false }));
    assert_eq!(
        start.args,
        vec![
            "run".to_string(),
            "-W".to_string(),
            "gc".to_string(),
            "-W".to_string(),
            "wmemcheck".to_string(),
            "-W".to_string(),
            "max-memory-size=16777216".to_string(),
            "{artifact}".to_string(),
        ]
    );

    let p3 = RuntimeFamily::Wasi.default_template(Some(RuntimeContract { logical_entry: Some("_start"), physical_entry: None, wasi_p3: true }));
    assert!(p3.args.windows(2).any(|pair| pair == ["-S", "p3"]));
}

#[test]
fn expands_runtime_template_placeholders() {
    let template = RuntimeFamily::Clr.default_template(None);
    let command = template.prepare_command(Path::new(r"\\?\C:\tmp\app.exe"), Path::new("ignored"), "ignored");
    assert_eq!(command.command, "dotnet");
    assert_eq!(command.args, vec!["exec".to_string(), r"C:\tmp\app.exe".to_string()]);
}
