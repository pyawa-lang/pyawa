# 文件：相对路径读写、目录枚举、文本换行
import os

os.mkdir("sub")
with open("sub/data.txt", "w", encoding="utf-8") as handle:
    handle.write("a\nb\n")
with open("sub/data.txt", "r", encoding="utf-8") as handle:
    text = handle.read()
print("text", repr(text))
print("size", os.path.getsize("sub/data.txt"))
with open("sub/data.txt", "a", encoding="utf-8") as handle:
    handle.write("c")
print("append", open("sub/data.txt", "rb").read().decode("utf-8"))
print("listdir", sorted(os.listdir("sub")))
print("exists", os.path.exists("sub/data.txt"), os.path.exists("sub/nope.txt"))
os.remove("sub/data.txt")
print("removed", os.path.exists("sub/data.txt"))
