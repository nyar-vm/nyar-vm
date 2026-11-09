//! `let` 绑定上下文推导：结构体字面量与泛型调用返回值的标注省略。
//!
//! 依赖链按真实布局分三组：`std`（`std.types` 命名空间持有 `Result`）→
//! `hardware`（设备域声明与泛型自由函数）→ `conformance`（消费者）。
//! 消费者分别省略结构体字面量与泛型调用返回值两处标注，验证绑定类型
//! 可以由初始化表达式推导，而不必依赖显式标注。

use crate::primitives::groups_with_primitives;
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler, hir::lowering::DEFAULT_COMPILE_ARCH};

const STD_TYPES_LIBRARY: &str = r#"
namespace std.types;

unite Result<T, E> {
    Fine {
        value: T,
    },
    Fail {
        error: E,
    },
}
"#;

const HARDWARE_LIBRARY: &str = r#"
namespace hardware;

structure HardwareFault {
    code: i64
}

class Device {
    _backend: i64
    _ordinal: i64
    _kind: i64
    _features: i64
    _lost: bool
}

micro hardware_request_device(available: bool) -> Result<Device, HardwareFault> {
    if available {
        return Fine {
            value: Device {
                _backend: 1,
                _ordinal: 0,
                _kind: 1,
                _features: 1,
                _lost: false,
            },
        }
    }
    return Fail { error: HardwareFault { code: 1006 } }
}

micro hardware_outcome_value_or<T>(outcome: Result<T, HardwareFault>, fallback: T) -> T {
    return match outcome {
        case Fine(value): value
        case Fail(_fault): fallback
    }
}

micro device_is_lost(device: Device) -> bool {
    return device._lost
}

[host_contract]
micro hardware_make_unspecified<T>() -> T
"#;

const STD_PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::bool")]
structure bool { }
"#;

fn compile_consumer(consumer: &str) -> Result<Vec<rusty_valkyrie::types::hir::HirModule>, rusty_valkyrie::HirError> {
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std", "std", STD_TYPES_LIBRARY),
        CompilerSourceGroup::single_source("hardware", "hardware", HARDWARE_LIBRARY).with_direct_dependencies(vec!["std".into()]),
        CompilerSourceGroup::single_source("conformance", "conformance", consumer)
            .as_entry()
            .with_direct_dependencies(vec!["std".into(), "hardware".into()]),
    ]);
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
}

/// 探针：带标注基线（用户现状）必须编译通过。
#[test]
fn probe_annotated_baseline() {
    let consumer = r#"
namespace hardware.conformance;

[export] micro c106_device_snapshot_frozen() -> i64 {
    let outcome: Result<hardware::Device, hardware::HardwareFault> = hardware::hardware_request_device(true)
    let fallback: hardware::Device = hardware::Device {
        _backend: 0,
        _ordinal: 0,
        _kind: 0,
        _features: 0,
        _lost: true,
    }
    let device: hardware::Device = hardware::hardware_outcome_value_or(outcome, fallback)
    if hardware::device_is_lost(device) {
        return 604
    }
    return 0
}
"#;
    compile_consumer(consumer).expect("带标注基线必须通过");
}

/// 探针：去掉两处标注后的真实诊断。
#[test]
fn probe_unannotated() {
    let consumer = r#"
namespace hardware.conformance;

[export] micro c106_device_snapshot_frozen() -> i64 {
    let outcome: Result<hardware::Device, hardware::HardwareFault> = hardware::hardware_request_device(true)
    let fallback = hardware::Device {
        _backend: 0,
        _ordinal: 0,
        _kind: 0,
        _features: 0,
        _lost: true,
    }
    let device = hardware::hardware_outcome_value_or(outcome, fallback)
    if hardware::device_is_lost(device) {
        return 604
    }
    return 0
}
"#;
    compile_consumer(consumer).expect("去标注后必须可推导");
}

