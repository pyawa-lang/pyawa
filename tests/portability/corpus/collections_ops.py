# collections：Counter/deque/OrderedDict（输出前排序）
import collections

counter = collections.Counter("abracadabra")
print("common", counter.most_common(3))
print("sorted_keys", sorted(counter))
deque = collections.deque([1, 2, 3])
deque.appendleft(0)
deque.append(4)
print("deque", list(deque))
ordered = collections.OrderedDict([("b", 2), ("a", 1)])
print("ordered", list(ordered.items()))
