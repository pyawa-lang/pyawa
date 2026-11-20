# re：确定性模式匹配
import re

pattern = re.compile(r"(\w+)=(\d+)")
text = "a=1; b=22; c=333"
print("findall", pattern.findall(text))
print("sub", pattern.sub(r"\2:\1", text))
print("split", re.split(r"\s*-\s*", "a - b-c"))
print("groups", pattern.search(text).groups())
