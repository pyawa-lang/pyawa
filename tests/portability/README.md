# 脚本可移植门（P0-2）

**判据唯一出处**：`docs/PLAN-milestones.md` §12.26；契约：`docs/REQUIREMENTS.md`
「脚本可移植性承诺」＋ `docs/DESIGN.md` §14。

## 怎么跑

```bash
cargo build -p pyawa-runtime --bin pyawa
python3 tests/portability/runner.py --out /tmp/portability-linux.json
# 另一平台（如 Windows CI）产出 /tmp/portability-windows.json
python3 tests/portability/compare.py /tmp/portability-linux.json /tmp/portability-windows.json
```

CI 里由 `portability` 矩阵 job（Linux＋Windows）各跑 `runner.py` 上传 canonical JSON，
`portability-compare`（ubuntu）下载两份产物后跑 `compare.py`；**未登记差异 ⇒ job 红**。

## 规则

- **归一化只在 harness**（`runner.py`）发生：运行时对 `sys.platform`／`os.name`／路径等**如实回答**；
- 白名单：`PLATFORM=`／`OSNAME=`／`SEP=`／`PATH=`（分隔符）／`CWD=`／`TEMP=`／`ERRNO=`／
  `WINERROR=`／`TIME=`／`ADDRESS=`／`PATHMODULE=`；其余字节逐字比对；
- 输出必须确定性：集合/字典输出前排序、浮点固定格式、不打印地址/时间（需要时用上述标签）；
- **已知差异**必须进 `known_diffs.json`（`{"script.py": "理由＋依据"}`），并在 `PLAN §12.26`／
  `divergences.md` 如实登记；**禁止**用白名单掩盖真实行为差异。
