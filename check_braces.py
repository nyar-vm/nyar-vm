
import sys

def check_balance(filename):
    try:
        with open(filename, 'r') as f:
            lines = f.readlines()
    except FileNotFoundError:
        print(f"File not found: {filename}")
        return

    stack = []
    
    for line_num, line in enumerate(lines, 1):
        for char in line:
            if char == '{':
                stack.append(('{', line_num))
            elif char == '}':
                if not stack:
                    print(f"Extra closing brace at line {line_num}")
                    return
                op, line_start = stack.pop()
                if not stack:
                     print(f"Stack became empty at line {line_num} (closed brace from line {line_start})")
    
    if stack:
        print(f"Unclosed braces: {stack}")
    else:
        print("Braces are balanced.")

if len(sys.argv) > 1:
    check_balance(sys.argv[1])
else:
    check_balance("examples/valkyrie-bootstrap/library/ast/parser.vk")
