pub mod io;
pub mod fs;
pub mod http;
pub mod json;
pub mod toml;
pub mod von;
pub mod net;
pub mod async_ffi;
pub mod bigint;
pub mod string;
pub mod float;
pub mod int;

use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::Value;
use nyar_vm::vm::traits::{ExternFunc, RuntimeProvider};
use nyar_vm::vm::platform::NyarPlatform;
use nyar_types::NyarError;
use nyar_gc::Trace;
use dashmap::DashMap;
use std::sync::Arc;

pub type FFIResult = Result<Value, NyarError>;

#[derive(Clone, Debug)]
pub struct FFIRegistry {
    pub functions: Arc<DashMap<String, ExternFunc>>,
    pub intrinsics: Arc<DashMap<u32, ExternFunc>>,
    pub loaders: Arc<DashMap<String, Arc<dyn ModuleLoader>>>,
}

pub trait ModuleLoader: Send + Sync + std::fmt::Debug {
    fn load(&self, path: &str) -> Result<Vec<(String, ExternFunc)>, NyarError>;
}

impl Default for FFIRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for FFIRegistry {
    fn resolve(&self, name: &str) -> Option<ExternFunc> {
        self.functions.get(name).map(|r| *r.value())
    }
    fn get_intrinsic(&self, id: u32) -> Option<ExternFunc> {
        self.intrinsics.get(&id).map(|r| *r.value())
    }
}

impl FFIRegistry {
    pub fn new() -> Self {
        Self {
            functions: Arc::new(DashMap::new()),
            intrinsics: Arc::new(DashMap::new()),
            loaders: Arc::new(DashMap::new()),
        }
    }

    pub fn register_loader(&mut self, name: impl Into<String>, loader: Arc<dyn ModuleLoader>) {
        self.loaders.insert(name.into(), loader);
    }

    pub fn load_module(&mut self, provider: &str, path: &str) -> Result<(), String> {
        if let Some(loader) = self.loaders.get(provider) {
            let exports = loader.load(path).map_err(|e| e.to_string())?;
            for (name, func) in exports {
                self.functions.insert(name, func);
            }
            Ok(())
        } else {
            Err(format!("Unknown provider: {}", provider))
        }
    }

    pub fn register(&self, name: String, func: ExternFunc) {
        self.functions.insert(name, func);
    }

    pub fn register_intrinsic(&self, id: u32, func: ExternFunc) {
        self.intrinsics.insert(id, func);
    }

