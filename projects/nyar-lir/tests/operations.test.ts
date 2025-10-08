import {describe, expect, it} from 'vitest';
import {
    add_node,
    CallNode,
    ConstantNode,
    create_graph,
    merge_classes,
    rebuild_graph,
    SymbolNode
} from '../src/index.js';

describe('E-graph operations', () => {
    describe('add_node', () => {
        it('should add a constant node to the graph', () => {
            const graph = create_graph();
            const node: ConstantNode = {type: 'CONSTANT', value: 42};

            const class_id = add_node(graph, node);

            expect(class_id).toBe(0);
            expect(graph.e_classes.size).toBe(1);
            expect(graph.e_classes.get(class_id)?.nodes).toContainEqual(node);
        });

        it('should return existing class ID for duplicate nodes', () => {
            const graph = create_graph();
            const node: ConstantNode = {type: 'CONSTANT', value: 42};

            const class_id1 = add_node(graph, node);
            const class_id2 = add_node(graph, node);

            expect(class_id1).toBe(class_id2);
            expect(graph.e_classes.size).toBe(1);
        });

        it('should add different constant nodes to different classes', () => {
            const graph = create_graph();
            const node1: ConstantNode = {type: 'CONSTANT', value: 42};
            const node2: ConstantNode = {type: 'CONSTANT', value: 43};

            const class_id1 = add_node(graph, node1);
            const class_id2 = add_node(graph, node2);

            expect(class_id1).not.toBe(class_id2);
            expect(graph.e_classes.size).toBe(2);
        });

        it('should add symbol nodes correctly', () => {
            const graph = create_graph();
            const node: SymbolNode = {type: 'SYMBOL', name: 'foo'};

            const class_id = add_node(graph, node);

            expect(class_id).toBe(0);
            expect(graph.e_classes.get(class_id)?.nodes).toContainEqual(node);
        });

        it('should add call nodes correctly', () => {
            const graph = create_graph();
            const symbol_node: SymbolNode = {type: 'SYMBOL', name: 'add'};
            const const_node: ConstantNode = {type: 'CONSTANT', value: 1};

            const symbol_class = add_node(graph, symbol_node);
            const const_class = add_node(graph, const_node);

            const call_node: CallNode = {
                type: 'CALL',
                callee: symbol_class,
                args: [const_class, const_class]
            };

            const call_class = add_node(graph, call_node);

            expect(call_class).toBe(2);
            expect(graph.e_classes.get(call_class)?.nodes).toContainEqual(call_node);
        });
    });

    describe('merge_classes', () => {
        it('should merge two different classes', () => {
            const graph = create_graph();
            const node1: ConstantNode = {type: 'CONSTANT', value: 1};
            const node2: ConstantNode = {type: 'CONSTANT', value: 2};

            const class_id1 = add_node(graph, node1);
            const class_id2 = add_node(graph, node2);

            const merged_id = merge_classes(graph, class_id1, class_id2);

            expect(merged_id).toBe(class_id1);
            expect(graph.e_classes.size).toBe(1);
            expect(graph.e_classes.get(class_id1)?.nodes).toHaveLength(2);
            expect(graph.worklist).toContain(class_id2);
        });

        it('should return same ID when merging identical classes', () => {
            const graph = create_graph();
            const node: ConstantNode = {type: 'CONSTANT', value: 42};

            const class_id = add_node(graph, node);
            const merged_id = merge_classes(graph, class_id, class_id);

            expect(merged_id).toBe(class_id);
            expect(graph.e_classes.size).toBe(1);
        });

        it('should throw error for invalid class IDs', () => {
            const graph = create_graph();

            expect(() => merge_classes(graph, 999, 1000)).toThrow('Invalid E-class ID');
        });
    });

    describe('rebuild_graph', () => {
        it('should process empty worklist without errors', () => {
            const graph = create_graph();

            expect(() => rebuild_graph(graph)).not.toThrow();
            expect(graph.worklist).toHaveLength(0);
        });

        it('should clear worklist after rebuild', () => {
            const graph = create_graph();
            const node1: ConstantNode = {type: 'CONSTANT', value: 1};
            const node2: ConstantNode = {type: 'CONSTANT', value: 2};

            const class_id1 = add_node(graph, node1);
            const class_id2 = add_node(graph, node2);

            merge_classes(graph, class_id1, class_id2);
            expect(graph.worklist.length).toBeGreaterThan(0);

            rebuild_graph(graph);
            expect(graph.worklist).toHaveLength(0);
        });
    });

    describe('Integration tests', () => {
        it('should handle complex e-graph construction', () => {
            const graph = create_graph();

            // 创建一些基本节点
            const add_symbol: SymbolNode = {type: 'SYMBOL', name: 'add'};
            const const_1: ConstantNode = {type: 'CONSTANT', value: 1};
            const const_2: ConstantNode = {type: 'CONSTANT', value: 2};

            const add_class = add_node(graph, add_symbol);
            const const_1_class = add_node(graph, const_1);
            const const_2_class = add_node(graph, const_2);

            // 创建调用节点: (add 1 2)
            const call_node: CallNode = {
                type: 'CALL',
                callee: add_class,
                args: [const_1_class, const_2_class]
            };

            const call_class = add_node(graph, call_node);

            expect(graph.e_classes.size).toBe(4);
            expect(call_class).toBe(3);

            // 测试合并操作
            const const_3: ConstantNode = {type: 'CONSTANT', value: 3};
            const const_3_class = add_node(graph, const_3);

            merge_classes(graph, const_1_class, const_3_class);
            rebuild_graph(graph);

            expect(graph.e_classes.size).toBe(4); // 合并后减少了一个类
        });

        it('should maintain node uniqueness after merge', () => {
            const graph = create_graph();
            const node: ConstantNode = {type: 'CONSTANT', value: 42};

            const class_id1 = add_node(graph, node);
            const class_id2 = add_node(graph, node);

            expect(class_id1).toBe(class_id2);
            expect(graph.e_classes.size).toBe(1);
        });
    });
});