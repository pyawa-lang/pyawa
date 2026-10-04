# memoryview 是类型（第 187 轮）：_collections_abc 里 Sequence.register(memoryview)
# 如实记：本层尚无内存视图语义，只保证"名字是类型对象"
print(str(isinstance(memoryview, type)))
print(type(memoryview).__name__)
