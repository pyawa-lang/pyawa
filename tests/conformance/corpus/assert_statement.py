# `assert`（第 101／102 轮）：编译面逐字节对齐（夹具两条 ✓），这里守**运行期**的真跑 ✓
# 顺带守**真假判定**：空 `str` 为假、非空为真（第 102 轮扩面 ✓）——`assert "x"` 曾因
# 「真假判定只接线 None／bool／int」而报未接线 ✗
assert True
if "":
    print("bad-empty")
else:
    print("empty-is-false")
if "x":
    print("nonempty-is-true")
assert 1 == 1
assert "x"
print("ok")
