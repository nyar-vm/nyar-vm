// HIR 到 MIR 的转换
// 通过 @nyar/vm 提供的编译管线进行转换

import { EGraph } from '@nyar/hir';
import { compile } from '@nyar/vm';
import { RustAstProgram } from './index.js';
import { AstToHirConverter } from './ast_to_hir.js';

export function rust_ast_to_mir(ast: RustAstProgram): EGraph {
    // 第一步：将Rust AST转换为HIR
    const converter = new AstToHirConverter();
    const hir_program = converter.convert_program(ast);
    
    // 第二步：使用@nyar/vm的编译管线将HIR转换为MIR
    const result = compile(hir_program, {
        target: 'mir',
        optimizations: ['constant-folding', 'dead-code-elimination']
    });
    
    if (!result.success) {
        throw new Error(`Compilation failed: ${result.errors.join(', ')}`);
    }
    
    return result.mir as EGraph;
}

// 示例：转换阶乘函数
export function convert_factorial_example(): EGraph {
    // 创建Rust AST的阶乘示例
    const factorial_ast = create_factorial_ast();
    
    return rust_ast_to_mir(factorial_ast);
}

function create_factorial_ast(): RustAstProgram {
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