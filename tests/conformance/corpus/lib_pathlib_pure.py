# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/pathlib/`（纯路径 `PurePosixPath` 的解析）。
import pathlib

p = pathlib.PurePosixPath("/a/b/c.py")
parts = str(p.parts)
name = p.name
suffix = p.suffix
joined = str(p.parent.joinpath("d.txt"))
