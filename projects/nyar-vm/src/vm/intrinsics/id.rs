use crate::bytecode::opcode::NyarBuiltin;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    Builtin(NyarBuiltin),
    BigIntConst,
    BigIntAdd,
    BigIntSub,
    BigIntMul,
    BigIntDiv,
    BigIntMod,
    BigIntNeg,
    BigIntEq,
    BigIntNe,
    BigIntLt,
    BigIntLe,
    BigIntGt,
    BigIntGe,
    BigIntToI64,
    BigIntFromI64,
    BigIntToString,
    StringConcat,
    StringLenBytes,
    StringLenChars,
    StringEq,
    StringNe,
    StringLt,
    StringLe,
    StringGt,
    StringGe,
    StringSubstr,
}

impl Intrinsic {
    pub fn try_from_u32(id: u32) -> Option<Self> {
        Some(match id {
            1 => Self::Builtin(NyarBuiltin::Print),
            2 => Self::Builtin(NyarBuiltin::Println),
            3 => Self::Builtin(NyarBuiltin::Exit),
            4 => Self::Builtin(NyarBuiltin::GetTime),
            5 => Self::Builtin(NyarBuiltin::Sleep),
            6 => Self::Builtin(NyarBuiltin::NativeAdd),
            7 => Self::Builtin(NyarBuiltin::Panic),
            8 => Self::Builtin(NyarBuiltin::MathSin),
            9 => Self::Builtin(NyarBuiltin::MathSqrt),
            10 => Self::Builtin(NyarBuiltin::MemAlloc),
            11 => Self::Builtin(NyarBuiltin::MathAbs),
            12 => Self::Builtin(NyarBuiltin::MathCos),
            13 => Self::Builtin(NyarBuiltin::MathTan),
            14 => Self::Builtin(NyarBuiltin::BitAnd),
            15 => Self::Builtin(NyarBuiltin::BitOr),
            16 => Self::Builtin(NyarBuiltin::BitXor),
            17 => Self::Builtin(NyarBuiltin::BitNot),
            18 => Self::Builtin(NyarBuiltin::BitShl),
            19 => Self::Builtin(NyarBuiltin::BitShr),
            20 => Self::Builtin(NyarBuiltin::MemFree),
            21 => Self::Builtin(NyarBuiltin::MemRealloc),
            22 => Self::Builtin(NyarBuiltin::MemSet),
            23 => Self::Builtin(NyarBuiltin::MemCopy),
            24 => Self::Builtin(NyarBuiltin::StrLen),
            25 => Self::Builtin(NyarBuiltin::StrCmp),
            26 => Self::Builtin(NyarBuiltin::MathRand),
            100 => Self::BigIntConst,
            101 => Self::BigIntAdd,
            102 => Self::BigIntSub,
            103 => Self::BigIntMul,
            104 => Self::BigIntDiv,
            105 => Self::BigIntMod,
            106 => Self::BigIntNeg,
            107 => Self::BigIntEq,
            108 => Self::BigIntNe,
            109 => Self::BigIntLt,
            110 => Self::BigIntLe,
            111 => Self::BigIntGt,
            112 => Self::BigIntGe,
            113 => Self::BigIntToI64,
            114 => Self::BigIntFromI64,
            115 => Self::BigIntToString,
            120 => Self::StringConcat,
            121 => Self::StringLenBytes,
            122 => Self::StringLenChars,
            123 => Self::StringEq,
            124 => Self::StringNe,
            125 => Self::StringLt,
            126 => Self::StringLe,
            127 => Self::StringGt,
            128 => Self::StringGe,
            129 => Self::StringSubstr,
            _ => return None,
        })
    }

    pub fn id(self) -> u32 {
        match self {
            Self::Builtin(b) => b as u32,
            Self::BigIntConst => 100,
            Self::BigIntAdd => 101,
            Self::BigIntSub => 102,
            Self::BigIntMul => 103,
            Self::BigIntDiv => 104,
            Self::BigIntMod => 105,
            Self::BigIntNeg => 106,
            Self::BigIntEq => 107,
            Self::BigIntNe => 108,
            Self::BigIntLt => 109,
            Self::BigIntLe => 110,
            Self::BigIntGt => 111,
            Self::BigIntGe => 112,
            Self::BigIntToI64 => 113,
            Self::BigIntFromI64 => 114,
            Self::BigIntToString => 115,
            Self::StringConcat => 120,
            Self::StringLenBytes => 121,
            Self::StringLenChars => 122,
            Self::StringEq => 123,
            Self::StringNe => 124,
            Self::StringLt => 125,
            Self::StringLe => 126,
            Self::StringGt => 127,
            Self::StringGe => 128,
            Self::StringSubstr => 129,
        }
    }

