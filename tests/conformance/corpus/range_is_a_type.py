# range 是类型（第 237 轮）：名字要指向类型对象，register 一族才认
# （**如实记** ✗：`type(range(3))` 本层仍给 `range_iterator` ✗ —— 参照给 `range` ✓，那条偏差另记 ✓。）
print(str(isinstance(range, type)))
print(str(list(range(3))))
