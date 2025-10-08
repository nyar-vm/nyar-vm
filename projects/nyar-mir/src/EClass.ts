import type { AnyNode } from './nodes/ENode';

export type EClassId = number | symbol;

export interface EClass {
    id: EClassId;
    nodes: AnyNode[];
    parents: Map<AnyNode, EClassId>;
}
