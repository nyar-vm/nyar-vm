//! `boot.js` island 生命周期真实执行测试。
//!
//! 用 Node 执行真实生成的 `boot.js`：最小 DOM 仿真提供节点、属性、事件与句柄表，
//! wasm 导出用固定桩函数代替（只验证宿主侧挂载/更新/解绑逻辑，不代表 wasm 语义）。
//! 断言覆盖 mount → input 事件 → 信号 → DOM 更新 → unmount 解绑 → 重挂载 →
//! `pagehide` 页面卸载 dispose（解绑监听/订阅、放下 wasm 实例引用、幂等），
//! 不允许用“产物里出现 `unmountIslands` 字符串”冒充生命周期正确。

use std::process::Command;

use rusty_valkyrie::asgard::codegen::{generate_boot_script, manifest_url_for_mode};
use tempfile::TempDir;

/// Node 探针脚本；与测试同目录的 `support/` 下维护。
const PROBE: &str = include_str!("support/boot_lifecycle_probe.mjs");

#[test]
fn boot_js_mount_update_unmount_and_remount_leave_no_bindings() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let boot_path = temp.path().join("boot.js");
    std::fs::write(&boot_path, generate_boot_script("probe", manifest_url_for_mode(true))).expect("写入 boot.js 失败");
    let runner_path = temp.path().join("boot_lifecycle_probe.mjs");
    std::fs::write(&runner_path, PROBE).expect("写入 Node 探针失败");

    let output = Command::new("node").arg(&runner_path).arg(&boot_path).output().expect("Node 必须可用（Asgard 浏览器产物宿主）");
    assert!(
        output.status.success(),
        "boot.js 生命周期探针失败 status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("BOOT_LIFECYCLE_OK"), "探针必须完整执行到生命周期断言终点");
}
