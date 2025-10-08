// Rust AST 到 HIR 的转换器
// 将Rust AST转换为@nyar/hir的HIR结构

import { 
    RustAstProgram, RustAstFunction, RustAstExpr, RustAstStmt, RustAstType, 
    RustAstExprBinary, RustAstExprLiteral, RustAstExprCall, RustAstExprPath, 
    RustAstExprBlock, RustAstExprIf, RustAstBinaryOp, RustAstUnaryOp
} from './index.js';

// 从 @nyar/hir 导入 HIR 类型
import { 
    HirProgram, HirFunction, HirExpr, HirStmt, HirType, HirBinaryOp, HirLiteral
} from '@nyar/hir';

export class AstToHirConverter {
    private var_counter = 0;

    convert_program(ast: RustAstProgram): HirProgram {
        const items: any[] = [];
        
        for (const item of ast.items) {
            if (item.type === 'Function') {
                items.push(this.convert_function(item as RustAstFunction));
            }
            // TODO: 支持更多Rust项类型（Struct, Enum, Module等）
        }

        return {
            type: 'Program',
            items
        };
    }

    private convert_function(func: RustAstFunction): HirFunction {
        const params = func.params.map(param => ({
            name: param.name,
            type: this.convert_type(param.type)
        }));

        const body = this.convert_block_expr(func.body);

        return {
            type: 'Function',
            name: func.name,
            params,
            return_type: this.convert_type(func.return_type),
            body
        };
    }

    private convert_type(ast_type: RustAstType): HirType {
        switch (ast_type.type) {
            case 'Primitive':
                return ast_type.name;
            case 'Reference':
                // 简化处理：引用类型转换为基本类型
                return this.convert_type(ast_type.element);
            case 'Pointer':
                // 简化处理：指针类型转换为基本类型
                return this.convert_type(ast_type.element);
            case 'Path':
                // 简化处理：路径类型转换为i32
                return 'i32';
            default:
                return 'i32';
        }
    }

    private convert_expr(ast_expr: RustAstExpr): HirExpr {
        switch (ast_expr.type) {
            case 'Binary':
                return this.convert_binary_expr(ast_expr as RustAstExprBinary);
            case 'Literal':
                return this.convert_literal_expr(ast_expr as RustAstExprLiteral);
            case 'Call':
                return this.convert_call_expr(ast_expr as RustAstExprCall);
            case 'Path':
                return this.convert_path_expr(ast_expr as RustAstExprPath);
            case 'Block':
                return this.convert_block_expr(ast_expr as RustAstExprBlock);
            case 'If':
                return this.convert_if_expr(ast_expr as RustAstExprIf);
            default:
                // 简化处理：其他表达式转换为字面量0
                return {
                    type: 'Literal',
                    value: 0,
                    literal_type: 'i32'
                };
        }
    }

    private convert_binary_expr(expr: RustAstExprBinary): HirExpr {
        return {
            type: 'Binary',
            op: this.convert_binary_op(expr.op),
            left: this.convert_expr(expr.left),
            right: this.convert_expr(expr.right)
        };
    }

    private convert_literal_expr(expr: RustAstExprLiteral): HirExpr {
        return {
            type: 'Literal',
            value: expr.value,
            literal_type: this.convert_type(expr.literal_type)
        };
    }

    private convert_call_expr(expr: RustAstExprCall): HirExpr {
        return {
            type: 'Call',
            func: this.convert_expr(expr.func),
            args: expr.args.map(arg => this.convert_expr(arg))
        };
    }

    private convert_path_expr(expr: RustAstExprPath): HirExpr {
        return {
            type: 'Path',
            name: expr.segments.join('::')
        };
    }

    private convert_block_expr(block: RustAstExprBlock): HirExpr {
        const stmts = block.stmts.map(stmt => this.convert_stmt(stmt));
        
        return {
            type: 'Block',
            stmts,
            expr: block.expr ? this.convert_expr(block.expr) : null
        };
    }

