# **只有注释的行**（第 286 轮）：参照的词法器把它当**空行** ⇒ 既不判缩进、也不发 `Indent`／`Dedent`。
# 先前只跳"纯空白行" ⇒ 一行缩进写的注释当场发 `Indent` ⇒ 下一句报
# "不认识的语句开头 Some(Indent)"（`Lib/test/support/__init__.py:190` 那种"续行注释"正是这个形状）。

x = 0           # comment with ( unbalanced
                # indented continuation comment
                # another ) and more
y = 1           # a comment after code
# a comment at column zero
print(str(x + y))

def outer():
    a = 1
    # 缩进的注释：块里也要照跳
    b = 2
    return a + b

print(str(outer()))

result = (
    1
    # 括号续行里的注释
    + 2
)
print(str(result))
