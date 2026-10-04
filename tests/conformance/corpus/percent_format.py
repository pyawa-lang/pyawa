# `str % value`（printf 风格；第 281 轮接线）—— `Lib/` 里遍地都是，实测第一个撞上的是
# `codecs.py` 的 `raise SystemError('… %s' % e)`（`encodings.*` 那一族因此全红）。
#
# 口径全部**照参照实测**：右操作数元组 ⇒ 位置实参、否则单个实参、`%(名字)` ⇒ 映射；
# 修饰符 `-`／`+`／空格／`#`／`0`／宽度／`.精度`；长度修饰符忽略；错误消息逐条量过。
#
# **不进语料**的（`MS-19` 的适用范围）：`%*` 的宽度取自实参（本层如实报未接线）、
# `%x` 一族对**超出 i64 的大整数**（十进制可以、其它进制未接）、`bytes % value`（PEP 461 未接）。

print("%s" % 1)
print("%s" % (1,))
print("%s %s" % (1, 2))
print("%r" % "a")
print("%d" % 3.7)
print("%i" % -5)
print("[%5s]" % "ab")
print("[%-5s]" % "ab")
print("%05d" % 42)
print("%.2f" % 3.14159)
print("%f" % 1)
print("%x" % 255)
print("%X" % 255)
print("%o" % 8)
print("%e" % 12345.678)
print("%g" % 0.00001)
print("%%")
print("%s" % None)
print("%c" % 65)
print("%(a)s" % {"a": 5})
print("%+d" % 5)
print("[% d]" % 5)
print("%#x" % 255)
print("%.3g" % 1234.5678)
print("%g" % 100000.0)
print("%.0f" % 2.5)
print("%.2s" % "abcdef")
print("[%10.3f]" % 3.14159)
print("%a" % "café")


def report(function):
    try:
        return "ok:" + str(function())
    except Exception as error:
        return type(error).__name__ + ":" + str(error)


print(report(lambda: "%s %s" % (1,)))
print(report(lambda: "%s" % (1, 2)))
print(report(lambda: "%d" % "a"))
print(report(lambda: "%f" % "x"))
print(report(lambda: "%x" % 3.9))
print(report(lambda: "%(a)s" % {}))
print(report(lambda: "%(a)s" % (1,)))
print(report(lambda: "%q" % 1))
print(report(lambda: "%c" % 1.5))
print(report(lambda: "%c" % "ab"))
print(report(lambda: "abc%" % 1))
