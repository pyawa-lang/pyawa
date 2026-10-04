# `print`（第 94 轮）：**输出是比对项**（`MS-8` 的 stdout ✓）——
# 字节经 `print ⇒ sys.stdout ⇒ _io ⇒ fs` 出去，harness 与参照侧逐行比 ✓
print("hi")
print("a", "b")
print()
print("x", "y", "z")
