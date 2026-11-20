# itertools：确定性输出
import itertools

print("product", list(itertools.product([1, 2], ["a"])))
print("islice", list(itertools.islice(itertools.count(10), 3)))
print("chain", list(itertools.chain([1], [2, 3])))
print("groupby", [(k, list(g)) for k, g in itertools.groupby([1, 1, 2, 2, 3])])
print("accumulate", list(itertools.accumulate([1, 2, 3])))
