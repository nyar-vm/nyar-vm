class Person:
    def __init__(self, name, age):
        self.name = name
        self.age = age

    def say_hello(self):
        print(f"Hello, I am {self.name}")
        print(f"I am {self.age} years old")

p = Person("Alice", 25)
p.say_hello()
print(p.name)
p.age = 26
print(p.age)
