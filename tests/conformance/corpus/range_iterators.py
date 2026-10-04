# range 的两种迭代器名字（第 228 轮）：大整数上限走 longrange_iterator
print(type(iter(range(3))).__name__)
print(type(iter(range(1 << 1000))).__name__)
print(str(next(iter(range(1 << 1000)))))