    pub fn register_std(&self) {
        self.register("std::io::print".to_string(), io::std_io_print);
        self.register("std::io::println".to_string(), io::std_io_println);
        self.register("std::io::read_line".to_string(), io::std_io_read_line);
        
        // Aliases for Lua/other languages
        self.register("print".to_string(), io::std_io_println);
        self.register("println".to_string(), io::std_io_println);
        
        self.register("std::fs::read_to_string".to_string(), fs::std_fs_read_to_string);
        self.register("std::fs::write".to_string(), fs::std_fs_write);
        self.register("std::fs::exists".to_string(), fs::std_fs_exists);
        self.register("std::fs::remove_file".to_string(), fs::std_fs_remove_file);
        self.register("std::fs::async_read_to_string".to_string(), fs::async_fs_read_to_string);
        self.register("std::fs::async_write".to_string(), fs::async_fs_write);

        // self.register("std::http::get".to_string(), http::std_http_get);
        // self.register("std::http::post".to_string(), http::std_http_post);
        // self.register("std::http::set_proxy".to_string(), http::std_http_set_proxy);

        // self.register("std::config::json::___parse".to_string(), json::std_json_parse);
        // self.register("std::config::json::___stringify".to_string(), json::std_json_stringify);

        // self.register("std::config::toml::___parse".to_string(), toml::std_toml_parse);
        // self.register("std::config::toml::___stringify".to_string(), toml::std_toml_stringify);

        // self.register("std::config::von::___parse".to_string(), von::std_von_parse);
        // self.register("std::config::von::___stringify".to_string(), von::std_von_stringify);

        // self.register("std::net::TcpStream::connect".to_string(), net::tcp_connect);
        // self.register("std::net::TcpStream::read".to_string(), net::tcp_read);
        // self.register("std::net::TcpStream::write".to_string(), net::tcp_write);
        // self.register("std::net::TcpStream::close".to_string(), net::tcp_close);
        // self.register("std::net::TcpListener::listen".to_string(), net::tcp_listen);
        // self.register("std::net::TcpListener::accept".to_string(), net::tcp_accept);
        // self.register("std::net::TcpListener::close".to_string(), net::tcp_close);

        // self.register("std::async::Async::delay".to_string(), async_ffi::async_delay);
        // self.register("std::async::Async::spawn".to_string(), async_ffi::async_spawn);
        // self.register("std::async::Async::await".to_string(), async_ffi::async_await);
        // self.register("std::async::Async::timeout".to_string(), async_ffi::async_timeout);

        self.register("std::gc::collect".to_string(), native_gc_collect);
        self.register("std::gc::stats".to_string(), native_gc_stats);

        // BigInt
        self.register_intrinsic(100, bigint::bigint_const);
        self.register_intrinsic(101, bigint::bigint_add);
        self.register_intrinsic(102, bigint::bigint_sub);
        self.register_intrinsic(103, bigint::bigint_mul);
        self.register_intrinsic(104, bigint::bigint_div);
        self.register_intrinsic(105, bigint::bigint_mod);
        self.register_intrinsic(106, bigint::bigint_neg);
        self.register_intrinsic(107, bigint::bigint_eq);
        self.register_intrinsic(108, bigint::bigint_ne);
        self.register_intrinsic(109, bigint::bigint_lt);
        self.register_intrinsic(110, bigint::bigint_le);
        self.register_intrinsic(111, bigint::bigint_gt);
        self.register_intrinsic(112, bigint::bigint_ge);
        self.register_intrinsic(113, bigint::bigint_to_i64);
        self.register_intrinsic(114, bigint::bigint_from_i64);
        self.register_intrinsic(115, bigint::bigint_to_string);

        // String
        self.register_intrinsic(120, string::string_concat);
        self.register_intrinsic(121, string::string_len_bytes);
        self.register_intrinsic(122, string::string_len_chars);
        self.register_intrinsic(123, string::string_eq);
        self.register_intrinsic(124, string::string_ne);
        self.register_intrinsic(125, string::string_lt);
        self.register_intrinsic(126, string::string_le);
        self.register_intrinsic(127, string::string_gt);
        self.register_intrinsic(128, string::string_ge);
        self.register_intrinsic(129, string::string_substr);

        // Float
        self.register_intrinsic(140, float::f32_add);
        self.register_intrinsic(141, float::f32_sub);
        self.register_intrinsic(142, float::f32_mul);
        self.register_intrinsic(143, float::f32_div);
        self.register_intrinsic(144, float::f32_neg);
        self.register_intrinsic(145, float::f32_eq);
        self.register_intrinsic(146, float::f32_ne);
        self.register_intrinsic(147, float::f32_lt);
        self.register_intrinsic(148, float::f32_le);
        self.register_intrinsic(149, float::f32_gt);
        self.register_intrinsic(150, float::f32_ge);
        self.register_intrinsic(151, float::f32_to_i32_s);
        self.register_intrinsic(152, float::f32_to_i32_u);
        self.register_intrinsic(153, float::f32_to_i64_s);
        self.register_intrinsic(154, float::f32_to_i64_u);
        self.register_intrinsic(155, float::f32_to_f64);

        self.register_intrinsic(160, float::f64_add);
        self.register_intrinsic(161, float::f64_sub);
        self.register_intrinsic(162, float::f64_mul);
        self.register_intrinsic(163, float::f64_div);
        self.register_intrinsic(164, float::f64_neg);
        self.register_intrinsic(165, float::f64_eq);
        self.register_intrinsic(166, float::f64_ne);
        self.register_intrinsic(167, float::f64_lt);
        self.register_intrinsic(168, float::f64_le);
        self.register_intrinsic(169, float::f64_gt);
        self.register_intrinsic(170, float::f64_ge);
        self.register_intrinsic(171, float::f64_to_i32_s);
        self.register_intrinsic(172, float::f64_to_i32_u);
        self.register_intrinsic(173, float::f64_to_i64_s);
        self.register_intrinsic(174, float::f64_to_i64_u);
        self.register_intrinsic(175, float::f64_to_f32);

        // Int32
        self.register_intrinsic(200, int::i32_add);
        self.register_intrinsic(201, int::i32_sub);
        self.register_intrinsic(202, int::i32_mul);
        self.register_intrinsic(203, int::i32_div_s);
        self.register_intrinsic(204, int::i32_div_u);
        self.register_intrinsic(205, int::i32_rem_s);
        self.register_intrinsic(206, int::i32_rem_u);
        self.register_intrinsic(207, int::i32_and);
        self.register_intrinsic(208, int::i32_or);
        self.register_intrinsic(209, int::i32_xor);
        self.register_intrinsic(210, int::i32_shl);
        self.register_intrinsic(211, int::i32_shr_s);
        self.register_intrinsic(212, int::i32_shr_u);
        self.register_intrinsic(213, int::i32_not);
        self.register_intrinsic(214, int::i32_neg);
        self.register_intrinsic(215, int::i32_eq);
        self.register_intrinsic(216, int::i32_ne);
        self.register_intrinsic(217, int::i32_lt_s);
        self.register_intrinsic(218, int::i32_lt_u);
        self.register_intrinsic(219, int::i32_le_s);
        self.register_intrinsic(220, int::i32_le_u);
        self.register_intrinsic(221, int::i32_gt_s);
        self.register_intrinsic(222, int::i32_gt_u);
        self.register_intrinsic(223, int::i32_ge_s);
        self.register_intrinsic(224, int::i32_ge_u);
        self.register_intrinsic(225, int::i32_to_f32_s);
        self.register_intrinsic(226, int::i32_to_f32_u);
        self.register_intrinsic(227, int::i32_to_f64_s);
        self.register_intrinsic(228, int::i32_to_f64_u);
        self.register_intrinsic(229, int::i32_extend64_s);
        self.register_intrinsic(230, int::i32_extend64_u);
        self.register_intrinsic(231, int::i32_trunc64_s);
        self.register_intrinsic(232, int::i32_trunc64_u);
        self.register_intrinsic(233, int::i32_add_sat_s);
        self.register_intrinsic(234, int::i32_add_sat_u);
        self.register_intrinsic(235, int::i32_sub_sat_s);
        self.register_intrinsic(236, int::i32_sub_sat_u);

        // Int64
        self.register_intrinsic(260, int::i64_add);
        self.register_intrinsic(261, int::i64_sub);
        self.register_intrinsic(262, int::i64_mul);
        self.register_intrinsic(263, int::i64_div_s);
        self.register_intrinsic(264, int::i64_div_u);
        self.register_intrinsic(265, int::i64_rem_s);
        self.register_intrinsic(266, int::i64_rem_u);
        self.register_intrinsic(267, int::i64_and);
        self.register_intrinsic(268, int::i64_or);
        self.register_intrinsic(269, int::i64_xor);
        self.register_intrinsic(270, int::i64_shl);
        self.register_intrinsic(271, int::i64_shr_s);
        self.register_intrinsic(272, int::i64_shr_u);
        self.register_intrinsic(273, int::i64_not);
        self.register_intrinsic(274, int::i64_neg);
        self.register_intrinsic(275, int::i64_eq);
        self.register_intrinsic(276, int::i64_ne);
        self.register_intrinsic(277, int::i64_lt_s);
        self.register_intrinsic(278, int::i64_lt_u);
        self.register_intrinsic(279, int::i64_le_s);
        self.register_intrinsic(280, int::i64_le_u);
        self.register_intrinsic(281, int::i64_gt_s);
        self.register_intrinsic(282, int::i64_gt_u);
        self.register_intrinsic(283, int::i64_ge_s);
        self.register_intrinsic(284, int::i64_ge_u);
        self.register_intrinsic(285, int::i64_to_f32_s);
        self.register_intrinsic(286, int::i64_to_f32_u);
        self.register_intrinsic(287, int::i64_to_f64_s);
        self.register_intrinsic(288, int::i64_to_f64_u);
        self.register_intrinsic(289, int::i64_add_sat_s);
        self.register_intrinsic(290, int::i64_add_sat_u);
    }
}

