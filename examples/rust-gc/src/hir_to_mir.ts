// HIR 到 MIR 的规范化转换器
import {
    HirBinaryOp,
    HirExpr,
    HirExprBinary,
    HirExprBlock,
    HirExprCall,
    HirExprIf,
    HirExprLiteral,
    HirExprPath,
    HirFunction,
    HirProgram,
    HirStmtLet,
} from './index.js';

import { EGraph, create_graph, EClassId, EClass } from '../../projects/mir/src/EGraph.js';
import { AnyNode, ConstantNode, SymbolNode, CallNode, LambdaNode } from '../../projects/mir/src/nodes/ENode.js';

// 核心库函数符号
export const CORE_SYMBOLS = {
    // 算术运算
    'core.ops.add': 'core.ops.add',
    'core.ops.sub': 'core.ops.sub',
    'core.ops.mul': 'core.ops.mul',
    'core.ops.div': 'core.ops.div',

    // 比较运算
    'core.ops.eq': 'core.ops.eq',
    'core.ops.ne': 'core.ops.ne',
    'core.ops.lt': 'core.ops.lt',
    'core.ops.gt': 'core.ops.gt',

    // 控制流
    'core.control.if': 'core.control.if',
};

// 操作符到核心符号的映射
const BINARY_OP_TO_SYMBOL: Record<HirBinaryOp, string> = {
    'Add': CORE_SYMBOLS['core.ops.add'],
    'Sub': CORE_SYMBOLS['core.ops.sub'],
    'Mul': CORE_SYMBOLS['core.ops.mul'],
    'Div': CORE_SYMBOLS['core.ops.div'],
    'Eq': CORE_SYMBOLS['core.ops.eq'],
    'Ne': CORE_SYMBOLS['core.ops.ne'],
    'Lt': CORE_SYMBOLS['core.ops.lt'],
    'Gt': CORE_SYMBOLS['core.ops.gt'],
};

// MIR 转换器类
export class HirToMirConverter {
    private graph: EGraph;
    private symbol_map: Map<string, EClassId> = new Map();
    private lambda_counter = 0;

    constructor() {
        this.graph = create_graph();
        this.initialize_core_symbols();
    }

    // 初始化核心符号
    private initialize_core_symbols(): void {
        Object.values(CORE_SYMBOLS).forEach(symbol => {
            this.create_symbol_node(symbol);
        });
    }

    // 转换整个程序
    public convert_program(program: HirProgram): EGraph {
        program.items.forEach(item => {
            if (item.type === 'Function') {
                this.convert_function(item);
            }
        });

        return this.graph;
    }

    // 转换函数
    private convert_function(func: HirFunction): void {
        // 创建函数符号
        const func_symbol_id = this.create_symbol_node(func.name);

        // 创建lambda节点表示函数体
        const body_id = this.convert_expr(func.body);

        // 创建lambda节点
        const lambda_node: LambdaNode = {
            type: 'LAMBDA',
            params: func.params.map(param => param.name),
            body: body_id,
        };

        const lambda_id = this.add_node(lambda_node);

        // 将函数符号与lambda关联
        this.merge_classes(func_symbol_id, lambda_id);
    }

    // 转换表达式
    private convert_expr(expr: HirExpr): EClassId {
        switch (expr.type) {
            case 'Binary':
                return this.convert_binary_expr(expr);
            case 'If':
                return this.convert_if_expr(expr);
            case 'Literal':
                return this.convert_literal_expr(expr);
            case 'Call':
                return this.convert_call_expr(expr);
            case 'Path':
                return this.convert_path_expr(expr);
            case 'Block':
                return this.convert_block_expr(expr);
            default:
                throw new Error(`Unsupported expression type: ${(expr as any).type}`);
        }
    }

    // 转换二元表达式为CALL节点
    private convert_binary_expr(expr: HirExprBinary): EClassId {
        const left_id = this.convert_expr(expr.left);
        const right_id = this.convert_expr(expr.right);

        // 获取对应的核心符号
        const symbol_name = BINARY_OP_TO_SYMBOL[expr.op];
        const symbol_id = this.symbol_map.get(symbol_name);

        if (!symbol_id) {
            throw new Error(`Symbol not found: ${symbol_name}`);
        }

        // 创建CALL节点
        const call_node: CallNode = {
            type: 'CALL',
            callee: symbol_id,
            args: [left_id, right_id],
        };

        return this.add_node(call_node);
    }

    // 转换if表达式为CALL节点
    private convert_if_expr(expr: HirExprIf): EClassId {
        const cond_id = this.convert_expr(expr.cond);

        // 创建then分支的lambda
        const then_lambda_id = this.create_lambda([], expr.then_expr);

        // 创建else分支的lambda
        const else_lambda_id = this.create_lambda([], expr.else_expr);

        // 获取if控制符号
        const if_symbol_id = this.symbol_map.get(CORE_SYMBOLS['core.control.if']);

        if (!if_symbol_id) {
            throw new Error('if symbol not found');
        }

        // 创建CALL节点: if(cond, then_lambda, else_lambda)
        const call_node: CallNode = {
            type: 'CALL',
            callee: if_symbol_id,
            args: [cond_id, then_lambda_id, else_lambda_id],
        };

        return this.add_node(call_node);
    }

