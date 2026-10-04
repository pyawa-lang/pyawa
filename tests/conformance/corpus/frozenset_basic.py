# frozenset（第 236 轮）：与 set 同载荷；len／迭代／repr 都要认
fs = frozenset([1, 2, 2, 3])
print(type(fs).__name__)
print(str(len(fs)))
print(str(len(list(fs))))
print(type(frozenset()).__name__)
