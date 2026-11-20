# os.stat 的可移植面：大小/类型（dev/ino/时间不进比对）
import os

with open("stat.txt", "w", encoding="utf-8") as handle:
    handle.write("hello")
st = os.stat("stat.txt")
print("size", st.st_size)
print("is_file", not os.path.isdir("stat.txt"))
print("dir_size_type", os.path.isdir("."))
print("sep_in_path", os.sep in os.path.abspath("stat.txt"))
os.remove("stat.txt")
