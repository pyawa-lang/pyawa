# NEXT（会话 12 · M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407 · 已知差异 0 · 新差异 0；`Lib/test` 可编译 397/397＝100%；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据；
> **逐轮复盘只进台账** `docs/rounds/04-post-m6-01.md` ✗ —— 本页不再累积历史
> （2026-11-10 瘦身：47 353 字符 / 单行 31 743 → 本页 ✓）。

- **HEAD**：`2203356`（round 226：array.array 惰性注册 MutableSequence＋type.__flags__＋__abc_tpflags__ 校验 ⇒ test_collections 2F/2E→1F/1E（总失败 2）；下轮：test_Callable 的 type.__call__ 根因／try 体内 await）
- **工作区**：干净 ✓（`git status --porcelain` 空）
- **下一阶段**：M6 后继续抬官方套件跑通面（**报告级口径** ✓，非判据）
- **官方套件**（报告级，会漂 ✓）：**79 ok / 68 unimplemented / 245 runtime / 5 timeout**（round 180 全量 survey）
- **内存类硬门** ✓：`python3 tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 全量套件）⇒ `target/memdangle_survey.tsv`；
  **memory 类计数收敛到 0** 是本专项第一硬门（**目前非 0** ✗，数字见台账）
- **未修 bug ＋ 判据**（如实 ✓）：
  - **编译器**：推导式内嵌生成器表达式引用推导变量 **round 162 已接线**（隐藏形参法 ✓；判据见台账 round 162 §2）
  - **iter_value 丢异常**（round 171 撤回）：`f(*Plain())` ×2 万仍漏 AttributeError/TypeError 各 2 万；三种修法都会让 test_dict.test_constructor 间歇失败 ⇒ 下一轮先定位 pending 槽副作用链
  - `test_weakref` **169 s** ✗（唯一超时；已修：dict 删除 O(1)、SkipTest 不挂回溯、分代 GC；剩因＝被吞 AttributeError 泄漏 +1（已修 __getattr__ 回退一族：writeln 40k⇒0、四连 skip 3957⇒216）；**还剩 216 颗**（TestLoader.loadTestsFromName@71/_compile@178/_isnotsuite@8/EnumType.__new__@294 等）⇒ 下一轮同法逐个钉）
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
