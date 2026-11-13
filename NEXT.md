# NEXT（会话 12 · M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407 · 已知差异 0 · 新差异 0；`Lib/test` 可编译 397/397＝100%；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。
> **本页规矩**（`agents-rules/round-rule.md` §5）：**≤ 40 行**；只写 状态／下一条命令／未修 bug＋判据；
> **逐轮复盘只进台账** `docs/rounds/04-post-m6-01.md` ✗ —— 本页不再累积历史
> （2026-11-10 瘦身：47 353 字符 / 单行 31 743 → 本页 ✓）。

- **HEAD**：`fd6fc82f`（本笔前；round 262（memory 专项①）collect 冻结窗口 double-decref 防御 ⇒ test_io 内存标记消除；全量 memory 仍 3（difflib／gc／weakref 时序，均 A/B 既有）；下轮续查 difflib 欠 incref＋到第 8 项 ABI 请示）
- **工作区**：干净 ✓（`git status --porcelain` 空）
- **下一阶段**：M6 后继续抬官方套件跑通面（**报告级口径** ✓，非判据）
- **计划次序**：**A 效率**（主线，见 `PLAN` §9.4 的 A→B→C 块 ✓）→ **B 内存守成** → **C 其余排后** ✗
- **完整工单**：`PLAN` §9.4（2026-11-12 **一次性下达** ✓，逐轮自走 ✗ 不再逐步请示；只对四类请示 ✗）
- **当前主线（2026-11-12 用户口径 ✓）**：**内存安全 ＋ 执行效率**（解释器硬要求 ✓）；其余**慢慢补** ✗
  - ① 内存硬门：`tools/memdangle_survey.py` 归零 ＋ 无 abort／segfault
  - ② 执行效率：`tools/bench_hot.py` 基线入库 ＋ 禁回退（>10% 红）＋ 报 CPython 比值（A2-6 后 18/26/24/15×；本批开工 214/232/208/135×）
  - ③ 其余（日常库／冷门语义／套件 %）**排后** ✗
- **官方套件**（报告级，会漂 ✓）：**79 ok / 68 unimplemented / 245 runtime / 5 timeout**（round 180 全量 survey）
- **内存类硬门** ✓：`python3 tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 全量套件）⇒ `target/memdangle_survey.tsv`；
  **memory 类计数收敛到 0** 是本专项第一硬门（**目前非 0** ✗，数字见台账）
- **未修 bug ＋ 判据**（如实 ✓）：
  - **编译器**：推导式内嵌生成器表达式引用推导变量 **round 162 已接线**（隐藏形参法 ✓；判据见台账 round 162 §2）
  - **iter_value 丢异常**（round 171 撤回）：`f(*Plain())` ×2 万仍漏 AttributeError/TypeError 各 2 万；三种修法都会让 test_dict.test_constructor 间歇失败 ⇒ 下一轮先定位 pending 槽副作用链
  - `test_weakref`：round 256 已收口 **17.3 s（<120 s ✓）**——真根因是 `_remove_dead_weakref`→`dict_remove_key` 线性扫（70k 批量死引用 O(n²)），已改哈希索引；另修「释放链里 gc.collect 重入」双放（`test_callback_gcs`）。剩余 26F/9E 是功能缺口
  - 形参所有权老账：round 255 探针 9 形态全 0 delta ⇒ 无最小复现、关闭 ✓；残留 `iter_value` 丢异常（有复现，三种修法已撤回 ⇒ 按 3 轮规则停手）· traceback 帧保活 ⇒ 工单第 4 项
  - 日常面：round 257 已收 `json.load`／`match` 行内 suite＋`as` 守卫／`defaultdict` copy·`|`·`==`／`urllib.request` import·file://；**HTTP 体读**未接（缺 `RawIOBase.read`／`recv_into` 面）✗
  - `P3-42` KeyError 族／特殊方法派发；`P3-35` 剩余 5 形态；`P3-29`／`P3-37` 剩余面；C2 剩余记档：组 traceback 格式／`sys.exception()` 多层恢复／组子类化／空组与单例细节
- **下一条命令**：`tools/profile_hot.sh --tier baseline --iters 100000`（**默认 release ✓**）→ 按新头部取 release_one／next_instruction／with_mro 热点 → A4 回退闸（基线入库）

## 关键口径（只留项目特有的；通用规则见 `agents-rules/`）

- **套件体检**：`tools/libtests_survey.py --release`（397 个顶层 `test_*.py`；目录
  `target/upstream/extracted/usr/lib/python3.14/test`）＋ `tools/libtests_triage.py`；
  **两个比例是报告级、不当验收**（`CM-15`）
- **改编译器后清缓存**：`find target Lib -name __pyawa__ -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`
- **判据①**：`tools/lib_import_ratio.py`（先重建 conformance 二进制；±1 报区间；禁止 `| tail`）
- **ABI**：`PA_ABI_VERSION`＝**1.4**（round 34 `fs.pipe` 表尾追加；`AB-41`）；指令集版本 **2**
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／`P3-42`（新增前先查已定义；墓碑算定义）
- **闸门**：`&&` 只看 exit code；`git commit -F - -- <显式路径>`；slowcheck 十一项全绿才提交