/// 探针：三处标注全部省略（含 `outcome`）仍可推导。
#[test]
fn probe_all_annotations_omitted() {
    let consumer = r#"
namespace hardware.conformance;

[export] micro c106_device_snapshot_frozen() -> i64 {
    let outcome = hardware::hardware_request_device(true)
    let fallback = hardware::Device {
        _backend: 0,
        _ordinal: 0,
        _kind: 0,
        _features: 0,
        _lost: true,
    }
    let device = hardware::hardware_outcome_value_or(outcome, fallback)
    if hardware::device_is_lost(device) {
        return 604
    }
    return 0
}
"#;
    compile_consumer(consumer).expect("全部省略标注后仍必须可推导");
}

/// 探针：去标注形态必须通过完整程序管线。
#[test]
fn probe_unannotated_full_program_pipeline() {
    let consumer = r#"
namespace hardware.conformance;

[export] micro c106_device_snapshot_frozen() -> i64 {
    let outcome: Result<hardware::Device, hardware::HardwareFault> = hardware::hardware_request_device(true)
    let fallback = hardware::Device {
        _backend: 0,
        _ordinal: 0,
        _kind: 0,
        _features: 0,
        _lost: true,
    }
    let device = hardware::hardware_outcome_value_or(outcome, fallback)
    if hardware::device_is_lost(device) {
        return 604
    }
    return 0
}
"#;
    let groups = [
        CompilerSourceGroup::single_source("primitive", "primitive", STD_PRIMITIVES),
        CompilerSourceGroup::single_source("std", "std", STD_TYPES_LIBRARY).with_direct_dependencies(vec!["primitive".into()]),
        CompilerSourceGroup::single_source("hardware", "hardware", HARDWARE_LIBRARY)
            .with_direct_dependencies(vec!["primitive".into(), "std".into()]),
        CompilerSourceGroup::single_source("conformance", "conformance", consumer).as_entry().with_direct_dependencies(vec![
            "primitive".into(),
            "std".into(),
            "hardware".into(),
        ]),
    ];
    ValkyrieCompiler::default().compile_source_groups_to_program(&groups).expect("去标注形态必须通过完整程序管线");
}

/// 反例探针：`T` 只出现在返回位置且无实参、无标注时不得静默猜类型。
#[test]
fn probe_return_only_generic_without_hint_fails_closed() {
    let consumer = r#"
namespace hardware.conformance;

[export] micro c900_unspecified() -> i64 {
    let value = hardware::hardware_make_unspecified()
    return 0
}
"#;
    let result = compile_consumer(consumer);
    assert!(result.is_err(), "无信息来源的泛型返回必须结构化失败，不得猜类型，实际 {result:?}");
}

/// 反例探针（全管线）：同一不可推导形态在完整程序构建中也必须结构化失败。
#[test]
fn probe_return_only_generic_full_pipeline_fails_closed() {
    let consumer = r#"
namespace hardware.conformance;

[export] micro c900_unspecified() -> i64 {
    let value = hardware::hardware_make_unspecified()
    return 0
}
"#;
    let groups = [
        CompilerSourceGroup::single_source("primitive", "primitive", STD_PRIMITIVES),
        CompilerSourceGroup::single_source("std", "std", STD_TYPES_LIBRARY).with_direct_dependencies(vec!["primitive".into()]),
        CompilerSourceGroup::single_source("hardware", "hardware", HARDWARE_LIBRARY)
            .with_direct_dependencies(vec!["primitive".into(), "std".into()]),
        CompilerSourceGroup::single_source("conformance", "conformance", consumer).as_entry().with_direct_dependencies(vec![
            "primitive".into(),
            "std".into(),
            "hardware".into(),
        ]),
    ];
    let result = ValkyrieCompiler::default().compile_source_groups_to_program(&groups);
    assert!(result.is_err(), "不可推导泛型调用必须在完整管线结构化失败，实际 {result:?}");
}
