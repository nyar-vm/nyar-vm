# 多继承系统 (Multiple Inheritance)

## 概述

Valkyrie 支持多继承，允许一个类同时继承多个父类。多继承使用 C3 线性化算法来解决方法解析顺序 (Method Resolution Order, MRO) 问题，确保继承的一致性和可预测性。

## 基本多继承语法

### 简单多继承

```valkyrie
class A {
    micro method_a(self) {
        print("Method from A")
    }
    
    micro common_method(self) {
        print("A's common method")
    }
}

class B {
    micro method_b(self) {
        print("Method from B")
    }
    
    micro common_method(self) {
        print("B's common method")
    }
}

class C {
    micro method_c(self) {
        print("Method from C")
    }
    
    micro common_method(self) {
        print("C's common method")
    }
}

# 多继承语法：class 子类(父类1, 父类2, ...)
class MultiChild(A, B, C) {
    micro own_method(self) {
        print("MultiChild's own method")
    }
}
```

### 重命名继承

当多个父类有同名方法时，可以使用重命名语法来避免冲突：

```valkyrie
class Display {
    micro show(self) {
        print("Display show")
    }
}

class Printer {
    micro show(self) {
        print("Printer show")
    }
    
    micro print(self) {
        print("Printing...")
    }
}

# 重命名继承语法：class 子类(rename: 父类, 其他父类)
class Document(rename: Display, Printer) {
    micro display_document(self) {
        # 通过重命名访问 Display 的方法
        self.rename.show()  # 调用 Display::show
        self.print()        # 调用 Printer::print
        self.show()         # 调用 Printer::show (C3 线性化的第一个匹配)
    }
}
```

### 复杂重命名场景

```valkyrie
class FileReader {
    micro read(self) -> string {
        "Reading from file"
    }
    
    micro close(self) {
        print("Closing file")
    }
}

class NetworkReader {
    micro read(self) -> string {
        "Reading from network"
    }
    
    micro close(self) {
        print("Closing connection")
    }
}

class Logger {
    micro log(self, message: string) {
        print("Log: {}", message)
    }
}

# 多重重命名
class HybridReader(file_reader: FileReader, net_reader: NetworkReader, Logger) {
    micro read_from_file(self) -> string {
        let content = self.file_reader.read()
        self.log(@format("Read from file: {}", content))
        content
    }
    
    micro read_from_network(self) -> string {
        let content = self.net_reader.read()
        self.log(@format("Read from network: {}", content))
        content
    }
    
    micro cleanup(self) {
        self.file_reader.close()
        self.net_reader.close()
    }
}
```

## C3 线性化算法

Valkyrie 使用 C3 线性化算法来确定方法解析顺序：

```valkyrie
class A {
    micro method(self) { print("A") }
}

class B(A) {
    micro method(self) { print("B") }
}

class C(A) {
    micro method(self) { print("C") }
}

class D(B, C) {
    # 没有重写 method
}

# C3 线性化顺序：D -> B -> C -> A
# 调用 d.method() 会调用 B::method
let d = D {}
d.method()  # 输出："B"
```

### 线性化顺序示例

```valkyrie
class Base {
    micro base_method(self) { print("Base") }
}

class Left(Base) {
    micro left_method(self) { print("Left") }
    micro common_method(self) { print("Left common") }
}

class Right(Base) {
    micro right_method(self) { print("Right") }
    micro common_method(self) { print("Right common") }
}

class Middle(Left, Right) {
    micro middle_method(self) { print("Middle") }
}

class Final(Middle, Right) {
    # C3 线性化：Final -> Middle -> Left -> Right -> Base
    micro test_resolution(self) {
        self.common_method()  # 调用 Left::common_method
        self.left_method()    # 调用 Left::left_method
        self.right_method()   # 调用 Right::right_method
        self.base_method()    # 调用 Base::base_method
    }
}
```

## 方法访问模式

### 直接访问

```valkyrie
class Child(A, B, C) {
    micro test_access(self) {
        # 直接调用，使用 C3 线性化顺序
        self.common_method()  # 调用第一个匹配的方法
        
        # 通过重命名访问特定父类的方法
        # 注意：没有 super 关键字
    }
}
```

### 重命名访问

```valkyrie
class AdvancedChild(primary: A, secondary: B, tertiary: C) {
    micro demonstrate_access(self) {
        # 通过重命名访问特定父类
        self.primary.common_method()    # 调用 A::common_method
        self.secondary.common_method()  # 调用 B::common_method
        self.tertiary.common_method()   # 调用 C::common_method
        
        # 直接访问使用 C3 线性化
        self.common_method()  # 调用 A::common_method (第一个)
    }
}
```

## 匿名类继承

Valkyrie 支持匿名类的继承：

