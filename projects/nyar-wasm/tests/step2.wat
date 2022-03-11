(module
  ;; 导入第一个示例的全局变量
  (import "data" "data" (global $data (mut i32)))

  ;; 导出一个函数，使用第一个示例的全局变量并增加它的值
  (func (export "increment_data") (result i32)
    (global.get $data)
    (i32.const 1)
    (i32.add)
    (global.set $data)
    (global.get $data)
  )
)