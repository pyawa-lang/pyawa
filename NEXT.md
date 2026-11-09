# NEXT（会话 12 · M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407 · 已知差异 0 · 新差异 0；`Lib/test` 可编译 397/397＝100%；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据；
> **逐轮复盘只进台账** `docs/rounds/04-post-m6-01.md` ✗ —— 本页不再累积历史
> （2026-11-10 瘦身：47 353 字符 / 单行 31 743 → 本页 ✓）。

- **HEAD**：`c061e34`（round 159：控制流异常 `SkipTest` 不挂 `__traceback__`（如实偏差）；线程化用例跳过后 live 645 783 ⇒ 8x）
- **工作区**：干净 ✓（`git status --porcelain` 空）
- **下一阶段**：M6 后继续抬官方套件跑通面（**报告级口径** ✓，非判据）
- **官方套件**（报告级，会漂 ✓）：约 **75–80 ok / 397 ≈ 19–20%**（10-31 是 41／10.3%）；超时 1–2（基本只剩 `test_weakref`）
- **内存类硬门** ✓：`python3 tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 全量套件）⇒ `target/memdangle_survey.tsv`；
  **memory 类计数收敛到 0** 是本专项第一硬门（**目前非 0** ✗，数字见台账）
- **未修 bug ＋ 判据**（如实 ✓）：
  - `test_weakref` 285 s ✗（唯一超时；根因＝4 万+ 异常对象的 traceback 保活被测局部；候选 `pending_exception`／`exception_state` 栈）
  - 形参所有权／引用泄漏 ✗（台账 13 处；三版方案已撤回）
  - 日常面：`json.load(f)` ✗（`StopIteration`）· `defaultdict` ✗ · `match` 同行 suite ✗ · `urllib.request` ✗
  - `P3-42` KeyError 族／特殊方法派发；`P3-35` 剩余 5 形态；`P3-29`／`P3-37` 剩余面
- **下一条命令**：`git fetch && git reset --hard origin/dev`（历史会重写 ✗ 先核对）→ `python3 tests/ci/check.py` → 按上面"未修 bug"取活

## 关键口径（只留项目特有的；通用规则见 `agents-rules/`）

- **套件体检**：`tools/libtests_survey.py --release`（397 个顶层 `test_*.py`；目录
  `target/upstream/extracted/usr/lib/python3.14/test`）＋ `tools/libtests_triage.py`；
  **两个比例是报告级、不当验收**（`CM-15`）
- **改编译器后清缓存**：`find target Lib -name __pyawa__ -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`
- **判据①**：`tools/lib_import_ratio.py`（先重建 conformance 二进制；±1 报区间；禁止 `| tail`）
- **ABI**：`PA_ABI_VERSION`＝**1.4**（round 34 `fs.pipe` 表尾追加；`AB-41`）；指令集版本 **2**
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-42`（新增前先查已定义；墓碑算定义）
- **闸门**：`&&` 只看 exit code；`git commit -F - -- <显式路径>`；slowcheck 十项全绿才提交