    // 转换字面量表达式
    private convert_literal_expr(expr: HirExprLiteral): EClassId {
        const constant_node: ConstantNode = {
            type: 'CONSTANT',
            value: expr.value,
        };

        return this.add_node(constant_node);
    }

    // 转换调用表达式
    private convert_call_expr(expr: HirExprCall): EClassId {
        const func_id = this.convert_expr(expr.func);
        const arg_ids = expr.args.map(arg => this.convert_expr(arg));

        const call_node: CallNode = {
            type: 'CALL',
            callee: func_id,
            args: arg_ids,
        };

        return this.add_node(call_node);
    }

    // 转换路径表达式
    private convert_path_expr(expr: HirExprPath): EClassId {
        const symbol_id = this.symbol_map.get(expr.name);

        if (!symbol_id) {
            throw new Error(`Symbol not found: ${expr.name}`);
        }

        return symbol_id;
    }

    // 转换块表达式
    private convert_block_expr(expr: HirExprBlock): EClassId {
        // 转换所有语句
        expr.stmts.forEach(stmt => {
            if (stmt.type === 'Let') {
                this.convert_let_stmt(stmt);
            }
        });

        // 返回最后一个表达式的结果
        return expr.expr ? this.convert_expr(expr.expr) : this.create_unit_literal();
    }

    // 转换let语句
    private convert_let_stmt(stmt: HirStmtLet): void {
        const init_id = this.convert_expr(stmt.init);

        // 创建符号并绑定到初始化值
        this.create_symbol_node(stmt.name);
        const symbol_id = this.symbol_map.get(stmt.name);

        if (symbol_id) {
            this.merge_classes(symbol_id, init_id);
        }
    }

    // 创建符号节点
    private create_symbol_node(name: string): EClassId {
        if (this.symbol_map.has(name)) {
            return this.symbol_map.get(name)!;
        }

        const symbol_node: SymbolNode = {
            type: 'SYMBOL',
            name,
        };

        const symbol_id = this.add_node(symbol_node);
        this.symbol_map.set(name, symbol_id);

        return symbol_id;
    }

    // 创建lambda节点
    private create_lambda(params: string[], body: HirExpr): EClassId {
        const body_id = this.convert_expr(body);

        const lambda_node: LambdaNode = {
            type: 'LAMBDA',
            params,
            body: body_id,
        };

        return this.add_node(lambda_node);
    }

    // 创建unit字面量
    private create_unit_literal(): EClassId {
        const unit_node: ConstantNode = {
            type: 'CONSTANT',
            value: null,
        };

        return this.add_node(unit_node);
    }

    // 添加节点到图
    private add_node(node: AnyNode): EClassId {
        // 简化实现：直接创建新的EClass
        const class_id = this.graph.next_id++;
        const e_class: EClass = {
            id: class_id,
            nodes: [node],
            parents: new Set(),
        };

        this.graph.e_classes.set(class_id, e_class);
        return class_id;
    }

    // 合并两个类
    private merge_classes(id1: EClassId, id2: EClassId): void {
        const class1 = this.graph.e_classes.get(id1);
        const class2 = this.graph.e_classes.get(id2);

        if (class1 && class2) {
            // 简化实现：将class2的节点合并到class1
            class1.nodes.push(...class2.nodes);
            this.graph.e_classes.delete(id2);
        }
    }
}

// 示例：转换阶乘函数
export function convert_factorial_example(): EGraph {
    const converter = new HirToMirConverter();

    // 创建阶乘函数的HIR表示
    const factorial_func = {
        type: 'Function',
        name: 'factorial',
        params: [
            {name: 'n', type: 'i32' as const}
        ],
        return_type: 'i32' as const,
        body: {
            type: 'If',
            cond: {
                type: 'Binary',
                op: 'Eq',
                left: {type: 'Path', name: 'n'},
                right: {type: 'Literal', value: 0, literal_type: 'i32'}
            },
            then_expr: {type: 'Literal', value: 1, literal_type: 'i32'},
            else_expr: {
                type: 'Binary',
                op: 'Mul',
                left: {type: 'Path', name: 'n'},
                right: {
                    type: 'Call',
                    func: {type: 'Path', name: 'factorial'},
                    args: [{
                        type: 'Binary',
                        op: 'Sub',
                        left: {type: 'Path', name: 'n'},
                        right: {type: 'Literal', value: 1, literal_type: 'i32'}
                    }]
                }
            }
        }
    };

    const program = {
        type: 'Program',
        items: [factorial_func]
    };

    return converter.convert_program(program);
}