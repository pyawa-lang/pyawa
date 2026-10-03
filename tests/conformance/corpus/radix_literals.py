# 进制前缀与下划线（第 126 轮）：0x / 0o / 0b 与 1_000 都走整数常量 ✓
assert 0xFF == 255
assert 0o17 == 15
assert 0b1010 == 10
assert 1_000 == 1000
assert 0xFFFF_FFFF == 4294967295
print("ok")
