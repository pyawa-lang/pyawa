# **带括号的海象后面接后缀链**（第 304 轮修）：`(ch := …).attr`／`(x := [7, 8])[0]`／`(f := len)("abc")`。
# 先前海象那一臂**提前 `return`**，绕过了 `parse_atom` 的统一后缀链（`.`／`(`／`[`，在 `match` 之后）
# ⇒ 报「括号没有闭合，实际 `Some(Dot)`」（`Lib/traceback.py:923` 的
# `(ch := lines[lineno][right_col]).isspace()`，那一族 15 个模块）。

# `.属性`
print(str((x := 5).bit_length()))
print(str(x))

# 下标
print(str((items := [7, 8, 9])[1]))
print(str(len(items)))

# 调用
print(str((f := len)("abcd")))
print(str(f("ab")))

# 串起来（属性 + 下标 + 调用）
print(str((text := "abcd").upper()))
print(str((box := [10, 20])[0]))

# 与二元运算混排（这条本来就通，防止回归）
print(str((y := 1) + 1))
