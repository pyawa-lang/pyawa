# 集合推导式（第 236 轮）：元素、if 条件、去重、成员判定
base = [1, 2, 3]
squares = {v * v for v in base}
has_nine = 9 in squares
has_four = 4 in squares
dup = {v - v for v in base}
dup_zero = 0 in dup
evens = {v for v in base if v > 1}
has_two = 2 in evens
has_one = 1 in evens
