// E-graph核心类型定义
import {EClass, EClassId} from "./EClass";

export type {ENode, ConstantNode, SymbolNode, CallNode, LambdaNode, AnyENode} from './nodes/ENode';

export interface EGraph {
    e_classes: Map<EClassId, EClass>;
    worklist: EClassId[];
    next_id: number;
}

export function create_graph(): EGraph {
    return {
        e_classes: new Map(),
        worklist: [],
        next_id: 0
    };
}