    pub fn execute_vm(self, vm: &mut NyarVM, args: &[Value]) -> Result<Value, NyarError> {
        match self {
            Self::Builtin(_) => Err(vm.error(nyar_types::VmErrorKind::RuntimeError(
                "builtin intrinsic must be provided by runtime".to_string(),
            ))),
            Self::BigIntConst => {
                if args.len() != 2 {
                    return Err(vm.error(nyar_types::VmErrorKind::RuntimeError(
                        "bigint_const expects 2 args".to_string(),
                    )));
                }
                let sign = args[0].try_as_int().ok_or_else(|| {
                    vm.error(nyar_types::VmErrorKind::TypeMismatch {
                        expected: "Int".to_string(),
                        found: format!("{:?}", args[0].tag()),
                    })
                })?;
                let bytes = args[1].try_as_bytes().ok_or_else(|| {
                    vm.error(nyar_types::VmErrorKind::TypeMismatch {
                        expected: "Bytes".to_string(),
                        found: format!("{:?}", args[1].tag()),
                    })
                })?;
                vm.execute_bigint_const(sign as u8, bytes.data.to_vec())?;
                vm.pop()
            }
            Self::BigIntAdd => execute_stack(vm, args, NyarVM::execute_bigint_add),
            Self::BigIntSub => execute_stack(vm, args, NyarVM::execute_bigint_sub),
            Self::BigIntMul => execute_stack(vm, args, NyarVM::execute_bigint_mul),
            Self::BigIntDiv => execute_stack(vm, args, NyarVM::execute_bigint_div),
            Self::BigIntMod => execute_stack(vm, args, NyarVM::execute_bigint_mod),
            Self::BigIntNeg => execute_stack(vm, args, NyarVM::execute_bigint_neg),
            Self::BigIntEq => execute_stack(vm, args, NyarVM::execute_bigint_eq),
            Self::BigIntNe => execute_stack(vm, args, NyarVM::execute_bigint_ne),
            Self::BigIntLt => execute_stack(vm, args, NyarVM::execute_bigint_lt),
            Self::BigIntLe => execute_stack(vm, args, NyarVM::execute_bigint_le),
            Self::BigIntGt => execute_stack(vm, args, NyarVM::execute_bigint_gt),
            Self::BigIntGe => execute_stack(vm, args, NyarVM::execute_bigint_ge),
            Self::BigIntToI64 => execute_stack(vm, args, NyarVM::execute_bigint_to_i64),
            Self::BigIntFromI64 => execute_stack(vm, args, NyarVM::execute_bigint_from_i64),
            Self::BigIntToString => execute_stack(vm, args, NyarVM::execute_bigint_to_string),
            Self::StringConcat => execute_stack(vm, args, NyarVM::execute_string_concat),
            Self::StringLenBytes => execute_stack(vm, args, NyarVM::execute_string_len_bytes),
            Self::StringLenChars => execute_stack(vm, args, NyarVM::execute_string_len_chars),
            Self::StringEq => execute_stack(vm, args, NyarVM::execute_string_eq),
            Self::StringNe => execute_stack(vm, args, NyarVM::execute_string_ne),
            Self::StringLt => execute_stack(vm, args, NyarVM::execute_string_lt),
            Self::StringLe => execute_stack(vm, args, NyarVM::execute_string_le),
            Self::StringGt => execute_stack(vm, args, NyarVM::execute_string_gt),
            Self::StringGe => execute_stack(vm, args, NyarVM::execute_string_ge),
            Self::StringSubstr => execute_stack(vm, args, NyarVM::execute_string_substr),
        }
    }
}

fn execute_stack(
    vm: &mut NyarVM,
    args: &[Value],
    f: fn(&mut NyarVM) -> Result<Option<usize>, NyarError>,
) -> Result<Value, NyarError> {
    let sp_before = vm.sp;
    for v in args {
        vm.push(*v)?;
    }
    let res = f(vm);
    match res {
        Ok(_) => vm.pop(),
        Err(e) => {
            vm.sp = sp_before;
            Err(e)
        }
    }
}
