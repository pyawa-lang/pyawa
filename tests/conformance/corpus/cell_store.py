# **给 cell／自由变量赋值要走 `STORE_DEREF`**（第 343 轮真 bug 修）：
# 名字一旦被内层函数闭包捕获就是 **cell**，而 `emit_store_name` 先前只认 `global` 与 `varnames`
# ⇒ 落到 `STORE_NAME` ✗ ⇒ 在**函数**帧里直接中止（"`STORE_NAME` 需要命名空间帧" ✗）。
# 实测原形：`Lib/collections/__init__.py` 的 `namedtuple` 第 437 行
# `_dict, _tuple, _len, _map, _zip = dict, tuple, len, map, zip` —— 这五个名字都被它内层那几个
# 方法捕获 ⇒ 是 cell ⇒ 解包赋值当场中止（上限榜上 78 个模块压在它上面；修完那一族整族消失 ✓，
# 它们现在撞的是 `NameError: name 'eval' is not defined` ✓）。
#
# **本轮不比**（另两条仍开着的缺口，已记台账）：① **同函数里两个以上 cell** 时，第二个起的槽位仍不对
#   （`cannot access free variable …` ✗）；② 同一函数里 cell 的**增强赋值**后由闭包读
#   （`LOAD_NAME 需要命名空间帧` ✗）⇒ 语料只钉**每个函数一个 cell** 这条已经修好的路。

import sys


def outer():
    captured = None

    def inner():
        return captured

    captured = 7
    return (inner(), captured)


print(str(outer()))


def holder():
    held = None

    def peek():
        return held

    held = sys.intern("abc")
    return (peek(), held == "abc")


print(str(holder()))


def keeper():
    value = None

    def peek():
        return value

    value = [1, 2]
    return (peek(), value)


print(str(keeper()))


def text_holder():
    text = None

    def peek():
        return text

    text = "hello"
    return (peek(), text.upper())


print(str(text_holder()))