    private convert_if_expr(expr: RustAstExprIf): HirExpr {
        return {
            type: 'If',
            cond: this.convert_expr(expr.cond),
            then_expr: this.convert_block_expr(expr.then_expr),
            else_expr: expr.else_expr ? this.convert_expr(expr.else_expr) : null
        };
    }

    private convert_stmt(ast_stmt: RustAstStmt): HirStmt {
        switch (ast_stmt.type) {
            case 'Let':
                return {
                    type: 'Let',
                    name: (ast_stmt as any).pattern.name || `tmp_${this.var_counter++}`,
                    init: this.convert_expr(ast_stmt.init)
                };
            case 'Expr':
                // 表达式语句转换为let语句
                return {
                    type: 'Let',
                    name: `tmp_${this.var_counter++}`,
                    init: this.convert_expr((ast_stmt as any).expr)
                };
            default:
                // 其他语句转换为空的let语句
                return {
                    type: 'Let',
                    name: `tmp_${this.var_counter++}`,
                    init: {
                        type: 'Literal',
                        value: 0,
                        literal_type: 'i32'
                    }
                };
        }
    }

    private convert_binary_op(op: RustAstBinaryOp): HirBinaryOp {
        // 转换Rust二元操作符到HIR操作符
        switch (op) {
            case 'Add': return 'Add';
            case 'Sub': return 'Sub';
            case 'Mul': return 'Mul';
            case 'Div': return 'Div';
            case 'Eq': return 'Eq';
            case 'Ne': return 'Ne';
            case 'Lt': return 'Lt';
            case 'Le': return 'Le';
            case 'Gt': return 'Gt';
            case 'Ge': return 'Ge';
            case 'And': return 'And';
            case 'Or': return 'Or';
            default: return 'Add'; // 默认操作符
        }
    }
}

// 示例：转换阶乘函数
export function convert_factorial_example(): HirProgram {
    const converter = new AstToHirConverter();
    
    // 创建Rust AST的阶乘示例
    const factorial_ast = create_factorial_ast();
    
    return converter.convert_program(factorial_ast);
}

function create_factorial_ast(): RustAstProgram {
    // 创建Rust AST的阶乘函数
    return {
        type: 'Program',
        items: [{
            type: 'Function',
            name: 'factorial',
            params: [{
                name: 'n',
                type: {
                    type: 'Primitive',
                    name: 'i32'
                }
            }],
            return_type: {
                type: 'Primitive',
                name: 'i32'
            },
            body: [
                {
                    type: 'Let',
                    pattern: {
                        type: 'Binding',
                        name: 'result',
                        pattern: null
                    },
                    init: {
                        type: 'Literal',
                        value: 1,
                        literal_type: {
                            type: 'Primitive',
                            name: 'i32'
                        }
                    },
                    type_annotation: null
                },
                {
                    type: 'Expr',
                    expr: {
                        type: 'While',
                        cond: {
                            type: 'Binary',
                            op: 'Gt',
                            left: {
                                type: 'Path',
                                segments: ['n']
                            },
                            right: {
                                type: 'Literal',
                                value: 0,
                                literal_type: {
                                    type: 'Primitive',
                                    name: 'i32'
                                }
                            }
                        },
                        body: [
                            {
                                type: 'Expr',
                                expr: {
                                    type: 'Binary',
                                    op: 'Mul',
                                    left: {
                                        type: 'Path',
                                        segments: ['result']
                                    },
                                    right: {
                                        type: 'Path',
                                        segments: ['n']
                                    }
                                }
                            },
                            {
                                type: 'Expr',
                                expr: {
                                    type: 'Binary',
                                    op: 'Sub',
                                    left: {
                                        type: 'Path',
                                        segments: ['n']
                                    },
                                    right: {
                                        type: 'Literal',
                                        value: 1,
                                        literal_type: {
                                            type: 'Primitive',
                                            name: 'i32'
                                        }
                                    }
                                }
                            }
                        ]
                    }
                },
                {
                    type: 'Expr',
                    expr: {
                        type: 'Path',
                        segments: ['result']
                    }
                }
            ],
            visibility: 'pub'
        }]
    };
}