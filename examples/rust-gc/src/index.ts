// Rust-GC 示例语言 - Rust AST 定义
// 这是源语言前端，负责将Rust代码解析为AST，然后转换为HIR

export interface RustAstProgram {
    type: 'Program';
    items: RustAstItem[];
}

export interface RustAstItem {
    type: 'Function' | 'Struct' | 'Enum' | 'Module';
}

export interface RustAstFunction extends RustAstItem {
    type: 'Function';
    name: string;
    params: RustAstParam[];
    return_type: RustAstType;
    body: RustAstBlock;
    visibility?: 'pub';
}

export interface RustAstParam {
    name: string;
    type: RustAstType;
}

export type RustAstType =
    | RustAstTypePrimitive
    | RustAstTypeReference
    | RustAstTypePointer
    | RustAstTypePath;

export interface RustAstTypePrimitive {
    type: 'Primitive';
    name: 'i32' | 'i64' | 'u32' | 'u64' | 'bool' | 'str' | 'char';
}

export interface RustAstTypeReference {
    type: 'Reference';
    lifetime?: string;
    mutable: boolean;
    element: RustAstType;
}

export interface RustAstTypePointer {
    type: 'Pointer';
    mutable: boolean;
    element: RustAstType;
}

export interface RustAstTypePath {
    type: 'Path';
    segments: string[];
}

export type RustAstExpr =
    | RustAstExprBinary
    | RustAstExprUnary
    | RustAstExprIf
    | RustAstExprWhile
    | RustAstExprFor
    | RustAstExprLoop
    | RustAstExprMatch
    | RustAstExprLiteral
    | RustAstExprCall
    | RustAstExprMethodCall
    | RustAstExprField
    | RustAstExprPath
    | RustAstExprBlock
    | RustAstExprStruct
    | RustAstExprTuple
    | RustAstExprArray
    | RustAstExprIndex
    | RustAstExprClosure;

export interface RustAstExprBinary {
    type: 'Binary';
    op: RustAstBinaryOp;
    left: RustAstExpr;
    right: RustAstExpr;
}

export type RustAstBinaryOp = 
    | 'Add' | 'Sub' | 'Mul' | 'Div' | 'Rem'  // Arithmetic
    | 'Eq' | 'Ne' | 'Lt' | 'Le' | 'Gt' | 'Ge'  // Comparison
    | 'And' | 'Or'  // Logical
    | 'BitAnd' | 'BitOr' | 'BitXor' | 'Shl' | 'Shr'  // Bitwise
    ;

export interface RustAstExprUnary {
    type: 'Unary';
    op: RustAstUnaryOp;
    expr: RustAstExpr;
}

export type RustAstUnaryOp = 'Not' | 'Neg' | 'Deref' | 'Ref' | 'RefMut';

export interface RustAstExprIf {
    type: 'If';
    cond: RustAstExpr;
    then_expr: RustAstBlock;
    else_expr: RustAstExpr | RustAstBlock | null;
}

export interface RustAstExprWhile {
    type: 'While';
    cond: RustAstExpr;
    body: RustAstBlock;
}

export interface RustAstExprFor {
    type: 'For';
    pattern: RustAstPattern;
    iter: RustAstExpr;
    body: RustAstBlock;
}

export interface RustAstExprLoop {
    type: 'Loop';
    body: RustAstBlock;
}

export interface RustAstExprMatch {
    type: 'Match';
    expr: RustAstExpr;
    arms: RustAstMatchArm[];
}

export interface RustAstMatchArm {
    pattern: RustAstPattern;
    guard: RustAstExpr | null;
    body: RustAstExpr;
}

export type RustAstPattern =
    | RustAstPatternWildcard
    | RustAstPatternLiteral
    | RustAstPatternPath
    | RustAstPatternStruct
    | RustAstPatternTuple
    | RustAstPatternBinding;

export interface RustAstPatternWildcard {
    type: 'Wildcard';
}

export interface RustAstPatternLiteral {
    type: 'Literal';
    value: number | boolean | string;
}

export interface RustAstPatternPath {
    type: 'Path';
    segments: string[];
}

export interface RustAstPatternStruct {
    type: 'Struct';
    path: RustAstPatternPath;
    fields: RustAstPatternField[];
}

export interface RustAstPatternField {
    name: string;
    pattern: RustAstPattern;
}

export interface RustAstPatternTuple {
    type: 'Tuple';
    elements: RustAstPattern[];
}

export interface RustAstPatternBinding {
    type: 'Binding';
    name: string;
    pattern: RustAstPattern | null;
}

export interface RustAstExprLiteral {
    type: 'Literal';
    value: number | boolean | string;
    literal_type: RustAstType;
}

export interface RustAstExprCall {
    type: 'Call';
    func: RustAstExpr;
    args: RustAstExpr[];
}

export interface RustAstExprMethodCall {
    type: 'MethodCall';
    receiver: RustAstExpr;
    method: string;
    args: RustAstExpr[];
    type_args: RustAstType[];
}

export interface RustAstExprField {
    type: 'Field';
    expr: RustAstExpr;
    field: string;
}

export interface RustAstExprPath {
    type: 'Path';
    segments: string[];
}

export interface RustAstExprBlock {
    type: 'Block';
    stmts: RustAstStmt[];
    expr: RustAstExpr | null;
}

export interface RustAstExprStruct {
    type: 'Struct';
    path: RustAstExprPath;
    fields: RustAstExprField[];
}

export interface RustAstExprField {
    name: string;
    value: RustAstExpr;
}

export interface RustAstExprTuple {
    type: 'Tuple';
    elements: RustAstExpr[];
}

export interface RustAstExprArray {
    type: 'Array';
    elements: RustAstExpr[];
}

export interface RustAstExprIndex {
    type: 'Index';
    expr: RustAstExpr;
    index: RustAstExpr;
}

export interface RustAstExprClosure {
    type: 'Closure';
    params: RustAstParam[];
    return_type: RustAstType | null;
    body: RustAstExpr;
}

export type RustAstStmt =
    | RustAstStmtLet
    | RustAstStmtExpr
    | RustAstStmtItem
    | RustAstStmtSemi;

export interface RustAstStmtLet {
    type: 'Let';
    pattern: RustAstPattern;
    init: RustAstExpr;
    type_annotation: RustAstType | null;
}

export interface RustAstStmtExpr {
    type: 'Expr';
    expr: RustAstExpr;
}

export interface RustAstStmtItem {
    type: 'Item';
    item: RustAstItem;
}

export interface RustAstStmtSemi {
    type: 'Semi';
    expr: RustAstExpr;
}

export type RustAstBlock = RustAstStmt[];

// AST 辅助函数
export function create_ast_program(items: RustAstItem[]): RustAstProgram {
    return { type: 'Program', items };
}

export function create_ast_function(
    name: string,
    params: RustAstParam[],
    return_type: RustAstType,
    body: RustAstBlock,
    visibility?: 'pub'
): RustAstFunction {
    return {
        type: 'Function',
        name,
        params,
        return_type,
        body,
        visibility,
    };
}