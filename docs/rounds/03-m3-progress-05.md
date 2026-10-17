> 本卷是 `docs/ROUNDS.md` 分卷台账之一（**非规范**、只增不改；卷目见该索引）✓
> 覆盖 **M3 会话 05**（Lead＋队员团队模式）：task-26（形参 cell 快路 ⇒ `importlib.resources` 一族）、
> task-27（行内 suite 源末无换行 panic——不变量 3 素材）、task-28（C ABI 边界补全＋CX-11 机械化守卫）、
> task-29（P3-27 `Lib/` 语料第一批 30 条）。判据① **296 → 306 ÷ 628 ⇒ 48.7%** ✓。
> 战役队列口径的**上限诊断**（上游全量 ⊕ 本仓 `Lib/` 优先的 overlay）实测 **312/628**，卡住的大族见下 ✓。

## 现在

- **HEAD**：`b93ea6d`（`dev`，**已推送** origin ✓）—— task-26/27/28/29 四笔 ＋ 本笔收口（docs）
- **工作区**：**干净** ✓（`git status --porcelain` 为空 ✓ —— 以本笔提交为准 ✓）
- **判据①**（`python3 tools/lib_import_ratio.py`，**先重建 conformance 二进制**后实测 ✓）：
  **通过 289 ＋ 参照口径 17 ＝ 306 ÷ 628 ⇒ 48.7%**（区间 305–307 ✓，单次 ±1 ⇒ 报区间 ✓）
  - 战役累计：**296 → 306**（+10 ✓，正是 `importlib.resources` 一族 10 个 ✓）
  - 进度指标（不作判据 ✓，`CM-15`）：`Lib/` 377 文件 ⇒ **290** 能 import ＝ 76.9%
- 判据② **推进中** ✓：用户拍板**补语料**（守 `MS-13` ② 原文）⇒ 第一批 30 条落进
  `tests/conformance/corpus/`（**182 → 212**，下限 112 ✓；conformance **212 通过 · 0 已知差异 · 0 新差异** ✓、
  自检 212/212 ✓）——**判据② 仍不能裁定**（`Lib/` 语料只是第一批，P3-27 未完 ✓，如实 ✓）

## task-26 落地 ✓（形参 cell 不走快路 ＋ `os.remove` ⇒ `importlib.resources` 一族 10 个通）

- **重放** `target/wip_param_cell.diff`（`emitter.rs` +37/-4，5 处全未被后续提交覆盖 ✓，逐条与参照
  `dis` 一致 ✓）：①`AssignTuple` 双名融合守卫（cell ⇒ 不融 ✓）②逐目标 ⇒ `STORE_DEREF` ✓
  ③`Assign` 右值快路守卫（`p = cls` ⇒ `LOAD_DEREF` ✓）④for 目标 ⇒ `STORE_DEREF` ✓
  ⑤`emit_operand` boolop 直接操作数守卫 ✓。防回退 ✓：非 cell 双名解包仍融合 ✓。
- **最小复现三例**（`target/lead/recon/param_cell.py`，参照 3.14）：`p = cls` 修前返回
  `<cell object>`（正是 `importlib.resources` 那格 ✗）⇒ 修后 `9` ✓；`a, cls = (7,8)` 修前
  `(1,1,7)` ⇒ 修后 `(8,8,7)` ✓；for 目标 cell 修前 `1` ⇒ 修后 `2` ✓。
- **收账第 2 格**：`Lib/importlib/resources/_common.py:128` 的
  `_os_remove=os.remove`（`@contextlib.contextmanager` 的**仅关键字默认值**在导入期求值 ✓）
  ⇒ `posix_module.rs` 挂 `("remove", unlink_native)` 别名（参照 `os.remove is os.unlink` ✓）
  ⇒ `import importlib.resources` 通 ✓。
- **验收** ✓：新增 `crates/pyawa-runtime/tests/param_cell_shapes.rs` **7 例**（4 条脚本两侧逐行一致
  ＋ 3 条 opcode 级护栏）与参照**逐行一致** ✓；编译器逐字节 4/4 ✓；`lib_compile` 377/377 ✓；
  收账两道必检（`Lib/importlib/**`＋`functools.py` 26/26 逐个 import ✓ ＋ 无关脚本 `startup ok` ✓）；
  **先重建 conformance 二进制**再量 ⇒ **306** ✓。
