# 构造器一族（第 130–131 轮）：全部经**逐条实测**确认已通 ✓
# 本轮顺手抓出两个真 bug：type() 返回借用指针未 retain ⇒ double free ✗；
# bool(x) 用了只认 bool/None 的 bool_value ⇒ bool(0) 返回 True ✗（现走执行器的通用真假判定 ✓）
assert list([1, 2, 3]) == [1, 2, 3]
assert list() == []
assert tuple([1, 2]) == (1, 2)
assert dict() == {}
assert str(12) == "12"
assert str("x") == "x"
assert int("42") == 42
assert int(7) == 7
assert float(2) == 2.0
assert bool(0) is False
assert bool(1) is True
assert bool([]) == False
assert len(list([1, 2])) == 2
assert type(1) is type(0)
print("ok")