pub fn native_gc_collect(vm: &mut NyarVM, _args: &[Value]) -> FFIResult {
    unsafe {
        use nyar_gc::Trace;
        vm.gc.full_gc(|ctx| {
            nyar_gc::stack::scan_thread_roots(ctx);
            vm.trace(ctx);
        });
    }
    Ok(Value::null())
}

pub fn native_gc_stats(vm: &mut NyarVM, _args: &[Value]) -> FFIResult {
    let stats = Value::dyn_object(&vm.gc);
    if let Some(obj) = stats.try_as_dyn_object_mut() {
        obj.entries.insert(
            "allocated_bytes".to_string(),
            Value::int(vm.gc.allocated_bytes() as i64),
        );
        obj.entries.insert(
            "total_collections".to_string(),
            Value::int(
                vm.gc
                    .total_collections
                    .load(std::sync::atomic::Ordering::Relaxed) as i64,
            ),
        );
        obj.entries.insert(
            "threshold".to_string(),
            Value::int(vm.gc.threshold.load(std::sync::atomic::Ordering::Relaxed) as i64),
        );
    }
    Ok(stats)
}

pub fn native_add(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    let b = args[1].as_int();
    Ok(Value::int(a + b))
}

pub fn native_get_time(vm: &mut NyarVM, _args: &[Value]) -> FFIResult {
    let now = vm.platform.now_ms();
    Ok(Value::int(now as i64))
}

