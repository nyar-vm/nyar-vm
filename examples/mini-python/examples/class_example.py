# 类定义示例

class Person:
    """人员类"""
    
    def __init__(self, name, age):
        self.name = name
        self.age = age
    
    def introduce(self):
        return f"My name is {self.name} and I am {self.age} years old"
    
    def have_birthday(self):
        self.age += 1
        print(f"Happy birthday! Now {self.age} years old")

class Student(Person):
    """学生类，继承自 Person"""
    
    def __init__(self, name, age, grade):
        super().__init__(name, age)
        self.grade = grade
    
    def study(self, subject):
        print(f"{self.name} is studying {subject}")

# 使用示例
student = Student("Alice", 20, "A")
student.introduce()
student.study("Python")
student.have_birthday()