import { SourceFile } from './src/source-file.js';

const file = new SourceFile('test.ts', 'line 1\nline 2\nline 3');
console.log('Content:', JSON.stringify(file.content));
console.log('Length:', file.content.length);

// 测试 span1: 0-7 ("line 1\n")
const span1 = { start: 0, end: 7 };
console.log('Span1 text:', JSON.stringify(file.substring(span1.start, span1.end)));

// 测试 span2: 7-13 ("line 2\n")  
const span2 = { start: 7, end: 13 };
console.log('Span2 text:', JSON.stringify(file.substring(span2.start, span2.end)));

// 测试合并: 0-13 ("line 1\nline 2\n")
const merged = { start: 0, end: 13 };
console.log('Merged text:', JSON.stringify(file.substring(merged.start, merged.end)));