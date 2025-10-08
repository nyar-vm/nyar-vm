import {AnyENode, CallNode, ConstantNode, EClass, EClassId, EGraph, LambdaNode, SymbolNode} from './egraph.js';

/**
 * 将ENode添加到E-graph中
 * 如果同样的ENode已存在，则返回已有的EClassId
 */
export function add_node(graph: EGraph, node: AnyENode): EClassId {
    // 首先检查是否已存在相同的节点
    for (const [existing_id, eclass] of graph.e_classes) {
        for (const existing_node of eclass.nodes) {
            if (nodes_equal(existing_node, node)) {
                return existing_id;
            }
        }
    }

    // 创建新的E-class
    const new_id = graph.next_id++;
    const new_eclass: EClass = {
        id: new_id,
        nodes: [node],
        parents: new Map()
    };

    graph.e_classes.set(new_id, new_eclass);
    return new_id;
}

/**
 * 合并两个E-class
 * 这是等价饱和的核心操作
 */
export function merge_classes(graph: EGraph, id1: EClassId, id2: EClassId): EClassId {
    if (id1 === id2) {
        return id1;
    }

    const eclass1 = graph.e_classes.get(id1);
    const eclass2 = graph.e_classes.get(id2);

    if (!eclass1 || !eclass2) {
        throw new Error('Invalid E-class ID');
    }

    // 将eclass2的所有节点和父节点合并到eclass1
    eclass1.nodes.push(...eclass2.nodes);

    // 合并父节点映射
    for (const [node, parent_id] of eclass2.parents) {
        eclass1.parents.set(node, parent_id);
    }

    // 将被合并的ID加入工作列表
    graph.worklist.push(id2);

    // 删除eclass2
    graph.e_classes.delete(id2);

    return id1;
}

/**
 * 重建E-graph
 * 基于工作列表传播merge操作带来的同余变化
 */
export function rebuild_graph(graph: EGraph): void {
    while (graph.worklist.length > 0) {
        const merged_id = graph.worklist.pop()!;

        // 查找所有引用这个merged_id的父节点
        for (const [eclass_id, eclass] of graph.e_classes) {
            for (const node of eclass.nodes) {
                if (references_class(node, merged_id)) {
                    // 如果这个节点引用了被合并的ID，我们需要处理它
                    // 这里可以实现更复杂的同余闭包逻辑
                    // 为简单起见，我们暂时只处理直接引用
                }
            }
        }
    }
}

/**
 * 检查两个节点是否相等
 */
function nodes_equal(node1: AnyENode, node2: AnyENode): boolean {
    if (node1.type !== node2.type) {
        return false;
    }

    switch (node1.type) {
        case 'CONSTANT':
            return (node1 as ConstantNode).value === (node2 as ConstantNode).value;

        case 'SYMBOL':
            return (node1 as SymbolNode).name === (node2 as SymbolNode).name;

        case 'CALL': {
            const call1 = node1 as CallNode;
            const call2 = node2 as CallNode;
            if (call1.callee !== call2.callee || call1.args.length !== call2.args.length) {
                return false;
            }
            return call1.args.every((arg, i) => arg === call2.args[i]);
        }

        case 'LAMBDA': {
            const lambda1 = node1 as LambdaNode;
            const lambda2 = node2 as LambdaNode;
            if (lambda1.params.length !== lambda2.params.length || lambda1.body !== lambda2.body) {
                return false;
            }
            return lambda1.params.every((param, i) => param === lambda2.params[i]);
        }

        default:
            return false;
    }
}

/**
 * 检查节点是否引用了指定的E-class ID
 */
function references_class(node: AnyENode, class_id: EClassId): boolean {
    switch (node.type) {
        case 'CONSTANT':
        case 'SYMBOL':
            return false;

        case 'CALL': {
            const call = node as CallNode;
            return call.callee === class_id || call.args.some(arg => arg === class_id);
        }

        case 'LAMBDA': {
            const lambda = node as LambdaNode;
            return lambda.body === class_id;
        }

        default:
            return false;
    }
}