import { describe, expect, it } from 'vitest';
import { convert_factorial_example } from '../src/hir_to_mir';
import { EGraph } from '@nyar/hir';

describe('HIR to MIR conversion', () => {
    it('should convert factorial example without errors', () => {
        const graph = new EGraph();

        // 运行转换函数，不应该抛出错误
        expect(() => {
            convert_factorial_example(graph);
        }).not.toThrow();

        // 验证图不为空
        expect(graph.e_classes.size).toBeGreaterThan(0);
    });

    it('should create valid E-graph structure', () => {
        const graph = new EGraph();
        convert_factorial_example(graph);

        // 验证E-graph的基本结构
        expect(graph).toBeDefined();
        expect(graph.e_classes).toBeDefined();
        expect(graph.worklist).toBeDefined();
        expect(graph.next_id).toBeGreaterThanOrEqual(0);
    });
});