pub fn native_sleep(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let ms = args[0].as_int() as u64;
    vm.platform.sleep_ms(ms);
    Ok(Value::null())
}

pub fn native_print(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let output = args.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" ");
    vm.log(&output);
    Ok(Value::null())
}

pub fn native_println(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let output = args.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" ");
    vm.log(&format!("{}\n", output));
    Ok(Value::null())
}

pub fn native_exit(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let code = args[0].as_int() as i32;
    std::process::exit(code);
}

pub fn native_bit_and(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    let b = args[1].as_int();
    Ok(Value::int(a & b))
}

pub fn native_bit_or(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    let b = args[1].as_int();
    Ok(Value::int(a | b))
}

pub fn native_bit_xor(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    let b = args[1].as_int();
    Ok(Value::int(a ^ b))
}

pub fn native_bit_not(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    Ok(Value::int(!a))
}

pub fn native_bit_shl(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    let b = args[1].as_int();
    Ok(Value::int(a << b))
}

pub fn native_bit_shr(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].as_int();
    let b = args[1].as_int();
    Ok(Value::int(a >> b))
}

pub fn native_panic(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let msg = args.get(0).map(|v| v.to_string()).unwrap_or_else(|| "panic".to_string());
    panic!("{}", msg);
}

pub fn native_math_sin(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].to_f64();
    Ok(Value::float(a.sin()))
}

pub fn native_math_cos(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].to_f64();
    Ok(Value::float(a.cos()))
}

pub fn native_math_tan(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].to_f64();
    Ok(Value::float(a.tan()))
}

pub fn native_math_sqrt(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].to_f64();
    Ok(Value::float(a.sqrt()))
}

pub fn native_math_abs(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let a = args[0].to_f64();
    Ok(Value::float(a.abs()))
}

pub fn native_math_rand(_vm: &mut NyarVM, _args: &[Value]) -> FFIResult {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    Ok(Value::int(rng.r#gen::<i64>()))
}

pub fn native_mem_alloc(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let size = args[0].as_int() as usize;
    Ok(Value::bytes(vec![0u8; size], &vm.gc))
}

pub fn native_mem_free(_vm: &mut NyarVM, _args: &[Value]) -> FFIResult {
    Ok(Value::null())
}

pub fn native_mem_realloc(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let val = args[0];
    let new_size = args[2].as_int() as usize;
    if let Some(bytes) = val.try_as_bytes_mut() {
        bytes.data.resize(new_size, 0);
        Ok(val)
    } else {
        let ptr = args[0].as_int() as *mut u8;
        let old_size = args[1].as_int() as usize;
        let layout = std::alloc::Layout::from_size_align(old_size, 8)
            .map_err(|_| NyarError::RuntimeError("Invalid layout".to_string()))?;
        unsafe {
            let new_ptr = std::alloc::realloc(ptr, layout, new_size);
            Ok(Value::int(new_ptr as i64))
        }
    }
}

pub fn native_mem_set(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let ptr = args[0].as_raw_ptr();
    let val = args[1].as_int() as u8;
    let count = args[2].as_int() as usize;
    if !ptr.is_null() {
        unsafe {
            std::ptr::write_bytes(ptr, val, count);
        }
    }
    Ok(Value::null())
}

pub fn native_mem_copy(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let dest = args[0].as_raw_ptr();
    let src = args[1].as_raw_ptr();
    let count = args[2].as_int() as usize;
    if !dest.is_null() && !src.is_null() {
        unsafe {
            std::ptr::copy_nonoverlapping(src, dest, count);
        }
    }
    Ok(Value::null())
}

pub fn native_str_len(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    if let Some(s) = args[0].try_as_str() {
        return Ok(Value::int(s.len() as i64));
    }
    if let Some(b) = args[0].try_as_bytes() {
        return Ok(Value::int(b.data.len() as i64));
    }
    let ptr = args[0].as_raw_ptr() as *const i8;
    if ptr.is_null() {
        return Ok(Value::int(0));
    }
    unsafe {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        Ok(Value::int(len as i64))
    }
}

pub fn native_str_cmp(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let s1 = args[0].as_raw_ptr() as *const i8;
    let s2 = args[1].as_raw_ptr() as *const i8;
    if s1.is_null() || s2.is_null() {
        return Ok(Value::int(if s1 == s2 { 0 } else { 1 }));
    }
    unsafe {
        let mut i = 0;
        while *s1.add(i) != 0 && *s1.add(i) == *s2.add(i) {
            i += 1;
        }
        Ok(Value::int((*s1.add(i) - *s2.add(i)) as i64))
    }
}
