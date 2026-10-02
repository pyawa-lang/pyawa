# f-string（第 238 轮）：插值、字面段、转换、格式规格、表达式、花括号转义
a = 1
b = 2
plain = f"{a}"
mixed = f"a{b}b"
converted = f"{a!r}"
spec = f"{a:>3}"
expression = f"{a + b}"
braces = f"{{}}"
empty = f""
