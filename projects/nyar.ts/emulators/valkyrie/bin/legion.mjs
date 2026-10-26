#!/usr/bin/env node
/**
 * `legion` 薄入口：仅 `plan` / `build` 子命令，经 `legionPlan` / `legionBuild` N-API。
 * 其余子命令请在 TypeScript 侧调用正交 API，勿再拼 argv 透传 Rust。
 */

import path from 'node:path';
import {legionBuild, legionPlan} from '../dist/index.js';

function usage() {
    console.error(`用法:
  legion plan [--target <triple>] [<projectDir>]
  legion build [--target <triple>] [--output <dir>] [<projectDir>]`);
}

function parseFlags(argv) {
    let target;
    let output;
    const positional = [];
    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === '--target' || arg === '-t') {
            target = argv[++i];
            continue;
        }
        if (arg === '--output' || arg === '-o') {
            output = argv[++i];
            continue;
        }
        if (arg.startsWith('-')) {
            console.error(`未知选项: ${arg}`);
            process.exit(2);
        }
        positional.push(arg);
    }
    const projectDir = path.resolve(positional[0] ?? process.cwd());
    return {target, output, projectDir};
}

const [subcommand, ...rest] = process.argv.slice(2);

if (subcommand === 'plan') {
    const {target, projectDir} = parseFlags(rest);
    const outcome = legionPlan({projectDir, target});
    if (!outcome.ok) {
        console.error(outcome.error ?? 'plan failed');
        process.exit(1);
    }
    console.log(`workspace: ${outcome.workspaceRoot}`);
    console.log(`project: ${outcome.project}`);
    console.log(`target: ${outcome.target}`);
    console.log(`sources: ${outcome.sourceCount}`);
    console.log(`semantic groups: ${outcome.semanticGroupCount}`);
    console.log(`dependencies: ${outcome.dependencyCount}`);
    for (const group of outcome.groups) {
        console.log(`  group ${group.dependencyKey}: ${group.fileCount} file(s)`);
    }
    console.log('plan: ok');
    process.exit(0);
}

if (subcommand === 'build') {
    const {target, output, projectDir} = parseFlags(rest);
    const outcome = legionBuild({projectDir, target, outputDir: output});
    if (!outcome.ok) {
        console.error(outcome.error ?? 'build failed');
        process.exit(1);
    }
    console.log(`project: ${outcome.project}`);
    console.log(`target: ${outcome.target}`);
    console.log(`output: ${outcome.outputDir}`);
    for (const artifact of outcome.artifacts) {
        console.log(`artifact: ${artifact}`);
    }
    process.exit(0);
}

if (!subcommand || subcommand === '--help' || subcommand === '-h') {
    usage();
    process.exit(subcommand ? 0 : 2);
}

console.error(`Legion 不再接受任意 argv 子命令 "${subcommand}"；仅支持 plan / build。`);
usage();
process.exit(2);