```valkyrie
# 匿名类继承
micro process_shape(shape: class(Drawable, Movable) {
    micro area(self) -> f64
}) {
    shape.draw()
    shape.move_to(10.0, 20.0)
    print("Area: {}", shape.area())
}

# 使用匿名类
let circle = class(Drawable, Movable) {
    radius: f64,
    x: f64,
    y: f64,
    
    micro area(self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

process_shape(circle { radius: 5.0, x: 0.0, y: 0.0 })
```

### 匿名类重命名继承

```valkyrie
# 匿名类的重命名继承
micro create_hybrid_processor() -> class(reader: FileReader, writer: FileWriter) {
    micro process(self, filename: string) {
        let content = self.reader.read_file(filename)
        let processed = content.to_uppercase()
        self.writer.write_file(filename + ".processed", processed)
    }
}

let processor = create_hybrid_processor() {
    # 匿名类实现
}

## 构造函数和初始化

```valkyrie
class Parent1 {
    value1: i32,
    
    micro new(v1: i32) -> Self {
        Self { value1: v1 }
    }
}

class Parent2 {
    value2: string,
    
    micro new(v2: string) -> Self {
        Self { value2: v2 }
    }
}

class MultiInherit(Parent1, Parent2) {
    own_value: f64,
    
    # 多继承的构造函数
    micro new(v1: i32, v2: string, own: f64) -> Self {
        Self {
            # 父类字段初始化
            value1: v1,
            value2: v2,
            # 自己的字段
            own_value: own,
        }
    }
}
```

## 抽象类和接口

```valkyrie
# 抽象基类
abstract class Shape {
    abstract micro area(self) -> f64
    abstract micro perimeter(self) -> f64
    
    # 具体方法
    micro describe(self) {
        print("Area: {}, Perimeter: {}", self.area(), self.perimeter())
    }
}

# 接口定义
trait Drawable {
    micro draw(self)
    micro set_color(self, color: Color)
}

# 多继承：抽象类 + 接口
class Rectangle(Shape): Drawable {
    width: f64,
    height: f64,
    color: Color,
    
    # 实现抽象方法
    micro area(self) -> f64 {
        self.width * self.height
    }
    
    micro perimeter(self) -> f64 {
        2.0 * (self.width + self.height)
    }
    
    # 实现接口方法
    micro draw(self) {
        print("Drawing rectangle {}x{}", self.width, self.height)
    }
    
    micro set_color(self, color: Color) {
        self.color = color
    }
}
```

## 钻石问题解决

```valkyrie
class GrandParent {
    micro method(self) { print("GrandParent") }
}

class Parent1(GrandParent) {
    micro method(self) { print("Parent1") }
}

class Parent2(GrandParent) {
    micro method(self) { print("Parent2") }
}

# 钻石继承
class Child(Parent1, Parent2) {
    # C3 线性化自动解决钻石问题
    # 线性化顺序：Child -> Parent1 -> Parent2 -> GrandParent
    
    micro test_diamond(self) {
        self.method()  # 调用 Parent1::method
    }
    
    # 如果需要调用特定父类的方法，使用重命名
}

# 使用重命名解决钻石问题
class ResolvedChild(p1: Parent1, p2: Parent2) {
    micro test_resolved(self) {
        self.p1.method()  # 明确调用 Parent1::method
        self.p2.method()  # 明确调用 Parent2::method
    }
}
```

## 最佳实践

### 1. 优先使用组合而非继承

```valkyrie
# 好的设计：组合
class Document {
    reader: FileReader,
    writer: FileWriter,
    logger: Logger,
    
    micro process(self) {
        let content = self.reader.read()
        let processed = self.transform(content)
        self.writer.write(processed)
        self.logger.log("Document processed")
    }
}

# 避免：过度继承
# class Document(FileReader, FileWriter, Logger) { ... }
```

### 2. 使用重命名避免方法冲突

```valkyrie
# 清晰的重命名
class MediaPlayer(audio: AudioPlayer, video: VideoPlayer) {
    micro play_audio(self, file: String) {
        self.audio.play(file)
    }
    
    micro play_video(self, file: String) {
        self.video.play(file)
    }
}
```

### 3. 文档化继承关系

```valkyrie
# 清晰的继承文档
@.doc("""
MultiProcessor 继承关系：
- DataProcessor: 提供数据处理能力
- NetworkHandler: 提供网络通信能力
- Logger: 提供日志记录能力

C3 线性化顺序：MultiProcessor -> DataProcessor -> NetworkHandler -> Logger
""")
class MultiProcessor(DataProcessor, NetworkHandler, Logger) {
    # 实现
}
```

## 总结

Valkyrie 的多继承系统特点：

1. **C3 线性化**：确保方法解析的一致性
2. **重命名机制**：解决方法名冲突
3. **无 super 关键字**：通过重命名明确访问父类方法
4. **匿名类支持**：支持临时的多继承类定义
5. **类型安全**：编译时检查继承关系的合法性

正确使用多继承可以实现灵活的代码复用，但应该谨慎使用，优先考虑组合和接口设计。