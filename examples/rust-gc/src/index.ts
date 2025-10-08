// Rust-GC 示例语言 - HIR 定义
export interface RustProgram {
    type: 'Program';
    items: RustItem[];
}

export interface RustItem {
    type: 'Function';
}

export interface RustFunction extends RustItem {
    type: 'Function';
    name: string;
    params: HirParam[];
    return_type: HirType;
    body: HirExpr;
}

export interface RustParam {
    name: string;
    type: HirType;
}

export type RustType = 'i32' | 'bool';

export type RustExpr =
    | HirExprBinary
    | HirExprIf
    | HirExprLiteral
    | HirExprCall
    | HirExprPath
    | HirExprBlock;

export interface RustExprBinary {
    type: 'Binary';
    op: HirBinaryOp;
    left: HirExpr;
    right: HirExpr;
}

export type RustBinaryOp = 'Add' | 'Sub' | 'Mul' | 'Div' | 'Eq' | 'Ne' | 'Lt' | 'Gt';

export interface RustExprIf {
    type: 'If';
    cond: HirExpr;
    then_expr: HirExpr;
    else_expr: HirExpr;
}

export interface RustExprLiteral {
    type: 'Literal';
    value: number | boolean;
    literal_type: HirType;
}

export interface RustExprCall {
    type: 'Call';
    func: HirExpr;
    args: HirExpr[];
}

export interface RustExprPath {
    type: 'Path';
    name: string;
}

export interface RustExprBlock {
    type: 'Block';
    stmts: HirStmt[];
    expr: HirExpr | null;
}

export type RustStmt = HirStmtLet;

export interface RustStmtLet {
    type: 'Let';
    name: string;
    init: HirExpr;
}

// 辅助函数
export function create_hir_program(items: RustItem[]): RustProgram {
    return {type: 'Program', items};
}

export function create_hir_function(
    name: string,
    params: HirParam[],
    return_type: HirType,
    body: HirExpr,
): HirFunction {
    return {
        type: 'Function',
        name,
        params,
        return_type,
        body,
    };
}

export function create_hir_param(name: string, type: HirType): HirParam {
    return {name, type};
}

export function create_hir_expr_binary(op: HirBinaryOp, left: HirExpr, right: HirExpr): HirExprBinary {
    return {type: 'Binary', op, left, right};
}

export function create_hir_expr_if(cond: HirExpr, then_expr: HirExpr, else_expr: HirExpr): HirExprIf {
    return {type: 'If', cond, then_expr, else_expr};
}

export function create_hir_expr_literal(value: number | boolean, literal_type: HirType): HirExprLiteral {
    return {type: 'Literal', value, literal_type};
}

export function create_hir_expr_call(func: HirExpr, args: HirExpr[]): HirExprCall {
    return {type: 'Call', func, args};
}

export function create_hir_expr_path(name: string): HirExprPath {
    return {type: 'Path', name};
}

export function create_hir_expr_block(stmts: HirStmt[], expr: HirExpr | null = null): HirExprBlock {
    return {type: 'Block', stmts, expr};
}

export function create_hir_stmt_let(name: string, init: HirExpr): HirStmtLet {
    return {type: 'Let', name, init};
}