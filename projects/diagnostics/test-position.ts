import { SourceFile } from './src/source-file.js';

const file = new SourceFile('test.ts', 'line 1\nline 2\nline 3');
console.log('Content:', JSON.stringify(file.content));

// 测试位置转换
console.log('Offset 0:', file.offsetToPosition(0));
console.log('Offset 6:', file.offsetToPosition(6));
console.log('Offset 7:', file.offsetToPosition(7));
console.log('Offset 12:', file.offsetToPosition(12));
console.log('Offset 13:', file.offsetToPosition(13));