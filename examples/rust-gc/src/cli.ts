#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'fs';
import { resolve, basename, extname } from 'path';
import { EGraph } from '@nyar/hir';
import { hir_to_mir } from './hir_to_mir.js';
import { compile_to_javascript } from './compiler/javascript.js';

/**
 * Rust-GC CLI 编译器
 * 支持: rust-gc compile xxx.rs --target js
 */
class RustGCCompiler {
    private source_file: string;
    private target: string;
    private output_file: string | null;
    
    constructor(args: string[]) {
        this.parse_args(args);
    }
    
    /**
     * 解析命令行参数
     */
    private parse_args(args: string[]): void {
        if (args.length < 3) {
            this.print_usage();
            process.exit(1);
        }
        
        if (args[2] !== 'compile') {
            console.error('错误: 只支持 "compile" 命令');
            this.print_usage();
            process.exit(1);
        }
        
        if (args.length < 4) {
            console.error('错误: 需要指定源文件');
            this.print_usage();
            process.exit(1);
        }
        
        this.source_file = args[3];
        
        // 解析目标参数
        let target_found = false;
        for (let i = 4; i < args.length; i++) {
            if (args[i] === '--target' && i + 1 < args.length) {
                this.target = args[i + 1];
                target_found = true;
                i++; // 跳过目标值
            } else if (args[i] === '-o' && i + 1 < args.length) {
                this.output_file = args[i + 1];
                i++; // 跳过输出文件
            }
        }
        
        if (!target_found) {
            console.error('错误: 需要指定 --target 参数');
            this.print_usage();
            process.exit(1);
        }
        
        if (this.target !== 'js' && this.target !== 'javascript') {
            console.error('错误: 目前只支持 "js" 或 "javascript" 目标');
            process.exit(1);
        }
    }
    
    /**
     * 打印使用说明
     */
    private print_usage(): void {
        console.log('使用方法: rust-gc compile <源文件> --target <目标> [选项]');
        console.log('');
        console.log('参数:');
        console.log('  <源文件>    Rust源文件路径');
        console.log('  --target    目标平台 (目前支持: js, javascript)');
        console.log('  -o          输出文件路径 (可选)');
        console.log('');
        console.log('示例:');
        console.log('  rust-gc compile factorial.rs --target js');
        console.log('  rust-gc compile factorial.rs --target js -o factorial.js');
    }
    
    /**
     * 运行编译器
     */
    async compile(): Promise<void> {
        try {
            console.log(`正在编译: ${this.source_file}`);
            console.log(`目标平台: ${this.target}`);
            
            // 读取源文件
            const source_code = this.read_source_file();
            
            // 解析Rust代码（简化版本，实际应该使用完整的Rust解析器）
            const hir_program = this.parse_rust_to_hir(source_code);
            
            // HIR到MIR转换
            console.log('正在执行 HIR -> MIR 转换...');
            const mir_graph = hir_to_mir(hir_program);
            
            // MIR到JavaScript编译
            console.log('正在执行 MIR -> JavaScript 编译...');
            const javascript_code = compile_to_javascript(mir_graph);
            
            // 确定输出文件路径
            const output_path = this.get_output_path();
            
            // 写入输出文件
            writeFileSync(output_path, javascript_code, 'utf8');
            
            console.log(`编译成功: ${output_path}`);
            
        } catch (error) {
            console.error('编译失败:', error instanceof Error ? error.message : String(error));
            process.exit(1);
        }
    }
    
    /**
     * 读取源文件
     */
    private read_source_file(): string {
        try {
            return readFileSync(this.source_file, 'utf8');
        } catch (error) {
            throw new Error(`无法读取源文件: ${this.source_file}`);
        }
    }
    
    /**
     * 简化的Rust到HIR解析
     * 注意: 这只是一个示例实现，实际的Rust解析要复杂得多
     */
    private parse_rust_to_hir(source_code: string): any {
        // 这里应该实现完整的Rust解析器
        // 目前返回一个简单的示例程序结构
        return {
            type: 'Program',
            items: [
                {
                    type: 'Function',
                    name: 'factorial',
                    params: [
                        { name: 'n', type: 'i32' }
                    ],
                    return_type: 'i32',
                    body: {
                        type: 'If',
                        cond: {
                            type: 'Binary',
                            op: 'Le',
                            left: { type: 'Path', name: 'n' },
                            right: { type: 'Literal', value: 1, literal_type: 'i32' }
                        },
                        then_expr: { type: 'Literal', value: 1, literal_type: 'i32' },
                        else_expr: {
                            type: 'Binary',
                            op: 'Mul',
                            left: { type: 'Path', name: 'n' },
                            right: {
                                type: 'Call',
                                func: { type: 'Path', name: 'factorial' },
                                args: [
                                    {
                                        type: 'Binary',
                                        op: 'Sub',
                                        left: { type: 'Path', name: 'n' },
                                        right: { type: 'Literal', value: 1, literal_type: 'i32' }
                                    }
                                ]
                            }
                        }
                    }
                }
            ]
        };
    }
    
    /**
     * 获取输出文件路径
     */
    private get_output_path(): string {
        if (this.output_file) {
            return resolve(this.output_file);
        }
        
        const source_dir = resolve(this.source_file).replace(/\\/g, '/').split('/').slice(0, -1).join('/');
        const source_name = basename(this.source_file, extname(this.source_file));
        return resolve(source_dir, `${source_name}.js`);
    }
}

/**
 * 主函数
 */
async function main(): Promise<void> {
    const compiler = new RustGCCompiler(process.argv);
    await compiler.compile();
}

// 运行CLI
if (import.meta.url === `file://${process.argv[1]}`) {
    main().catch(error => {
        console.error('未处理的错误:', error);
        process.exit(1);
    });
}

export { RustGCCompiler };