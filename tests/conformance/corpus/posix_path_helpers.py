# posix 面与 builtins 模块（第 195 轮）：只用两侧都有的模块，避开 harness 的搜索路径差异
import builtins
import posix

print(str(builtins.len is len))
print(posix._path_normpath("a//b"))
print(posix._path_normpath("//a/../.."))
print(str(posix._path_splitroot_ex("/a//b")))
