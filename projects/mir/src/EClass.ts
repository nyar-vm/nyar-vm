import type {AnyENode} from "./nodes/ENode";

export type EClassId = number | symbol;

export interface EClass {
    id: EClassId;
    nodes: AnyENode[];
    parents: Map<AnyENode, EClassId>;
}