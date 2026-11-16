//! 真实 JVM 宿主冒烟：`JvmBinaryBackend` → `.class`/`.jar` → `java -jar`。
//!
//! 不经 bundled driver（JVM 家族当前冻结）；二进制检查走 `acorn-jvm`。

mod support;

use std::process::Command;

use nyar::{BinaryArch, BinaryFlavor, BinaryTarget, TargetFamily, backends::TargetCodeGenBackend};
use nyar_emitter::nyar_backend_jvm::{JvmBinaryBackend, JvmJarPackage, decode_instructions};
use tempfile::tempdir;

use crate::support::{compilation_options, demo_jvm_backend_input};

fn java_available() -> bool {
    Command::new("java").arg("-version").output().map(|output| output.status.success()).unwrap_or(false)
}

#[test]
fn rejects_dangling_frozen_launcher_before_writing() {
    let output_dir = tempdir().expect("temp dir");
    let mut input = demo_jvm_backend_input(output_dir.path());
    input.entry_launcher = Some("nyar/invoke/ghost".to_string());
    let backend = JvmBinaryBackend::new();
    let error = backend.validate(&input).expect_err("manifest 指向不存在的 launcher 必须拒绝，不得扫描补造");
    assert!(error.to_string().contains("不在主类或 companion 类中"), "{error}");
}

#[test]
fn emits_runnable_jar_verified_by_host_java() {
    if !java_available() {
        eprintln!("skip jvm host runtime: java not on PATH");
        return;
    }

    let output_dir = tempdir().expect("temp dir");
    let options = compilation_options(BinaryTarget::new(TargetFamily::Jvm, BinaryArch::Any, BinaryFlavor::ManagedClr), "demo");
    let input = demo_jvm_backend_input(output_dir.path());
    let backend = JvmBinaryBackend::new();
    backend.validate(&input).expect("validate ok");
    backend.compile(input, &options).expect("compile ok");

    let jar_path = output_dir.path().join("demo.jar");
    assert!(jar_path.is_file(), "missing {}", jar_path.display());

    let class_path = output_dir.path().join("demo").join("Main.class");
    assert!(class_path.is_file(), "class should land under package path {}", class_path.display());

    let jar_bytes = std::fs::read(&jar_path).expect("read jar");
    let package = JvmJarPackage::from_bytes("demo.jar", &jar_bytes).expect("decode jar");
    // `Main-Class` 只能指向 lowering 冻结的入口 launcher，不再是扫描补造的模块类。
    assert_eq!(package.main_class.as_deref(), Some("nyar.invoke.main"));
    package.read_class("nyar/invoke/main").expect("read class").expect("frozen launcher class present");
    let main_class = package.read_class("demo/Main").expect("read class").expect("demo/Main present");
    let main_index = main_class.methods.iter().position(|method| method.name == "main").expect("main method");
    let code = main_class.raw_method_code.get(main_index).and_then(|code| code.as_ref()).expect("main code");
    let decoded = decode_instructions(code, &main_class.constant_pool);
    assert!(!decoded.is_empty(), "spy-equivalent decode should see instructions");

    let run = Command::new("java").arg("-jar").arg(&jar_path).output().expect("java -jar");
    assert!(
        run.status.success(),
        "java -jar failed: status={:?}\nstdout={}\nstderr={}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
