# **类型对象都有 `__doc__` 这个属性**（第 288 轮）：参照里没写文档串的类是 `None`、
# 写了的是字符串；`Lib/io.py:72` 一进门就 `_io._IOBase.__doc__`（先前直接 AttributeError）。


class WithDoc:
    """带文档串。"""


class WithoutDoc:
    pass


print(str(WithDoc.__doc__))
print(str(WithoutDoc.__doc__ is None))
print(str(hasattr(WithDoc, "__doc__")))
print(str(hasattr(WithoutDoc, "__doc__")))
print(str(WithDoc.__name__))
