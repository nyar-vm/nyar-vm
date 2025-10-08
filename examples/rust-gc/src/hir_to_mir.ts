import { EGraph } from '@nyar/hir';
import { ENode, ConstantNode, SymbolNode, CallNode } from '@nyar/mir';

/**
 * HIR到MIR转换器
 * 将Rust-GC的HIR结构转换为MIR的E-graph表示
 */
export function convert_factorial_example(graph: EGraph): void {
    // 创建阶乘函数的MIR表示
    // fn factorial(n: i32) -> i32 {
    //     if n <= 1 { 1 } else { n * factorial(n - 1) }
    // }

    // 创建常量节点
    const const_1 = graph.add(new ConstantNode(1));
    const const_0 = graph.add(new ConstantNode(0));
    
    // 创建符号节点（函数参数）
    const n_param = graph.add(new SymbolNode('n'));
    
    // 创建比较操作：n <= 1
    const le_call = graph.add(new CallNode('core.ops.le', [n_param, const_1]));
    
    // 创建递归调用：factorial(n - 1)
    const sub_call = graph.add(new CallNode('core.ops.sub', [n_param, const_1]));
    const recursive_call = graph.add(new CallNode('factorial', [sub_call]));
    
    // 创建乘法操作：n * factorial(n - 1)
    const mul_call = graph.add(new CallNode('core.ops.mul', [n_param, recursive_call]));
    
    // 创建条件表达式：if n <= 1 { 1 } else { n * factorial(n - 1) }
    const if_expr = graph.add(new CallNode('core.if', [le_call, const_1, mul_call]));
    
    // 创建函数节点
    const factorial_func = graph.add(new CallNode('core.function', [
        graph.add(new SymbolNode('factorial')),
        graph.add(new CallNode('core.params', [n_param])),
        if_expr
    ]));
    
    // 将函数添加到全局作用域
    graph.add(new CallNode('core.global', [
        graph.add(new SymbolNode('factorial')),
        factorial_func
    ]));
}

/**
 * 通用的HIR到MIR转换函数
 */
export function hir_to_mir(hir_program: any): EGraph {
    const graph = new EGraph();
    
    // 这里可以实现完整的HIR到MIR转换逻辑
    // 目前先使用示例函数
    convert_factorial_example(graph);
    
    return graph;
}