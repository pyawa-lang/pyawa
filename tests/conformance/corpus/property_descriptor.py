# `@property` 的**描述符协议**（第 279 轮接线 ✓）：`__get__` 得进 `property` 的**类型字典**才生效
# （属性访问通道只认 `type_lookup(类型, "__get__")`）。同一轮还修了两处：
#   ① **装饰器调用的 `self` 槽**——参照的 `@property def g` 产 `LOAD_NAME property; <函数>; CALL 0`
#      （没有 `PUSH_NULL`），函数落在 `self` 槽上、按 `property(g)` 解析；先前类调用把它丢了 ✗；
#   ② 嵌套 `if` 的尾位（见 `nested_if_tail.py`）。
# 同一形状是 `Lib/importlib/_bootstrap.py` 的 `ModuleSpec.has_location`／`cached`／`parent` ✓。
#
# **不进语料的部分**（"尚未实现"，`MS-19` 的适用范围 ✓）：`@value.setter` 的**数据描述符写**
# （`property.__set__`）与 `@property` 的 `__set_name__`／文档串 —— 那些还没有对照物。


def fget(self):
    return 42


plain = property(fget)


class Box:
    def __init__(self, value):
        self._value = value

    @property
    def value(self):
        return self._value

    @property
    def doubled(self):
        return self._value * 2


box = Box(21)
class_level = Box.value  # 类级访问 ⇒ 交出 property 自己（`obj is None` 那一支）

print("fget" if plain.fget is fget else "no")
print("21" if box.value == 21 else "no")
print("42" if box.doubled == 42 else "no")
print("prop" if type(class_level).__name__ == "property" else "no")
print("none" if Box.__dict__["value"].fget is None else "no")
