# 属性内建（第 148 轮）：getattr / hasattr / setattr 走真正的属性通道
# （此前 getattr/hasattr 把对象当字典查 ⇒ 对非字典是 UB，实测会 abort ✗）
class C:
    x = 1

    def m(self):
        return 2


c = C()
assert getattr(c, "x") == 1
assert getattr(c, "m")() == 2
assert hasattr(c, "x") is True
assert hasattr(c, "zz") is False
assert getattr(c, "zz", 5) == 5
assert getattr(c, "zz", "s") == "s"
setattr(c, "y", 9)
assert c.y == 9
assert getattr(c, "y") == 9
print("ok")
