//! CLI 的 JSON 与运行时值边界；缺少导出类型和布局合同的结构值必须拒绝。

use miette::{IntoDiagnostic, Result};
use nyar_gc::{ObjectHeap, ObjectPayload};
use acorn_odin::{OvmAbiExport, OvmAbiType, OvmOptionBoxing, OvmTextRepresentation};
use serde_json::Value as JsonValue;

use crate::{array_runtime, error::OdinRuntimeError, text_runtime, value::Value};

/// `Option<Option<T>>` 的 boxed 线格式：`null` 表示外层 `None`，
/// `{"$some": <inner>}` 表示外层 `Some`（内层 `None` 即 `{"$some": null}`）。
const SOME_WIRE_KEY: &str = "$some";

/// 将 JSON 标量转换为运行时值，不分配堆对象。
pub fn value_from_json(json: &JsonValue) -> Result<Value, OdinRuntimeError> {
    materialize_value_from_json(json, None)
}

/// 转换 JSON 标量；提供堆也不能代替缺失的导出类型和布局合同。
pub fn materialize_value_from_json(json: &JsonValue, heap: Option<&mut ObjectHeap>) -> Result<Value, OdinRuntimeError> {
    Ok(match json {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(value) => Value::Bool(*value),
        JsonValue::Number(number) => json_number_to_value(number)?,
        JsonValue::String(value) => Value::String(value.clone()),
        JsonValue::Array(_) => {
            if heap.is_none() {
                return Err(OdinRuntimeError::TypeMismatch {
                    expected: "heap-backed array contract",
                    actual: "JSON array without runtime heap".to_owned(),
                });
            }
            return Err(OdinRuntimeError::UnsupportedFeature("JSON array export type/layout contract"));
        }
        JsonValue::Object(_) => {
            return Err(OdinRuntimeError::TypeMismatch {
                expected: "declared object export contract",
                actual: "JSON object without layout contract".to_owned(),
            });
        }
    })
}

fn json_number_to_value(number: &serde_json::Number) -> Result<Value, OdinRuntimeError> {
    if let Some(value) = number.as_i64() {
        Ok(scalar_i64(value))
    }
    else if let Some(value) = number.as_f64() {
        Ok(Value::F64(value))
    }
    else {
        Err(OdinRuntimeError::TypeMismatch { expected: "number", actual: number.to_string() })
    }
}

fn scalar_i64(value: i64) -> Value {
    if value >= i32::MIN as i64 && value <= i32::MAX as i64 { Value::I32(value as i32) } else { Value::I64(value) }
}

/// 将运行时标量序列化为 JSON；结构值没有正式导出合同就直接失败。
pub fn value_to_json(value: &Value) -> Result<JsonValue, OdinRuntimeError> {
    value_to_json_with_heap(value, None)
}

/// 将运行时值序列化为 JSON；对象必须有明确的导出布局合同。
pub fn value_to_json_with_heap(value: &Value, heap: Option<&ObjectHeap>) -> Result<JsonValue, OdinRuntimeError> {
    match value {
        Value::Null => Ok(JsonValue::Null),
        Value::Bool(value) => Ok(JsonValue::Bool(*value)),
        Value::I32(value) => Ok(JsonValue::from(*value)),
        Value::I64(value) => Ok(JsonValue::from(*value)),
        Value::F32(value) => Ok(JsonValue::from(*value)),
        Value::F64(value) => Ok(JsonValue::from(*value)),
        Value::String(value) => Ok(JsonValue::String(value.clone())),
        Value::Object(id) => {
            if heap.is_none() {
                return Err(OdinRuntimeError::TypeMismatch {
                    expected: "declared object export contract",
                    actual: format!("object#{id} without heap"),
                });
            }
            Err(OdinRuntimeError::UnsupportedFeature("JSON object export type/layout contract"))
        }
        Value::Coroutine(id) => Err(OdinRuntimeError::TypeMismatch { expected: "JSON-exportable value", actual: format!("coroutine#{id}") }),
    }
}

