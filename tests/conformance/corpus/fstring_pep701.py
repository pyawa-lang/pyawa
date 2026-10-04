# **f-string 的两处口径**（第 305 轮）：
# ① 插值表达式**两端的空白合法**（`f"{ w }"`）—— 先前片段原样再词法化 ⇒ 前导空格被当成缩进 ⇒
#    报「表达式里出现 `Some(Indent)`」（`Lib/traceback.py` 那一族）；
# ② **同种引号可以嵌在插值里**（PEP 701，3.12+）：`f'{g(1, '__notes__', repr)}'` ——
#    先前"见引号就收尾" ⇒ 正文被截断 ⇒ 报「f-string: expecting '}'」（`traceback.py:1072`）。
# 两处的报错现在都**带位点**（第 305 轮加），便于定位。


def g(value, name, conv):
    return str(conv(value)) + ":" + str(name)


print(f"{ 1 + 1 }")
print(f"{g(1, '__notes__', repr)}")
print(f'{g(2, "double", repr)}')
print(f"{g(3, 'single', repr)}")
print(f"{{literal}} {g(4, 'x', repr)}")
print(f"{ {'k': 1}['k'] }")
print(f"{'a' * 3}")
print(f"{g(5, 'y', repr)!r}")
print(f"{3.14159:{'0.2f'}}")
