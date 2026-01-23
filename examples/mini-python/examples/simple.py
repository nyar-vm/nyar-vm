# 简单的 Python 示例文件

def greet(name):
    """问候函数"""
    message = "Hello, " + name + "!"
    print(message)
    return message

def calculate(x, y):
    """计算函数"""
    result = x * 2 + y
    return result

# 简单的变量和运算
x = 10
y = 20
z = x + y
print(z)

# 测试条件语句
if z > 25:
    print("z is greater than 25")
else:
    print("z is not greater than 25")

# 主程序
if __name__ == "__main__":
    greet("World")
    result = calculate(10, 5)
    print("Result:", result)