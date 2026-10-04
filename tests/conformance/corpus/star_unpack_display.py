# 显示里的星号解包（第 120 轮）：`BUILD_LIST/BUILD_SET` ＋ `LIST_EXTEND/SET_UPDATE` ✓
# 元组末尾再 `CALL_INTRINSIC_1 6` ✓
tail = [2, 3]
t = (1, *tail)
assert t == (1, 2, 3)

l = [1, *tail]
assert l == [1, 2, 3]

only = (*tail,)
assert only == (2, 3)

s = {1, *tail}
assert len(s) == 3
print("ok")
