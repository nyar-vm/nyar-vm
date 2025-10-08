// Rust-GC 示例语言 - HIR 定义
export interface HirProgram {
    type: 'Program';
    items: HirItem[];
}

export interface HirItem {
    type: 'Function';
}

export interface HirFunction extends HirItem {
    type: 'Function';
    name: string;
    params: HirParam[];
    returnType: HirType;
    body: HirExpr;
}

export interface HirParam {
    name: string;
    type: HirType;
}

export type HirType = 'i32' | 'bool';

export type HirExpr =
    | HirExprBinary
    | HirExprIf
    | HirExprLiteral
    | HirExprCall
    | HirExprPath
    | HirExprBlock;

export interface HirExprBinary {
    type: 'Binary';
    op: HirBinaryOp;
    left: HirExpr;
    right: HirExpr;
}

export type HirBinaryOp = 'Add' | 'Sub' | 'Mul' | 'Div' | 'Eq' | 'Ne' | 'Lt' | 'Gt';

export interface HirExprIf {
    type: 'If';
    cond: HirExpr;
    thenExpr: HirExpr;
    elseExpr: HirExpr;
}

export interface HirExprLiteral {
    type: 'Literal';
    value: number | boolean;
    literalType: HirType;
}

export interface HirExprCall {
    type: 'Call';
    func: HirExpr;
    args: HirExpr[];
}

export interface HirExprPath {
    type: 'Path';
    name: string;
}

export interface HirExprBlock {
    type: 'Block';
    stmts: HirStmt[];
    expr: HirExpr | null;
}

export type HirStmt = HirStmtLet;

export interface HirStmtLet {
    type: 'Let';
    name: string;
    init: HirExpr;
}

// 辅助函数
export function createHirProgram(items: HirItem[]): HirProgram {
    return {type: 'Program', items};
}

export function createHirFunction(
    name: string,
    params: HirParam[],
    returnType: HirType,
    body: HirExpr,
): HirFunction {
    return {
        type: 'Function',
        name,
        params,
        returnType,
        body,
    };
}

export function createHirParam(name: string, type: HirType): HirParam {
    return {name, type};
}

export function createHirExprBinary(op: HirBinaryOp, left: HirExpr, right: HirExpr): HirExprBinary {
    return {type: 'Binary', op, left, right};
}

export function createHirExprIf(cond: HirExpr, thenExpr: HirExpr, elseExpr: HirExpr): HirExprIf {
    return {type: 'If', cond, thenExpr, elseExpr};
}

export function createHirExprLiteral(value: number | boolean, literalType: HirType): HirExprLiteral {
    return {type: 'Literal', value, literalType};
}

export function createHirExprCall(func: HirExpr, args: HirExpr[]): HirExprCall {
    return {type: 'Call', func, args};
}

export function createHirExprPath(name: string): HirExprPath {
    return {type: 'Path', name};
}

export function createHirExprBlock(stmts: HirStmt[], expr: HirExpr | null = null): HirExprBlock {
    return {type: 'Block', stmts, expr};
}

export function createHirStmtLet(name: string, init: HirExpr): HirStmtLet {
    return {type: 'Let', name, init};
}