- **踩坑记** ✓：`__pyawa__` 旧编译缓存按**脚本目录**复用 ⇒ 目录内任何改动都吃旧产物 ✗
  （第一次 quickcheck 探针跑出"修复前"结果 ✗）⇒ 收尾 `rm -rf Lib/__pyawa__`（＋ `target/**/__pyawa__`）✓。
- **顺带** ✓：`tools/sync_lib.py` 的 `SLICE` 补 `importlib/{readers,simple}.py` ＋
  `importlib/resources/**/*.py`（**故意用 `**/*.py`** ✗：`**` 会把上游 `__pycache__/*.pyc`
  也匹配进来 ⇒ 污染 `Lib/` ✗）。**如实记** ✗：`sync_lib.py --check` 在本笔之前就是红的，
  唯一不一致＝`Lib/enum.py`（有意保留的过渡假货，已提交）⇒ `check()` 只遍历 `Lib/` 与上游比对、
  **不读 `SLICE`** ⇒ 本笔不影响它 ✓。

## task-27 落地 ✓（行内 suite ＋ 源末无换行 ⇒ Rust panic —— 用户点名的「②」，坐实＋修通）

- **复现**（关键：源文件**末尾无换行**——之前所有人都被"带换行"骗了 ✗）：
  `printf 'for i in range(2): print(i)' > p.py && ./target/debug/pyawa p.py` ⇒
  `panicked at crates/pyawa-core/src/compile/parser.rs:1814:43: index out of bounds`（rc 101 ✓）。
  `while/def/class/async def` 行内体同病 ✓；`if True: pass` 不触发（碰巧先收了游标 ✓）。
  **conformance harness 会先补末尾换行 ⇒ corpus 测不到** ✗（回归必须走 Rust 侧直测 ✓）。
- **修法** ✓（`parser.rs` +55/-7）：①`parse_statements` 循环顶加 **EOF 正常收尾**（`tokens.get`
  为 `None` ⇒ `break` ✓）；②`other` 臂裸索引改 `spans.get(...).copied().unwrap_or(0)` ✓；
  ③四处行内体收尾**条件化前移**（`class`/`def` 仅 `Newline|Dedent` 才 `+= 1`；`for`/`while`
  行内仅 `Newline` 才吃 ⇒ 停在 `End` 不吃 ✓——否则"不 panic 但 `Err("模块结尾多出了 None")`" ✗）；
  ④**行内空体必须报错**（`class C:`／`def f():`／`for …:`／`while …:`／`if …:` 冒号后直接 EOF
  ⇒ 普通 `CompileError`"…的体要缩进" ✓，参照是 `IndentationError` ✓，**绝不静默接受** ✗）。
- **验收** ✓：新增 `crates/pyawa-core/tests/eof_inline_suite.rs` **2 例**（6 条合法 ⇒ `Ok` ✓；
  5 条空体 ⇒ `Err` 且消息可读 ✓；`catch_unwind` 断言**绝不 panic** ✓）；硬验收
  `printf 'for i in range(2): print(i)' > p.py && ./target/debug/pyawa p.py` ⇒ **输出 `0`/`1`、rc 0**，
  与参照**逐行一致** ✓；25 条无换行 fuzz ⇒ panic 0、合法 Err 0 ✓。
- **如实记** ✗（既有缺口、与本笔无关，带换行同样报错）：`with open(...): pass`／
  `match 1: case 1: pass`／`for …: stmt\nelse: stmt`（**同行 else suite**）⇒
  「`else` 的冒号后面要换行」✗（参照接受 ✓）。探针 `target/recon/fuzz_else.sh` ✓。

## task-28 落地 ✓（C ABI 全入口 `catch_unwind` 边界 ＋ `CX-11` 机械化守卫）

- **实测缺口** ✗：`crates/pyawa-abi/src/` 有 **73** 个 `#[unsafe(no_mangle)]` 导出
  （`lib.rs` 56 ＋ `helpers.rs` 17），修前只有 41 处走 `boundary()` ⇒ **32 个入口无边界** ✗
  （含 `helpers.rs` 的 17 个 `paL_*` 全裸 ✗——只盯 `lib.rs` 就是个洞 ✓）。
