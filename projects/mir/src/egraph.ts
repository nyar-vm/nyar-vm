// E-graph核心类型定义
export type EClassId = number | symbol;

export interface ENode {
    type: 'CONSTANT' | 'SYMBOL' | 'CALL' | 'LAMBDA';
}

export interface ConstantNode extends ENode {
    type: 'CONSTANT';
    value: any;
}

export interface SymbolNode extends ENode {
    type: 'SYMBOL';
    name: string;
}

export interface CallNode extends ENode {
    type: 'CALL';
    callee: EClassId;
    args: EClassId[];
}

export interface LambdaNode extends ENode {
    type: 'LAMBDA';
    params: string[];
    body: EClassId;
}

export type AnyENode = ConstantNode | SymbolNode | CallNode | LambdaNode;

export interface EClass {
    id: EClassId;
    nodes: AnyENode[];
    parents: Map<AnyENode, EClassId>;
}

export interface EGraph {
    eclasses: Map<EClassId, EClass>;
    worklist: EClassId[];
    nextId: number;
}

export function createEGraph(): EGraph {
    return {
        eclasses: new Map(),
        worklist: [],
        nextId: 0
    };
}