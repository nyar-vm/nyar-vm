// 测试文件 - 验证HIR到MIR的转换
import { convert_factorial_example } from './hir_to_mir.js';
import { EGraph } from '@nyar/hir';

export function run_test(): void {
    // 创建 EGraph 实例
    const egraph = new EGraph();
    
    // 转换阶乘示例
    convert_factorial_example(egraph);
    
    // 输出结果
    console.log('Factorial example converted successfully');
}

// 运行测试
run_test();