- **修法** ✓：①新增**指针版边界** `boundary_ptr`（panic ⇒ `NULL` ＋ 写 `state.message`，
  栈契约 `—` 不变 ✓）覆盖 `pa_errmsg`/`pa_tostring`/`pa_tointstring`/`pa_tobytes`；
  ②拆 `is_tag_inner`/`call_inner`/`settable_inner` ⇒ 7 个 `pa_is*`、`pa_call`/`pa_pcall`、
  `pa_rawset`/`pa_settable` **各自函数体内显式 `boundary`** ✓；③`pa_version`/`pa_abi_version`/
  `pa_abi_size` 进 `CX-11-EXEMPT` 白名单（源码注释写明理由 ✓）；④`paL_*` 17 个全部包上 ✓。
  ⇒ **73 个导出：70 有边界 ＋ 3 白名单** ✓。
- **机械化守卫** ✓（`crates/pyawa-abi/tests/boundary_guard.rs`，295 行）：递归枚举 `src/**/*.rs`
  每个 `#[unsafe(no_mangle)]`，跳字符串/注释的花括号配平取**自有函数体**，断言
  「体内有 `boundary(`/`boundary_ptr(` ∨ `CX-11-EXEMPT` 白名单行且理由 ≥8 字」＋ 显式期望名单
  双向一致 ＋ `extern "C"` ＋ 不重名 ＋ 解析器自检。**造反例验证会红**（裸入口 ⇒ exit 101 ✓；
  只写标记未入名单 ⇒ exit 101 ✓；撤反例后 `cmp` 证明 `lib.rs` 逐字节还原 ✓）。
  证据 `target/abi-boundary/guard_counterexample*_red.txt` ✓。
- **T-AB-2 回归** ✓：`tests/abi.rs` 新增 2 例——指针边界 panic ⇒ `NULL` ＋ `pa_errmsg` 含
  panic 文本 ✓；故意 panic 的宿主函数经 **`pa_call`** ⇒ `PA_ERR_RUNTIME`、越界 `pa_tointeger(99)`
  ⇒ `PA_ERR_INVALID`、进程不崩 ✓。
- **验收** ✓：`cargo test -p pyawa-abi` 全绿（abi 47 ＋ guard 2 ＋ conformance 3 ＋ header 2 ✓）；
  slowcheck 十项全绿 ✓、0 警告 ✓。
- **如实记** ✗：①`§15.4` 的 paL_ 计数口径（18 vs 17，`paL_newstate` 按规格在 `pyawa-runtime`）
  未动；②`boundary_ptr` 在 state 为 NULL/已释放时只能给 `NULL`（已写进契约 ✓）；
  ③守卫只覆盖 Rust 侧 `src/`，`include/pa.h` 声明与导出表一致性（`AB-6`/`T-AB-5`）仍是另一条线 ✗。

## task-29 落地 ✓（P3-27 第一批：`Lib/`＋`Lib/test/` 语料 30 条，判据② 补语料路线）

- **用户拍板** ✓：判据② **补语料**（守 `MS-13` ② 原文）。
- **侦察** ✓：`Lib/**/*.py` 全量 **377/377** 过纯 Python 编译不变量（`lib_compile` 闸门 ✓）；
  其中 **280/377 可 import**（只读探针，0 崩溃 0 超时 ✓）⇒ 落点 `target/recon/lib_importable.tsv`。
- **30 条语料** ✓（`Lib/test/` 来源 11 ＋ `Lib/` 来源 19，全以 `\n` 结尾 ⇒ **不依赖 harness 补换行** ✓）：
  manifest 登记 ＋ 新增出处守卫 `tools/check_lib_corpus.py`（片段语料 ↔ `Lib/test/` 源文件逐字比对 ✓）。
- **验收** ✓：`check_corpus_floor.py` 绿（**182 → 212**，各内容面只涨 ✓：类 34／异常 27／import 51／
  生成器 6／描述符 4／元类 4）；`conformance` **212 通过 · 0 已知差异 · 0 新差异** ✓ ＋ 自检
  212/212 ✓（`MS-12` ✓）；`tests/ci/check.py` 12/12 ✓；`check_fixture_cases.py` 499 ✓。
