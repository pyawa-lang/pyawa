# 相邻 f-string 拼接（第 113 轮）：`f"a{x}" f"b"` 要合成**一个** f-string ✓
# 上游 `_bootstrap.py:1426`／`site.py:210` 都卡在**跨行**的相邻 f-string ✗
name = "x"

a = f"a{name}" f"b"
assert a == "axb"

b = f"1{name} " f"2{name}"
assert b == "1x 2x"

print(f"p{name} "
      f"q{name}")
print("ok")
