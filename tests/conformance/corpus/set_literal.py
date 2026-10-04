# 集合字面量（第 242 轮）：逐元素 BUILD_SET、成员判定、去重
a = 1
b = 2
pair = {a, b}
has_one = 1 in pair
has_two = 2 in pair
has_three = 3 in pair
same = {1, 1, 2}
values = {a - a, b - b}
has_zero = 0 in values
single = {7}
has_seven = 7 in single
