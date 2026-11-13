//! Convert [`GenerateModule`] into `.odin` bytes.

use miette::{Result, miette};
use acorn_odin::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinImport, OdinImportKind, OdinModuleData, encode_module,
};

use super::generate::{GenerateConstant, GenerateExportKind, GenerateFunction, GenerateImportKind, GenerateModule, GenerateOperand};

/// Nyar bytecode compiler for generate-time modules.
#[derive(Debug, Default, Clone, Copy)]
pub struct NyarBytecodeCompiler;

impl NyarBytecodeCompiler {
    /// Compile a generate-time module into `.odin` bytes.
    pub fn compile(&self, module: &GenerateModule) -> Result<Vec<u8>> {
        Ok(encode_module(&self.to_module_data(module)?))
    }

    /// Convert a generate-time module into [`OdinModuleData`].
    pub fn to_module_data(&self, module: &GenerateModule) -> Result<OdinModuleData> {
        let mut code_bytes = Vec::new();
        let mut functions = Vec::new();

        for function in &module.functions {
            let code_offset = code_bytes.len() as i32;
            for instruction in &function.instructions {
                code_bytes.push(instruction.opcode as u8);
                for operand in &instruction.operands {
                    emit_operand(&mut code_bytes, operand, module, function)?;
                }
            }
            functions.push(OdinFunction {
                name: function.name.clone(),
                arity: function.parameters.len() as i32,
                local_count: function.slot_count(),
                code_offset,
                code_length: (code_bytes.len() as i32) - code_offset,
            });
        }

        Ok(OdinModuleData {
            version: Odin_VERSION,
            name: module.name.clone(),
            constants: convert_constants(&module.constants),
            functions,
            imports: convert_imports(&module.imports),
            exports: convert_exports(&module.exports),
            witness_entries: Vec::new(),
            code_bytes,
            globals: Vec::new(),
            init_function_indices: Vec::new(),
            layouts: Vec::new(),
        })
    }
}

fn emit_operand(writer: &mut Vec<u8>, operand: &GenerateOperand, module: &GenerateModule, function: &GenerateFunction) -> Result<()> {
    match operand {
        GenerateOperand::I32(value) => writer.extend_from_slice(&value.to_le_bytes()),
        GenerateOperand::I64(value) => writer.extend_from_slice(&value.to_le_bytes()),
        GenerateOperand::F32(value) => writer.extend_from_slice(&value.to_le_bytes()),
        GenerateOperand::F64(value) => writer.extend_from_slice(&value.to_le_bytes()),
        GenerateOperand::Str(value) => emit_string(writer, value),
        GenerateOperand::Local { index } | GenerateOperand::Param { index } | GenerateOperand::Const { pool_index: index } => {
            writer.extend_from_slice(&index.to_le_bytes())
        }
        GenerateOperand::Label { name } => {
            return Err(miette!("unresolved label `{name}` in function `{}`: resolve labels before emitting `.odin` bytecode", function.name));
        }
        GenerateOperand::FuncRef { name } => {
            let Some(resolved) = module.resolve_function_id(name)
            else {
                return Err(miette!(
                    "unresolved function reference `{name}` in function `{}`: module `{}` has no function with that name",
                    function.name,
                    module.name
                ));
            };
            writer.extend_from_slice(&resolved.index.to_le_bytes());
        }
        GenerateOperand::Null => writer.extend_from_slice(&0i32.to_le_bytes()),
    }
    Ok(())
}

fn emit_string(writer: &mut Vec<u8>, value: &str) {
    writer.extend_from_slice(&(value.len() as i32).to_le_bytes());
    writer.extend_from_slice(value.as_bytes());
}

fn convert_constants(pool: &super::generate::GenerateConstantPool) -> Vec<OdinConstant> {
    pool.entries
        .iter()
        .map(|entry| match entry {
            GenerateConstant::Null => OdinConstant::Null,
            GenerateConstant::Bool(value) => OdinConstant::Boolean(*value),
            // 超出 `i32` 表示范围必须用 `Integer64`：`value as i32` 只保留低 32 位
            // 并符号扩展，会把 `i64::MAX` 静默编成 `-1`。
            GenerateConstant::Int64(value) => match i32::try_from(*value) {
                Ok(value) => OdinConstant::Integer32(value),
                Err(_) => OdinConstant::Integer64(*value),
            },
            GenerateConstant::Float64(value) => OdinConstant::Float64(*value),
            GenerateConstant::String(value) => OdinConstant::String(value.clone()),
        })
        .collect()
}

fn convert_imports(imports: &[super::generate::GenerateModuleImport]) -> Vec<OdinImport> {
    imports
        .iter()
        .map(|import| {
            let kind = match import.kind {
                GenerateImportKind::Function | GenerateImportKind::Global | GenerateImportKind::Module => OdinImportKind::Function,
            };
            OdinImport { kind, module_name: import.module_name.clone(), symbol_name: import.symbol_name.clone() }
        })
        .collect()
}

