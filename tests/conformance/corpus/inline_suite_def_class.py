# 行内 suite：`def`／`class` 的行内体 ＋ 调用／属性读。
def double(x): return x * 2


class Box: value = 7


result = double(4)
boxed = Box.value