- **panic 猎捕** ✓：377 个 Lib 模块 import 扫描 **0 崩溃** ✓（runner 口径）；CLI 口径的越界
  panic 已单列成 task-27 ✓。
- **如实记（未决缺口，按 `MS-19` 不进语料不进差异清单，挂账）** ✗：`enum` 成员（`Color.RED.name`）、
  `copy.copy`、`types.SimpleNamespace`、`operator.attrgetter`、`@dataclass`、`fnmatch`
  （`PatternError`）、`urllib.parse.quote`（`frozenset.union`）、`str.capitalize`、`typing` 泛型别名
  `str()` 渲染、**`random.Random(42).random()` 与参照序列不同**（0.0838… vs 0.6394…，参照文档保证
  同种子同序列 ⇒ **真差异**，已立 `P3-30` ✓）、`itertools.groupby` 未接线。
- **过程更正** ✓：`twoside.py` 初版漏了 harness 的 `unescape`，曾把 `string.punctuation` 的反斜杠
  渲染误判为差异 ✗ ⇒ 补上后复核全量 ✓。

## 上限诊断（overlay：上游全量 ⊕ 本仓 `Lib/` 优先）—— 排期仪器，不作判据 ✓

- 实测 **312/628**（`target/lead/recon/ceiling_overlay.txt` ✓）。**口径修正** ✗：原 `--ceiling`
  把上游 `lib-full` 插在 `sys.path[0]` ⇒ 上游**原版**文件遮住本仓**适配版**（`Lib/enum.py` 等 ⇒
  全红假象 ✗）；正确做法＝overlay（上游全量 ＋ 本仓 `Lib/` 覆盖其上 ✓）。
- **卡住的大族**（每族＝「首个撞上的报错」，同族多格按序解锁）：
  | 规模 | 族 | 修法方向 |
  |---|---|---|
  | 42 | `set`/`dict`/`bytearray` 的 `\|=` 一族就地运算尚未接线 | executor 补就地运算（最小复现 `s = {1}; s \|= {2}` ✗） |
  | 41 | `No module named '_socket'` | socket 支（**用户已拍板立项**，`P3-29` ✓） |
  | 26 | 跳转目标标签从未落点（multiprocessing 族） | 编译器 bug，先钉最小复现 |
  | 17 | `difflib:1835` 嵌套元组目标 `(a,b),(c,d) = x,y` 解析 | 解析器补元组目标嵌套 ⇒ 通 unittest/test.support 链 |
  | 10 | `subprocess` 解析（「表达式里出现 Some(Dot)」） | 解析器 |
  | 10 | importlib 一族（本笔已修 ⇒ 现在是 overlay 旧读数） | 已收 ✓ |
  | 6 | 生成器表达式自由变量不在本层 cell/free 表 | 编译器（`_pydecimal`/`decimal` 族） |
  | 6 | `gettext` 制表符缩进尚未接线 | lexer（argparse 链） |
  | 6 | pickle `*` 出现在关键字实参之后 | 解析器 |
  | 24 | `_codecs_jp/iso2022/cn/kr/tw/hk` CJK 一族 | CJK 专支（既有挂账） |
- **队列结论** ✓：要抬到 421/628 还差 **115**；按上表族规模排序，wave-2 主攻
  **`\|=` 就地运算（42）→ difflib 嵌套元组（17，连 unittest/test.support 链）→ multiprocessing
  跳转标签（26）→ CJK（24）**；socket（`P3-29`）排 typing 收口后（用户已拍板 ✓）。

## 下一手（wave-2，按「阻塞最多模块的报错签名」队列）

1. **`set`/`dict`/`bytearray` 的 `|=` 一族**（42 下游）：executor 就地运算，最小复现已钉。
2. **difflib 嵌套元组目标**（17 下游＋test.support 链）：解析器补 `(a,b),(c,d) = …` ⇒ 同步
   `Lib/difflib.py`/`Lib/unittest/**`/`Lib/test/support/__init__.py`（**先重建 conformance 二进制**
   再量 ✓）。
3. multiprocessing 跳转标签（26 下游）：先钉最小复现。
4. CJK `_codecs_*` 专支（24 下游）。
5. socket 支（`P3-29`，用户已拍板立项，排 typing 收口后）。
