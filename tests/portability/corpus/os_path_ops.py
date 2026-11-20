# os.path 的跨平台面：分隔符按归一化标签输出
import os
import sys

print("SEP=" + os.sep)
print("PATH=" + os.path.join("a", "b", "c.txt"))
print("PATH=" + os.path.basename("/x/y/z.txt"))
print("PATH=" + os.path.dirname("/x/y/z.txt"))
print("PATH=" + str(os.path.splitext("data.tar.gz")))
print("PATH=" + os.path.normpath("a/./b/../c"))
print("PATH=" + os.path.join("", "a"))
print("has_ext", os.path.splitext(".hidden")[1] == "")
print("PLATFORM=" + sys.platform)
print("OSNAME=" + os.name)
