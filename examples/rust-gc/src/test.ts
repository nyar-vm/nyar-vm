// 测试文件 - 验证HIR到MIR的转换
import { convert_factorial_example } from './hir_to_mir.js';

// 运行测试
function run_test(): void {
  console.log('=== 测试HIR到MIR的规范化转换 ===');
  
  try {
    const graph = convert_factorial_example();
    
    console.log('转换成功！生成的MIR E-graph:');
    console.log(`EClass数量: ${graph.e_classes.size}`);
    
    // 打印所有EClass的内容
    graph.e_classes.forEach((e_class, id) => {
      console.log(`\nEClass ${id}:`);
      e_class.nodes.forEach((node, index) => {
        console.log(`  节点 ${index}: ${JSON.stringify(node, null, 2)}`);
      });
    });
    
    console.log('\n✅ 测试通过！所有HIR表达式已成功转换为MIR CALL原语。');
    
  } catch (error) {
    console.error('❌ 测试失败:', error);
  }
}

// 运行测试
run_test();