fn convert_exports(exports: &[super::generate::GenerateModuleExport]) -> Vec<OdinExport> {
    exports
        .iter()
        .map(|export| {
            let kind = match export.kind {
                GenerateExportKind::Function | GenerateExportKind::Global => OdinExportKind::Function,
            };
            OdinExport { kind, symbol_name: export.name.clone(), function_index: export.function_index }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::generate::{GenerateFunction, GenerateInstruction, GenerateOperand};
    use acorn_odin::OdinHeadCode;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(" ")
    }

    fn constants_section(bytes: &[u8]) -> &[u8] {
        let count = i32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        for index in 0..count {
            let base = 16 + index * 9;
            if bytes[base] == 0x01 {
                let offset = i32::from_le_bytes(bytes[base + 1..base + 5].try_into().unwrap()) as usize;
                let size = i32::from_le_bytes(bytes[base + 5..base + 9].try_into().unwrap()) as usize;
                return &bytes[offset..offset + size];
            }
        }
        panic!("no constants section");
    }

    fn module_with_constants(values: &[i64]) -> GenerateModule {
        let mut module = GenerateModule::new("probe");
        for value in values {
            module.constants.add_int64(*value);
        }
        module
    }

    /// `i32` 范围内的整数保持 `Integer32`；越界必须发 `Integer64`，不得截断。
    #[test]
    fn i64_constants_choose_integer32_or_integer64_by_range() {
        let values = [i32::MAX as i64, 2_147_483_648, i64::MAX, i32::MIN as i64];
        let module = module_with_constants(&values);
        let data = NyarBytecodeCompiler.to_module_data(&module).expect("convert constants");

        assert_eq!(
            data.constants,
            vec![
                OdinConstant::Integer32(i32::MAX),
                OdinConstant::Integer64(2_147_483_648),
                OdinConstant::Integer64(i64::MAX),
                OdinConstant::Integer32(i32::MIN),
            ],
            "范围判定必须按 i32 边界逐值生效"
        );

        let bytes = encode_module(&data);
        assert_eq!(
            hex(constants_section(&bytes)),
            "04 00 00 00 \
             11 ff ff ff 7f \
             12 00 00 00 80 00 00 00 00 \
             12 ff ff ff ff ff ff ff 7f \
             11 00 00 00 80",
            "常量项必须逐字节：i32 边界 `0x11`，越界 `0x12` + 8 字节 LE"
        );
    }

    /// 解析成功的标签仍按相对偏移落盘（回归覆盖 `ResolvedLabel` 路径）。
    #[test]
    fn resolved_label_emits_relative_offset() {
        let mut module = GenerateModule::new("probe");
        let mut function = GenerateFunction::new("main");
        function.add_instruction(GenerateInstruction::new(OdinHeadCode::Jump, vec![GenerateOperand::Label { name: "end".to_string() }]));
        function.add_instruction(GenerateInstruction::new(OdinHeadCode::Return, Vec::new()));
        function.labels.insert("end".to_string(), 2);
        function.add_instruction(GenerateInstruction::new(OdinHeadCode::Return, Vec::new()));
        module.add_function(function);

        let data = NyarBytecodeCompiler.to_module_data(&module).expect("resolved label encodes");
        assert_eq!(hex(&data.code_bytes), "01 06 00 00 00 05 05", "跳转目标必须是相对偏移 6");
    }

    /// 未解析标签在发射阶段结构化失败，不得把“未解析”编码成偏移 0。
    #[test]
    fn unresolved_label_is_rejected_before_encoding() {
        let mut module = GenerateModule::new("probe");
        let mut function = GenerateFunction::new("main");
        function.add_instruction(GenerateInstruction::new(OdinHeadCode::Jump, vec![GenerateOperand::Label { name: "missing".to_string() }]));
        module.add_function(function);

        let error = NyarBytecodeCompiler.compile(&module).expect_err("unresolved label must fail");
        assert_eq!(format!("{error}"), "unresolved label `missing` in function `main`: resolve labels before emitting `.odin` bytecode");
    }

    /// 缺失函数引用在发射阶段结构化失败，不得回落成函数索引 0。
    #[test]
    fn missing_function_reference_is_rejected_before_encoding() {
        let mut module = GenerateModule::new("probe");
        let mut function = GenerateFunction::new("main");
        function.add_instruction(GenerateInstruction::new(OdinHeadCode::Call, vec![GenerateOperand::FuncRef { name: "ghost".to_string() }]));
        module.add_function(function);

        let error = NyarBytecodeCompiler.compile(&module).expect_err("missing function must fail");
        assert_eq!(
            format!("{error}"),
            "unresolved function reference `ghost` in function `main`: module `probe` has no function with that name"
        );
    }
}
