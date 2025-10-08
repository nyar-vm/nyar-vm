export {
    // 类型定义
    EClassId,
    ENode,
    ConstantNode,
    SymbolNode,
    CallNode,
    LambdaNode,
    AnyENode,
    EClass,
    EGraph,
    create_graph
} from './egraph.js';

export {
    add_node,
    merge_classes,
    rebuild_graph
} from './operations.js';