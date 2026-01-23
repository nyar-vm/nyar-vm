# 控制流示例

def fibonacci(n):
    """计算斐波那契数列"""
    if n <= 1:
        return n
    else:
        return fibonacci(n-1) + fibonacci(n-2)

def process_numbers(numbers):
    """处理数字列表"""
    result = []
    
    for num in numbers:
        if num % 2 == 0:
            result.append(num * 2)
        else:
            result.append(num + 1)
    
    return result

def count_down(start):
    """倒计时"""
    while start > 0:
        print(f"Count: {start}")
        start -= 1
    print("Done!")

# 测试代码
numbers = [1, 2, 3, 4, 5]
processed = process_numbers(numbers)
print("Processed numbers:", processed)

for i in range(5):
    fib = fibonacci(i)
    print(f"Fibonacci({i}) = {fib}")

count_down(3)