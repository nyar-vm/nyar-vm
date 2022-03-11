import { readFileSync } from 'fs';

import pkg from 'wat2wasm';
const { wat2wasm } = pkg;

async function run() {
  try {
    // 读取 .wat 文件
    const watCode = readFileSync('module.wat', 'utf8');

    // 转换为二进制数据
    const wasmBytes = await wat2wasm(watCode);

    // 创建 WebAssembly 模块
    const wasmModule = new WebAssembly.Module(wasmBytes.buffer);

    // 创建 WebAssembly 实例
    const imports = {}; // 导入对象，如果有导入的函数/值需要提供，在这里进行配置
    const wasmInstance = new WebAssembly.Instance(wasmModule, imports);

    // 获取导出的函数
    const { yourExportedFunction } = wasmInstance.exports;

    // 调用导出函数
    const result = yourExportedFunction();
    console.log(result);
  } catch (error) {
    console.error(error);
  }
}

run();