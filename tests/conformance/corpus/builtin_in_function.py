# 函数里调用内建（第 156 轮修的真 bug）：LOAD_GLOBAL 低位那把 NULL 压错了位置
# 先前任何"函数里调用内建"都报 TypeError: 'NULL' object is not callable


def take_len():
    return len([1, 2, 3])


def take_min():
    return min(3, 1, 2)


def take_bool():
    return bool(1)


def call_two():
    return max(len("ab"), len("abc"))


assert take_len() == 3
assert take_min() == 1
assert take_bool() is True
assert call_two() == 3
assert globals() is not None
_seen = 1
assert "_seen" in globals()
print("ok")
