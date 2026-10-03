# 第四批方法（第 155 轮）：list.remove + str.lstrip/rstrip/title/capitalize
# 期望值先经参照核对
assert " He ".lstrip() == "He "
assert " He ".rstrip() == " He"
assert "a b".title() == "A B"
assert "aBc".title() == "Abc"
assert "aB".capitalize() == "Ab"
assert "".capitalize() == ""
items = [1, 2, 3]
items.remove(2)
assert items == [1, 3]
more = [1, 2, 2]
more.remove(2)
assert more == [1, 2]
# 注：`.remove(9)` 的报错路径**不放进语料** ✗ —— `except ValueError` 会踩到已登记的
# 「异常匹配」崩溃区 ✓（第 60 轮前后登记过同族问题 ✓），与本批方法无关 ✓。
print("ok")
