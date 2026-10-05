"""按"整条 use 语句"清理编译器的 unused import 警告（第 148 轮固化，供每刀复用 ✓）。
只处理 crates/pyawa-core/src 下的文件；跳过再导出通道 `pub use crate::executor::<域>::*;` ✓。"""
import re, subprocess, pathlib
SRC = pathlib.Path("crates/pyawa-core/src")
def check():
    r = subprocess.run(["cargo", "check", "--workspace", "--all-targets"], capture_output=True, text=True)
    return r.stderr + r.stdout
for step in range(20):
    txt = check()
    hits = [(pathlib.Path(m.group(2)), int(m.group(3)),
             [n.strip(" `") for n in re.split(r",| and ", m.group(1)) if n.strip(" `")])
            for m in re.finditer(r"warning: unused imports?: ([^\n]+)\n\s*--> ([^\n:]+):(\d+):\d+", txt)]
    hits = [h for h in hits if str(h[0]).startswith(str(SRC))]
    if not hits:
        print("清理：无未用 import ✓（第", step, "步）"); break
    f, ln, names = hits[0]
    lines = f.read_text().splitlines(keepends=True)
    i = ln - 1
    while i >= 0 and not lines[i].lstrip().startswith("use "): i -= 1
    j = i
    while j < len(lines) and not lines[j].rstrip().endswith(";"): j += 1
    stmt = "".join(lines[i:j + 1])
    if "pub use crate::" in stmt and "::*;" in stmt:
        print("跳过再导出通道:", stmt.strip()[:70]); break
    new = stmt
    for n in names:
        new = re.sub(r"(?<![A-Za-z_0-9])" + re.escape(n) + r"\s*,\s*", "", new, count=1)
        if n in new: new = re.sub(r"\s*,\s*" + re.escape(n) + r"(?![A-Za-z_0-9])", "", new, count=1)
        if n in new: new = re.sub(r"(?<![A-Za-z_0-9])" + re.escape(n) + r"(?![A-Za-z_0-9])", "", new, count=1)
    tail = new.strip().rstrip(";")
    lines[i:j + 1] = [] if (tail.endswith("{}") or tail.endswith("::")) else [new]
    f.write_text("".join(lines)); print("清理:", f.name, ln, names)
print("剩余警告:", len([l for l in check().splitlines() if l.startswith("warning")]))
