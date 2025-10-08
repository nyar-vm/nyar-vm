import {EClassId} from "../EClass";

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