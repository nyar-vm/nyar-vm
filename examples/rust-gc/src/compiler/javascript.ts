import { EGraph } from '@nyar/hir';
import { ENode, ConstantNode, SymbolNode, CallNode } from '@nyar/mir';

/**
 * JavaScript后端编译器
 * 将MIR E-graph转换为JavaScript代码
 */
export class JavaScriptCompiler {
    private indent_level = 0;
    private indent_size = 4;
    
    /**
     * 编译MIR到JavaScript
     */
    compile(graph: EGraph): string {
        const functions: string[] = [];
        
        // 遍历E-graph中的所有函数
        for (const [eclass_id, eclass] of graph.e_classes) {
            for (const enode of eclass.nodes) {
                if (enode instanceof CallNode && enode.func === 'core.function') {
                    const func_code = this.compile_function(enode);
                    functions.push(func_code);
                }
            }
        }
        
        // 生成运行时支持代码
        const runtime = this.generate_runtime();
        
        // 组合最终代码
        return `${runtime}\n\n${functions.join('\n\n')}`;
    }
    
    /**
     * 编译函数
     */
    private compile_function(func_node: CallNode): string {
        const [name_node, params_node, body_node] = func_node.args;
        
        let func_name = 'anonymous';
        if (name_node instanceof SymbolNode) {
            func_name = name_node.name;
        }
        
        let params: string[] = [];
        if (params_node instanceof CallNode && params_node.func === 'core.params') {
            params = params_node.args.map(arg => {
                if (arg instanceof SymbolNode) {
                    return arg.name;
                }
                return 'arg';
            });
        }
        
        const body_code = this.compile_expression(body_node);
        
        return `function ${func_name}(${params.join(', ')}) {\n${this.indent()}${body_code}\n}`;
    }
    
    /**
     * 编译表达式
     */
    private compile_expression(node: ENode): string {
        if (node instanceof ConstantNode) {
            return this.compile_constant(node);
        } else if (node instanceof SymbolNode) {
            return node.name;
        } else if (node instanceof CallNode) {
            return this.compile_call(node);
        }
        
        return 'undefined';
    }
    
    /**
     * 编译常量
     */
    private compile_constant(node: ConstantNode): string {
        const value = node.value;
        if (typeof value === 'string') {
            return `"${value.replace(/"/g, '\\"')}"`;
        } else if (typeof value === 'boolean') {
            return value.toString();
        } else if (typeof value === 'number') {
            return value.toString();
        } else if (value === null) {
            return 'null';
        }
        return 'undefined';
    }
    
    /**
     * 编译函数调用
     */
    private compile_call(node: CallNode): string {
        const args_code = node.args.map(arg => this.compile_expression(arg)).join(', ');
        
        // 处理核心操作
        if (node.func.startsWith('core.ops.')) {
            const op = node.func.substring(9); // 移除 'core.ops.' 前缀
            return this.compile_operation(op, node.args);
        }
        
        // 处理条件语句
        if (node.func === 'core.if') {
            const [cond, then_expr, else_expr] = node.args;
            return `(${this.compile_expression(cond)} ? ${this.compile_expression(then_expr)} : ${this.compile_expression(else_expr)})`;
        }
        
        // 普通函数调用
        return `${node.func}(${args_code})`;
    }
    
    /**
     * 编译操作符
     */
    private compile_operation(op: string, args: ENode[]): string {
        if (args.length === 2) {
            const [left, right] = args;
            const left_code = this.compile_expression(left);
            const right_code = this.compile_expression(right);
            
            switch (op) {
                case 'add': return `(${left_code} + ${right_code})`;
                case 'sub': return `(${left_code} - ${right_code})`;
                case 'mul': return `(${left_code} * ${right_code})`;
                case 'div': return `(${left_code} / ${right_code})`;
                case 'eq': return `(${left_code} === ${right_code})`;
                case 'ne': return `(${left_code} !== ${right_code})`;
                case 'lt': return `(${left_code} < ${right_code})`;
                case 'gt': return `(${left_code} > ${right_code})`;
                case 'le': return `(${left_code} <= ${right_code})`;
                case 'ge': return `(${left_code} >= ${right_code})`;
            }
        }
        
        return 'undefined';
    }
    
    /**
     * 生成运行时支持代码
     */
    private generate_runtime(): string {
        return `// Nyar Runtime Support
const core = {
    ops: {
        add: (a, b) => a + b,
        sub: (a, b) => a - b,
        mul: (a, b) => a * b,
        div: (a, b) => a / b,
        eq: (a, b) => a === b,
        ne: (a, b) => a !== b,
        lt: (a, b) => a < b,
        gt: (a, b) => a > b,
        le: (a, b) => a <= b,
        ge: (a, b) => a >= b,
    }
};`;
    }
    
    /**
     * 缩进辅助
     */
    private indent(): string {
        return ' '.repeat(this.indent_size * this.indent_level);
    }
}

/**
 * 编译MIR到JavaScript的便捷函数
 */
export function compile_to_javascript(graph: EGraph): string {
    const compiler = new JavaScriptCompiler();
    return compiler.compile(graph);
}