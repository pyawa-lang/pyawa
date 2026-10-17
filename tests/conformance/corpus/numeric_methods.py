# **数值方法三件**（第 352 轮）：`int.bit_count` ✓、`float.is_integer` ✓、`float.as_integer_ratio` ✓
# —— `float` 类型先前**根本没有 getattr 槽** ⇒ 后两件一律 AttributeError。

print(str((255).bit_count()))
print(str((-1).bit_count()))
print(str((0).bit_count()))
print(str((255).bit_length()))
print(str((2.0).is_integer()))
print(str((2.5).is_integer()))
print(repr((0.5).as_integer_ratio()))
print(repr((2.0).as_integer_ratio()))
print(repr((0.1).as_integer_ratio()))
print(repr((1.5).as_integer_ratio()))
print(str((-2.0).is_integer()))
print(str((0.0).is_integer()))
