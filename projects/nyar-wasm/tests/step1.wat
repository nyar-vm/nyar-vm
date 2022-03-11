(module
  ;; 导出一个全局变量
  (global $data (mut i32) (i32.const 42))

  ;; 导出一个函数，返回全局变量的值
  (func (export "get_data") (result i32)
    (global.get $data))
)