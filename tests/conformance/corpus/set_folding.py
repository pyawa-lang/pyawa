# 集合字面量折叠（第 249 轮）：≥3 个常量元素 ⇒ frozenset 常量 ＋ SET_UPDATE，语义仍是集合
three = {1, 2, 3}
has_one = 1 in three
has_four = 4 in three
total = 0
for item in three:
    total = total + item
dedup = {1, 1, 2}
has_two = 2 in dedup
words = {"b", "a", "c"}
has_a = "a" in words