/// 解析顶层调用参数列表；当前只接受标量参数。
pub fn parse_call_args_json(source: &str) -> Result<Vec<Value>> {
    let json: JsonValue = serde_json::from_str(source).into_diagnostic()?;
    let JsonValue::Array(items) = json
    else {
        return Err(miette::miette!("--args-json must be a JSON array"));
    };
    items.iter().map(value_from_json).collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// 在运行时堆边界解析调用参数；结构参数缺少导出合同则失败。
pub fn parse_call_args_json_with_heap(source: &str, heap: &mut ObjectHeap) -> Result<Vec<Value>> {
    let json: JsonValue = serde_json::from_str(source).into_diagnostic()?;
    let JsonValue::Array(items) = json
    else {
        return Err(miette::miette!("--args-json must be a JSON array"));
    };
    let mut args = Vec::with_capacity(items.len());
    for item in &items {
        args.push(materialize_value_from_json(item, Some(heap))?);
    }
    Ok(args)
}

fn contract_failure(expected: &'static str, actual: String) -> OdinRuntimeError {
    OdinRuntimeError::TypeMismatch { expected, actual }
}

fn json_kind(json: &JsonValue) -> &'static str {
    match json {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn layout_index(layout: i32, what: &'static str) -> Result<u32, OdinRuntimeError> {
    u32::try_from(layout).map_err(|_| contract_failure(what, format!("negative layout index `{layout}`")))
}

fn object_slots<'a>(heap: &'a ObjectHeap, value: &Value, what: &'static str) -> Result<&'a [Value], OdinRuntimeError> {
    let Value::Object(id) = value
    else {
        return Err(contract_failure(what, value.type_name().to_string()));
    };
    match heap.get(*id) {
        Some(ObjectPayload::LayoutObject { slots, .. }) => Ok(slots.as_slice()),
        Some(_) => Err(contract_failure(what, format!("coroutine-backed object#{id}"))),
        None => Err(contract_failure(what, format!("dangling object#{id}"))),
    }
}

fn set_slot(heap: &mut ObjectHeap, id: nyar_gc::ObjectId, slot: usize, value: Value) -> Result<(), OdinRuntimeError> {
    heap.set_field(id, slot, value).map_err(|message| contract_failure("declared layout slot", message.to_string()))
}

/// 严格按合同解析调用参数列表。
///
/// 每个参数只按 sidecar 记录的结构物化；合同缺失或形状不符即失败闭合，
/// 禁止退化成“看起来像字符串就按 raw、看起来像对象就按托管”的运行时探测。
pub fn parse_call_args_json_with_contract(source: &str, export: &OvmAbiExport, heap: &mut ObjectHeap) -> Result<Vec<Value>, OdinRuntimeError> {
    let json: JsonValue = serde_json::from_str(source).map_err(|error| contract_failure("JSON args array", format!("{error}")))?;
    let JsonValue::Array(items) = json
    else {
        return Err(contract_failure("JSON args array", json_kind(&json).to_string()));
    };
    if items.len() != export.params.len() {
        return Err(contract_failure(
            "declared parameter count",
            format!("{} args for `{}` expects {}", items.len(), export.logical_name, export.params.len()),
        ));
    }
    let mut args = Vec::with_capacity(items.len());
    for (item, param) in items.iter().zip(export.params.iter()) {
        let value = materialize_value(item, &param.ty, heap)
            .map_err(|error| contract_failure("declared parameter contract", format!("`{}` (index {}): {error}", param.name, param.index)))?;
        args.push(value);
    }
    Ok(args)
}

fn materialize_value(json: &JsonValue, ty: &OvmAbiType, heap: &mut ObjectHeap) -> Result<Value, OdinRuntimeError> {
    match ty {
        OvmAbiType::Unit => match json {
            JsonValue::Null => Ok(Value::Null),
            _ => Err(contract_failure("unit (JSON null)", json_kind(json).to_string())),
        },
        OvmAbiType::I32 => match json.as_i64() {
            Some(value) if (i32::MIN as i64..=i32::MAX as i64).contains(&value) => Ok(Value::I32(value as i32)),
            _ => Err(contract_failure("i32 JSON integer", json_kind(json).to_string())),
        },
        OvmAbiType::I64 => match json.as_i64() {
            Some(value) => Ok(Value::I64(value)),
            None => Err(contract_failure("i64 JSON integer", json_kind(json).to_string())),
        },
        OvmAbiType::F32 => match json.as_f64() {
            Some(value) => Ok(Value::F32(value as f32)),
            None => Err(contract_failure("f32 JSON number", json_kind(json).to_string())),
        },
        OvmAbiType::F64 => match json.as_f64() {
            Some(value) => Ok(Value::F64(value)),
            None => Err(contract_failure("f64 JSON number", json_kind(json).to_string())),
        },
        OvmAbiType::Bool => match json.as_bool() {
            Some(value) => Ok(Value::Bool(value)),
            None => Err(contract_failure("bool JSON boolean", json_kind(json).to_string())),
        },
        OvmAbiType::Char => {
            let mut chars = json.as_str().into_iter().flat_map(|text| text.chars());
            match (chars.next(), chars.next()) {
                (Some(ch), None) => Ok(Value::I32((ch as u32) as i32)),
                _ => Err(contract_failure("char JSON single-character string", json_kind(json).to_string())),
            }
        }
        OvmAbiType::Text { representation } => match representation {
            // Raw 与 Managed 是合同事实，不是运行时探测结果。
            OvmTextRepresentation::Raw => match json.as_str() {
                Some(text) => Ok(Value::String(text.to_owned())),
                None => Err(contract_failure("raw utf8 JSON string", json_kind(json).to_string())),
            },
            OvmTextRepresentation::Managed { layout, field_count, byte_slot } => {
                let Some(text) = json.as_str()
                else {
                    return Err(contract_failure("managed utf8 JSON string", json_kind(json).to_string()));
                };
                if *byte_slot >= *field_count {
                    return Err(contract_failure("managed utf8 byte slot", format!("byte_slot {byte_slot} outside field_count {field_count}")));
                }
                let bytes = text.as_bytes().iter().map(|byte| Value::I32(*byte as i32)).collect::<Vec<_>>();
                let bytes = array_runtime::build_fixed_array(heap, 0, &bytes);
                let id = heap.alloc_layout_object(layout_index(*layout, "managed utf8 layout")?, *field_count);
                set_slot(heap, id, *byte_slot, bytes)?;
                Ok(Value::Object(id))
            }
        },
        OvmAbiType::Array { element } => {
            let Some(items) = json.as_array()
            else {
                return Err(contract_failure("array JSON array", json_kind(json).to_string()));
            };
            let elements = items.iter().map(|item| materialize_value(item, element, heap)).collect::<Result<Vec<_>, _>>()?;
            Ok(array_runtime::build_fixed_array(heap, 0, &elements))
        }
        OvmAbiType::FixedArray { element, length } => {
            let Some(items) = json.as_array()
            else {
                return Err(contract_failure("fixed array JSON array", json_kind(json).to_string()));
            };
            if items.len() != *length {
                return Err(contract_failure("fixed array length", format!("expected {length}, got {}", items.len())));
            }
            let elements = items.iter().map(|item| materialize_value(item, element, heap)).collect::<Result<Vec<_>, _>>()?;
            Ok(array_runtime::build_fixed_array(heap, 0, &elements))
        }
        OvmAbiType::Option { inner, boxing } => {
            if json.is_null() {
                return Ok(Value::Null);
            }
            match boxing {
                OvmOptionBoxing::Niche => materialize_value(json, inner, heap),
                OvmOptionBoxing::Boxed { layout, tag_slot, payload_slot } => {
                    let JsonValue::Object(map) = json
                    else {
                        return Err(contract_failure("boxed option {\"$some\": ...}", json_kind(json).to_string()));
                    };
                    if map.len() != 1 || !map.contains_key(SOME_WIRE_KEY) {
                        return Err(contract_failure("boxed option {\"$some\": ...}", format!("keys {:?}", map.keys().collect::<Vec<_>>())));
                    }
                    let payload = materialize_value(&map[SOME_WIRE_KEY], inner, heap)?;
                    let id = heap.alloc_layout_object(layout_index(*layout, "boxed option layout")?, 2);
                    set_slot(heap, id, *tag_slot, Value::I32(1))?;
                    set_slot(heap, id, *payload_slot, payload)?;
                    Ok(Value::Object(id))
                }
            }
        }
        OvmAbiType::Tuple { fields } => {
            let Some(items) = json.as_array()
            else {
                return Err(contract_failure("tuple JSON array", json_kind(json).to_string()));
            };
            if items.len() != fields.len() {
                return Err(contract_failure("tuple arity", format!("expected {}, got {}", fields.len(), items.len())));
            }
            let elements =
                items.iter().zip(fields.iter()).map(|(item, field)| materialize_value(item, field, heap)).collect::<Result<Vec<_>, _>>()?;
            Ok(array_runtime::build_fixed_array(heap, 0, &elements))
        }
        OvmAbiType::Aggregate { layout, name, fields, .. } => {
            let JsonValue::Object(map) = json
            else {
                return Err(contract_failure("named aggregate JSON object", json_kind(json).to_string()));
            };
            if map.len() != fields.len() {
                return Err(contract_failure(
                    "named aggregate field count",
                    format!("`{name}` expects {} fields, got {}", fields.len(), map.len()),
                ));
            }
            let id = heap.alloc_layout_object(layout_index(*layout, "named aggregate layout")?, fields.len());
            for (slot, field) in fields.iter().enumerate() {
                let Some(field_json) = map.get(&field.name)
                else {
                    return Err(contract_failure("named aggregate field name", format!("`{name}` missing field `{}`", field.name)));
                };
                let value = materialize_value(field_json, &field.ty, heap)?;
                set_slot(heap, id, slot, value)?;
            }
            Ok(Value::Object(id))
        }
        OvmAbiType::InstanceHandle { name, .. } => {
            Err(contract_failure("instance handle construction contract", format!("`{name}` 只能由语言侧构造函数创建，宿主不得按字段重建")))
        }
        OvmAbiType::Unite { layout, name, tag_slot, payload_slot, variants } => {
            let (variant, payload) = match json {
                // 零载荷变体在线格式上只有身份名，与带载荷变体的对象形态可区分。
                JsonValue::String(variant_name) => {
                    let Some(variant) = variants.iter().find(|variant| &variant.name == variant_name)
                    else {
                        return Err(contract_failure("declared unite variant name", format!("`{name}` 没有变体 `{variant_name}`")));
                    };
                    if variant.payload.is_some() {
                        return Err(contract_failure(
                            "unite payload presence",
                            format!("`{name}.{variant_name}` 携带载荷，必须用 {{\"variant\",\"payload\"}} 形态"),
                        ));
                    }
                    (variant, Value::Null)
                }
                JsonValue::Object(map) => {
                    let variant_name = map
                        .get("variant")
                        .and_then(JsonValue::as_str)
                        .ok_or_else(|| contract_failure("unite variant name", format!("`{name}` 缺少字符串 `variant` 键")))?;
                    let Some(variant) = variants.iter().find(|variant| variant.name == variant_name)
                    else {
                        return Err(contract_failure("declared unite variant name", format!("`{name}` 没有变体 `{variant_name}`")));
                    };
                    let Some(payload_ty) = variant.payload.as_deref()
                    else {
                        return Err(contract_failure(
                            "unite payload presence",
                            format!("`{name}.{variant_name}` 零载荷变体不接受 `payload` 键"),
                        ));
                    };
                    let Some(payload_json) = map.get("payload")
                    else {
                        return Err(contract_failure("unite payload presence", format!("`{name}.{variant_name}` 缺少 `payload` 键")));
                    };
                    if map.len() != 2 {
                        return Err(contract_failure("unite wire keys", format!("`{name}` 只接受 `variant` + `payload`，收到额外键")));
                    }
                    (variant, materialize_value(payload_json, payload_ty, heap)?)
                }
                _ => return Err(contract_failure("unite wire (variant name or object)", json_kind(json).to_string())),
            };
            let id = heap.alloc_layout_object(layout_index(*layout, "unite layout")?, 2);
            set_slot(heap, id, *tag_slot, Value::I32(variant.tag as i32))?;
            set_slot(heap, id, *payload_slot, payload)?;
            Ok(Value::Object(id))
        }
    }
}

/// 严格按合同序列化导出返回值。
pub fn value_to_json_with_contract(value: &Value, returns: &OvmAbiType, heap: &ObjectHeap) -> Result<JsonValue, OdinRuntimeError> {
    match returns {
        OvmAbiType::Unit => match value {
            Value::Null => Ok(JsonValue::Null),
            other => Err(contract_failure("unit return", other.type_name().to_string())),
        },
        OvmAbiType::I32 => match value {
            Value::I32(value) => Ok(JsonValue::from(*value)),
            other => Err(contract_failure("i32 return", other.type_name().to_string())),
        },
        OvmAbiType::I64 => match value {
            Value::I64(value) => Ok(JsonValue::from(*value)),
            other => Err(contract_failure("i64 return", other.type_name().to_string())),
        },
        OvmAbiType::F32 => match value {
            Value::F32(value) => Ok(JsonValue::from(*value)),
            other => Err(contract_failure("f32 return", other.type_name().to_string())),
        },
        OvmAbiType::F64 => match value {
            Value::F64(value) => Ok(JsonValue::from(*value)),
            other => Err(contract_failure("f64 return", other.type_name().to_string())),
        },
        OvmAbiType::Bool => match value {
            Value::Bool(value) => Ok(JsonValue::Bool(*value)),
            other => Err(contract_failure("bool return", other.type_name().to_string())),
        },
        OvmAbiType::Char => match value {
            Value::I32(code) => char::from_u32(*code as u32)
                .map(|ch| JsonValue::String(ch.to_string()))
                .ok_or_else(|| contract_failure("char return", format!("invalid scalar value {code}"))),
            other => Err(contract_failure("char return", other.type_name().to_string())),
        },
        OvmAbiType::Text { representation } => match representation {
            OvmTextRepresentation::Raw => match value {
                Value::String(text) => Ok(JsonValue::String(text.clone())),
                other => Err(contract_failure("raw utf8 return", other.type_name().to_string())),
            },
            OvmTextRepresentation::Managed { byte_slot, .. } => {
                let slots = object_slots(heap, value, "managed utf8 return")?;
                let bytes_value = slots
                    .get(*byte_slot)
                    .ok_or_else(|| contract_failure("managed utf8 byte slot", format!("slot {byte_slot} out of range")))?;
                let bytes = byte_array(heap, bytes_value)?;
                Ok(JsonValue::String(
                    text_runtime::from_bytes(&bytes, "utf8 managed contract")
                        .map_err(|error| contract_failure("managed utf8 byte payload", error.to_string()))?,
                ))
            }
        },
        OvmAbiType::Array { element } => {
            let slots = object_slots(heap, value, "array return")?;
            let items = slots.iter().map(|item| value_to_json_with_contract(item, element, heap)).collect::<Result<Vec<_>, _>>()?;
            Ok(JsonValue::Array(items))
        }
        OvmAbiType::FixedArray { element, length } => {
            let slots = object_slots(heap, value, "fixed array return")?;
            if slots.len() != *length {
                return Err(contract_failure("fixed array return length", format!("expected {length}, got {}", slots.len())));
            }
            let items = slots.iter().map(|item| value_to_json_with_contract(item, element, heap)).collect::<Result<Vec<_>, _>>()?;
            Ok(JsonValue::Array(items))
        }
        OvmAbiType::Option { inner, boxing } => {
            if matches!(value, Value::Null) {
                return Ok(JsonValue::Null);
            }
            match boxing {
                OvmOptionBoxing::Niche => value_to_json_with_contract(value, inner, heap),
                OvmOptionBoxing::Boxed { tag_slot, payload_slot, .. } => {
                    let slots = object_slots(heap, value, "boxed option return")?;
                    let tag = slots
                        .get(*tag_slot)
                        .ok_or_else(|| contract_failure("boxed option tag slot", format!("slot {tag_slot} out of range")))?;
                    if !matches!(tag, Value::I32(1)) {
                        return Err(contract_failure("boxed option tag", format!("expected 1 (Some), got {}", tag.type_name())));
                    }
                    let payload = slots
                        .get(*payload_slot)
                        .ok_or_else(|| contract_failure("boxed option payload slot", format!("slot {payload_slot} out of range")))?;
                    let inner_json = value_to_json_with_contract(payload, inner, heap)?;
                    let mut map = serde_json::Map::new();
                    map.insert(SOME_WIRE_KEY.to_owned(), inner_json);
                    Ok(JsonValue::Object(map))
                }
            }
        }
        OvmAbiType::Tuple { fields } => {
            let slots = object_slots(heap, value, "tuple return")?;
            if slots.len() != fields.len() {
                return Err(contract_failure("tuple return arity", format!("expected {}, got {}", fields.len(), slots.len())));
            }
            let items = slots
                .iter()
                .zip(fields.iter())
                .map(|(slot, field)| value_to_json_with_contract(slot, field, heap))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(JsonValue::Array(items))
        }
        OvmAbiType::Aggregate { name, fields, .. } => {
            let slots = object_slots(heap, value, "named aggregate return")?;
            if slots.len() < fields.len() {
                return Err(contract_failure(
                    "named aggregate return slots",
                    format!("`{name}` expects {} slots, object has {}", fields.len(), slots.len()),
                ));
            }
            let mut map = serde_json::Map::with_capacity(fields.len());
            for (slot, field) in fields.iter().enumerate() {
                map.insert(field.name.clone(), value_to_json_with_contract(&slots[slot], &field.ty, heap)?);
            }
            Ok(JsonValue::Object(map))
        }
        OvmAbiType::InstanceHandle { name, .. } => {
            Err(contract_failure("instance handle serialization contract", format!("`{name}` 是不透明实例句柄，不能序列化成字段快照")))
        }
        OvmAbiType::Unite { name, tag_slot, payload_slot, variants, .. } => {
            if matches!(value, Value::Null) {
                return Err(contract_failure("unite return", "null".to_owned()));
            }
            let slots = object_slots(heap, value, "unite return")?;
            let tag =
                slots.get(*tag_slot).ok_or_else(|| contract_failure("unite tag slot", format!("`{name}` tag_slot {tag_slot} out of range")))?;
            let Value::I32(tag) = tag
            else {
                return Err(contract_failure("unite tag return", format!("`{name}` tag slot holds {}", tag.type_name())));
            };
            let Some(variant) = variants.iter().find(|variant| variant.tag == *tag as u32)
            else {
                return Err(contract_failure("declared unite tag", format!("`{name}` 没有 tag {tag} 的变体")));
            };
            match variant.payload.as_deref() {
                None => Ok(JsonValue::String(variant.name.clone())),
                Some(payload_ty) => {
                    let payload = slots
                        .get(*payload_slot)
                        .ok_or_else(|| contract_failure("unite payload slot", format!("`{name}` payload_slot {payload_slot} out of range")))?;
                    let mut map = serde_json::Map::new();
                    map.insert("variant".to_owned(), JsonValue::String(variant.name.clone()));
                    map.insert("payload".to_owned(), value_to_json_with_contract(payload, payload_ty, heap)?);
                    Ok(JsonValue::Object(map))
                }
            }
        }
    }
}

/// 托管字节数组：槽位必须是 `I32` 0..=255 的稠密对象。
fn byte_array(heap: &ObjectHeap, value: &Value) -> Result<Vec<u8>, OdinRuntimeError> {
    object_slots(heap, value, "utf8 byte array")?
        .iter()
        .map(|byte| match byte {
            Value::I32(byte) if (0..=0xFF).contains(byte) => Ok(*byte as u8),
            other => Err(contract_failure("utf8 byte array element", other.type_name().to_string())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorn_odin::OvmAbiParam;

    fn export(param: OvmAbiType, returns: OvmAbiType) -> OvmAbiExport {
        OvmAbiExport {
            logical_name: "entry".to_string(),
            params: vec![OvmAbiParam { index: 0, name: "value".to_string(), ty: param, carrier: "handle".to_string() }],
            returns,
            returns_carrier: "handle".to_string(),
        }
    }

    fn roundtrip(param: &OvmAbiType, json: &str) -> JsonValue {
        let mut heap = ObjectHeap::new();
        let args = parse_call_args_json_with_contract(&format!("[{json}]"), &export(param.clone(), param.clone()), &mut heap)
            .expect("参数必须按合同物化");
        value_to_json_with_contract(&args[0], param, &heap).expect("返回值必须按合同序列化")
    }

    #[test]
    fn raw_and_managed_text_follow_the_recorded_contract() {
        let raw = OvmAbiType::Text { representation: OvmTextRepresentation::Raw };
        let managed = OvmAbiType::Text { representation: OvmTextRepresentation::Managed { layout: 3, field_count: 1, byte_slot: 0 } };
        let mut heap = ObjectHeap::new();
        let args = parse_call_args_json_with_contract("[\"abc\"]", &export(raw.clone(), raw.clone()), &mut heap).expect("raw utf8 参数");
        assert_eq!(args[0], Value::String("abc".to_string()), "Raw 槽必须保持宿主串");
        // 同一 JSON 在 Managed 合同下必须是托管对象，而不是被探测成 raw 串。
        let mut heap = ObjectHeap::new();
        let args =
            parse_call_args_json_with_contract("[\"abc\"]", &export(managed.clone(), managed.clone()), &mut heap).expect("managed utf8 参数");
        assert!(matches!(args[0], Value::Object(_)), "Managed 槽必须是托管对象");
        assert_eq!(value_to_json_with_contract(&args[0], &managed, &heap).expect("托管串序列化"), JsonValue::String("abc".to_string()));
        // Managed 合同收到裸串必须失败闭合：不许“当 Object 不行再当 String”。
        let mut heap = ObjectHeap::new();
        let error = parse_call_args_json_with_contract("[[1]]", &export(managed, raw), &mut heap).expect_err("宿主裸串不是托管文本");
        assert!(error.to_string().contains("managed utf8 JSON string"), "got: {error}");
    }

    #[test]
    fn unite_selects_variant_and_only_reads_matching_payload() {
        let unite = OvmAbiType::Unite {
            layout: 7,
            name: "Maybe".to_string(),
            tag_slot: 0,
            payload_slot: 1,
            variants: vec![
                acorn_odin::OvmAbiVariant { name: "None".to_string(), tag: 0, payload: None },
                acorn_odin::OvmAbiVariant { name: "Some".to_string(), tag: 1, payload: Some(Box::new(OvmAbiType::I32)) },
            ],
        };
        assert_eq!(roundtrip(&unite, "\"None\""), JsonValue::String("None".to_string()));
        assert_eq!(roundtrip(&unite, "{\"variant\":\"Some\",\"payload\":42}"), serde_json::json!({ "variant": "Some", "payload": 42 }));
        // 零载荷变体不接受 payload 键，带载荷变体必须携带 payload。
        let mut heap = ObjectHeap::new();
        let error =
            parse_call_args_json_with_contract("[{\"variant\":\"None\",\"payload\":1}]", &export(unite.clone(), unite.clone()), &mut heap)
                .expect_err("零载荷变体不得伪造载荷");
        assert!(error.to_string().contains("零载荷变体"), "got: {error}");
    }

    #[test]
    fn boxed_option_keeps_some_none_distinguishable_from_none() {
        let option = OvmAbiType::Option {
            inner: Box::new(OvmAbiType::Option { inner: Box::new(OvmAbiType::I64), boxing: OvmOptionBoxing::Niche }),
            boxing: OvmOptionBoxing::Boxed { layout: 9, tag_slot: 0, payload_slot: 1 },
        };
        let mut heap = ObjectHeap::new();
        let args = parse_call_args_json_with_contract(
            "[null,{\"$some\":null},{\"$some\":7}]",
            &OvmAbiExport {
                logical_name: "entry".to_string(),
                params: (0..3)
                    .map(|index| OvmAbiParam { index, name: format!("value{index}"), ty: option.clone(), carrier: "handle".to_string() })
                    .collect(),
                returns: option.clone(),
                returns_carrier: "handle".to_string(),
            },
            &mut heap,
        )
        .expect("三种 Option 形态都必须可物化");
        assert_eq!(args[0], Value::Null, "None");
        assert!(matches!(args[1], Value::Object(_)), "Some(None) 必须是 box 对象，不能塌陷成 null");
        assert!(matches!(args[2], Value::Object(_)), "Some(Some(7))");
        assert_eq!(value_to_json_with_contract(&args[0], &option, &heap).expect("None"), JsonValue::Null);
        assert_eq!(value_to_json_with_contract(&args[1], &option, &heap).expect("Some(None)"), serde_json::json!({ "$some": null }));
        assert_eq!(value_to_json_with_contract(&args[2], &option, &heap).expect("Some(Some(7))"), serde_json::json!({ "$some": 7 }));
    }
}
