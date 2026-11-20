# 语言：类、属性、继承、super
class Base:
    kind = "base"
    def __init__(self, value):
        self.value = value
    def describe(self):
        return f"{self.kind}:{self.value}"

class Child(Base):
    kind = "child"
    def __init__(self, value):
        super().__init__(value * 2)
    @property
    def doubled(self):
        return self.value * 2
    @staticmethod
    def static_tag():
        return "static"
    @classmethod
    def class_tag(cls):
        return cls.kind

obj = Child(3)
print("describe", obj.describe())
print("doubled", obj.doubled)
print("static", Child.static_tag())
print("class", Child.class_tag())
print("isinstance", isinstance(obj, Base))
print("mro", [cls.__name__ for cls in Child.__mro__])
print("repr", type(obj.value).__name__)
