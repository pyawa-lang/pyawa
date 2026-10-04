# int 方法面（第 195 轮）：to_bytes / bit_length
# 以及 startswith / endswith 接受字符串元组（同轮）
assert (255).to_bytes(2, "big") == b"\x00\xff"
assert (1).to_bytes(2, "little") == b"\x01\x00"
assert (0).to_bytes(1, "big") == b"\x00"
assert (65535).to_bytes(2, "big") == b"\xff\xff"

assert (5).bit_length() == 3
assert (0).bit_length() == 0
assert (255).bit_length() == 8

assert "abc".startswith(("a", "x"))
assert not "abc".startswith(("x", "y"))
assert "abc".endswith(("c", "z"))
assert "abc".startswith("a")
assert "abc".endswith("c")
print("ok")
