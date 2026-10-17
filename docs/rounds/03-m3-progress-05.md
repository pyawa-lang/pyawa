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

## Round 2（wave-2）落地 ✓

- **HEAD**：`570cb80`（`dev`，**已推送** origin ✓）—— task-4（`|=` 就地族）＋ task-6（嵌套 boolop）＋
  task-5B（语料第二批 22 条）＋ docs 收口
- **判据①**（重建 conformance 后实测 ✓）：**通过 324 ＋ 参照口径 17 ＝ 341 ÷ 628 ⇒ 54.3%**
  （区间 340–342 ✓；round 1 末 306 ⇒ **+34**＝`|=` 解锁后同步进 `Lib/` 的 34 个文件 ✓）
  - 进度指标：`Lib/` **411** 文件 ⇒ 324 能 import（round 1 末 377／290）
- 语料：**212 → 234**（conformance 234 通过 · 0 已知差异 · 0 新差异 ✓，自检 234/234 ✓）

## task-6 落地 ✓（嵌套 boolop 丢 cleanup 标签 —— 编译期 panic ＋ 4 个值错形态）

- **侦察**（ds-corpus-libtest，task-5 A）：multiprocessing 一族 26 个模块的真凶**不是**它——是
  `signal.py:7–12` 的 lambda 体 `A and (B and not C) or D` ⇒ `multiprocessing.process:18
  import signal` 一编译就 panic。最小复现 `target/recon/jump_label_min.py`（10 行，**必须用名字**，
  字面量被常量折叠绕开 ✗）⇒ `panicked at emitter.rs:718: 跳转目标标签 2 从未落点`（rc 101）。
- **根因** ✓：`emit_test_value` 的 `BoolOp` 分支收 `cleanup` 形参但**从不落点它**（只给自己的末操作数
  传 `Some(fresh)`）⇒ 嵌套 boolop 作为外层末操作数时外层标签被吞。
- **修法** ✓（`emitter.rs` +11/-2）：`fresh = cleanup.unwrap_or_else(new_label)`；内层操作数跳
  `dest = if short == jump_if_true { target } else { fresh }`——**同极性嵌套直接跳 `target` 保持值** ✓。
- **意外收获** ✓：全真值矩阵（16 形态 × 全组合，全用名字）暴露 **4 个「不 panic 但值错」** ✗：
  `(a and b) and c`（`a` 假时错返回 `c`）／`a and (b and c) and d`／`a or (b and c) and d`／
  `(a or b) or c` ⇒ 同根因（同极性嵌套非末操作数应直跳 `target`）。修前 9/16 一致 ⇒ 修后 **16/16** ✓。
- **验收** ✓：`boolop_nest.rs`（14 形态全编译成功＋`POP_JUMP_IF_*` 极性序列逐个与参照 `dis` 一致 ✓）
  ＋ `boolop_nest_shapes.rs`（16 组「结果＋已求值序列」两侧逐行一致，副作用钉短路次序 ✓）。
- **如实记** ✗：`signal.py` 编译**通过**但 import 仍失败（`No module named '_signal'` C 模块）⇒
  **未同步**；multiprocessing 一族还压 `_signal`/`_socket` ⇒ 本轮不收。

## task-4 落地 ✓（`set`／`dict` 就地位运算族 ⇒ 解锁 34 个模块同步）

- **修法** ✓（`protocol.rs` +219/-7）：`inplace_arithmetic` 按类型分派＋新增 `inplace_set_operation`
  （`|=`／`&=`／`-=`／`^=`，右值 set/frozenset，**返回同一对象** `is` 真 ✓）／`inplace_dict_merge`
  （`|=`：dict／键值对迭代／生成器／有 `keys()` 的映射，错误臂逐字对参照 ✓）／`dict_put_owned`
  （引用账按 `OM-20` 纪律，错误路径还 leftover ✓）。
- **不多接线** ✓（照参照）：`bytearray |=`／`list |=`／`dict -=` 报 `TypeError`（参照同 ✓）；
  不可变类型＝基运算（原路未动 ✓）；`frozenset |=` 左值回 plain set（既有基运算偏差，如实记 ✗）。
- **对拍** ✓：32 行探针与参照 `diff` 空（`PYAWA_DANGLING=1` 下同 ✓）；错误文案逐字一致
  （含 `dictionary update sequence element #0 has length 1; 2 is required` ✓）。
- **收账** ✓：`|=` 解锁 ⇒ **34 个文件同步进 `Lib/`**（`find_syncable` 量出＋两道必检 34/34 逐个
  import ✓＋`startup ok` ✓＋与上游逐字节一致 ✓）；`dbm/gnu`／`dbm/sqlite3` 缺 C 基座 ⇒ 不进 ✓。
  SLICE 同步补 34 条（`tools/sync_lib.py` +41）。
- **下一道闸** ✗（如实记）：sysconfig／ctypes 族现压 `posix.getcwd`／`posix.readlink` 未接线
  ⇒ **未同步 `Lib/sysconfig/`**。
- **踩坑记** ✗（既有问题，非本轮引入）：`tools/sync_lib.py --sync` 有 **11 条无 `.py` 的陈旧
  SLICE 条目**（`_compat_pickle`／`ast`／…）会让 `--sync` 直接报「上游没有」退出 ⇒ 待修；
  `Lib/enum.py` 仍是 CX-8 唯一不一致项（有意适配件）。

## task-5B 落地 ✓（P3-27 语料第二批 22 条：212 → 234）

- 形参 cell／闭包程序级回归 **6**（含 `singledispatch.register` 的 `argname, cls = next(...)`
  ＋`p = cls` 实测原形）＋行内 suite 程序级 **3**＋`Lib/` import 驱动 **10**（collections／abc／
  codecs／encodings.aliases／pathlib／contextlib／datetime）＋`Lib/test/` 逐字片段 **3**
  （出处守卫扩到 7 条语料 9 段正文 ✓）。
- **验收** ✓：corpus floor **234** 只涨 ✓（类 38／异常 27／import 62／生成器 8／元类 4）；
  conformance **234 通过 · 0 新差异** ✓；check.py 12/12 ✓；红条目一律不收 ✓。

## task-5B／task-7 侦察副产物（新钉的真 bug，均已配 10 行复现，挂账 round 3+）

1. **`global X` ＋ 函数内 `import X` ⇒ `UnboundLocalError`**（读按局部、绑定按全局两张表 ✗）；
   受害者 `collections.Counter.most_common`（`global heapq`＋惰性 import）。`target/recon/global_import_min.py` ✓。
2. **`warnings.catch_warnings(record=True)` ⇒ `帧操作失败：StackUnderflow`** ✗。
3. **`collections.ChainMap.keys()` 静默返回空** ✗（`cm["x"]` 对）。
4. **未捕获 `KeyError` 消息少一层 repr**（`'missing'` vs `missing`）✗。
5. **`functools.wraps` 不搬 `__name__`** ✗。

## task-7 侦察 ✓（round 3 弹药：difflib ⇒ unittest ⇒ test.support 链路全量缺口，9＋2 格）

落点 `target/recon/parser_batch_recon.md`（每格≤10 行最小复现＋源文件行号＋「修完下一格」overlay 实测）：

| 格 | 源 | 修完下一格 |
|---|---|---|
| 嵌套元组目标 `(a,b),(c,d) = x,y` | `difflib.py:1835` | **difflib 直接通**；unittest → `_signal` |
| `.05` 前导点浮点 | `subprocess.py:2077` | `_signal` |
| 行内 if 分号表＋次行 else | `urllib/request.py:1805` | `sys._getframe` depth=0 |
| `*` 后置于关键字实参 | `pickle.py:615` | 内建 `dir` |
| `with … as (a, b)` 元组目标 | `xml/etree/ElementTree.py:727` | **直接通** |
| 推导式模式目标 | `_pyrepl/terminfo.py:405` | `'function' object has no attribute '__builtins__'` |
| 元组里的 `*` | `tomllib/_parser.py:96/98/100` | 注解赋值非名字目标 |
| 带注解赋值非名字目标 | `test/libregrtest/main.py:58` 等 | `os.getcwd`；base_eventqueue **通** |
| 制表符缩进（gettext 是 **tab 注释**） | `gettext.py:519` 等 | **直接通** |
| 下标赋值基底是调用 `f(...)[k] = v` | `_pyrepl/keymap.py:204` | （base_eventqueue 第二格） |
| PEP 604 union 注解 dataclass 字段 | `a: str | None = None` | `__builtins__` 面 |

- **切批建议**（按解析器同段归类）：①目标/模式族（嵌套元组＋with-as 元组＋推导式目标＋调用基底下标）
  ②表达式族（`.05`、元组 `*`）③语句族（行内 if 分号 else、注解赋值目标、tab）④实参族（`*` 后置）。
- 已立 **P3-31**（解析器缺口批）✓。

## Round 3（P3-31 解析器缺口批）落地 ✓

- **HEAD**：`f4a21a3`（`dev`，**已推送** origin ✓）—— task-8（P3-31 十格全落）＋ docs 收口
- **判据①**（重建 conformance 后实测，两次一致 ✓）：**通过 341 ＋ 参照口径 17 ＝ 358 ÷ 628 ⇒ 57.0%**
  （区间 357–359；round 2 末 341 ⇒ **+17**＝本批同步的 11 个模块 ＋ 链上顺带 ✓）
  - 进度指标：`Lib/` **425** 文件 ⇒ 342 能 import（80.5%）
- 战役累计：208 → 296 → 306 → 341 → **358**（本会话 +62；距 421 阈值还差 63）

## task-8 落地 ✓（P3-31 解析器缺口批：四族 10 格 ⇒ 10/10，无停手项）

- **改动**（`parser.rs` +422/-143、`emitter.rs` +406/-261、`lexer.rs` +65/-6、`compile.rs` +15/-7、
  `symbols.rs` +9/-8——解析器改动**与语义同笔**落地 ✓）：
  1. **嵌套元组目标**：`ParenTarget` 统一收法（`Single`＝分组不解包 ✓／`Tuple`＝解包组／`NotTarget`）＋
     源码目标嵌套组递归；emitter 把 AssignTuple 抽成递归 `emit_unpack_assign`（每层一条 UNPACK，
     位点＝那层跨度 ✓）；**顺带把 `STORE_FAST_STORE_FAST` 由"恰好两项"推广成按对融合**
     （参照实测：`a,b,c,d=x` 两条融合、`a,(b,*c)` 在 UNPACK_EX 后也融 ✓）。
  2. **`with … as (a,b)`**：`Statement::With` 的 target 改 `Option<ForTarget>`（复用 for 目标）。
  3. **推导式模式目标**：`ComprehensionTarget` 改递归 `Group`（`[off]` 是解包组，方括号不当分组 ✓）。
  4. **调用基底下标赋值** `f(...)[k]=v`：整段表达式解析后按"后面是否 `=`"分流成目标链
     （求值次序 value→container→key 对参照 ✓）。
  5. **`.05` 前导点浮点**：词法层 `.`＋数字 ⇒ FLOAT（位点从 `.` 起 ✓）。
  6. **元组显示里的 `*`**：首项与后续项 `*expr`；尾随逗号并入显示跨度（`x = *a,` 位点对参照 ✓）。
  7. **行内 if 分号表＋次行 else**：词法新增 `Lexeme::Semicolon`（不再折成 `Newline`）；行内体只吃
     分号不吃换行 ✓。
  8. **带注解赋值的非名字目标**：`self.x: int = 1` ⇒ AssignAttr/AssignSubscript（注解不求值，
     与 3.14 对非简单名字目标一致 ✓）。
  9. **制表符缩进**：缩进宽度按 tabstop=8 归一；**tab 注释行/空行不参与缩进判定** ✓；列号仍按字符 ✓。
  10. **`*` 后置关键字实参**：允许 `*` 在关键字之后（`**` 之后仍照参照报语法错 ✓）。
- **验收** ✓：新增 `crates/pyawa-core/tests/parse_batch_shapes.rs`（**18 个形状**，期望指令流＋
  `co_positions` 由本机 3.14 `dis`/`op.positions` 一次性导出、非手写 ✓）＋
  `crates/pyawa-runtime/tests/parse_batch_shapes.rs`（同一段脚本两侧逐行一致，钉值＋求值次序＋
  注解不求值＋tab ✓）；`compile` 逐字节 4/4 ✓；`lib_compile` 全量 ✓；conformance 无新差异 ✓；
  quickcheck／slowcheck 十项全绿 ✓、0 警告 ✓。
- **收账** ✓：11 个模块／14 个文件同步进 `Lib/`（`difflib`／`gettext`／`xml.etree.ElementTree`／
  `tomllib`×4／`_pyrepl.{base_eventqueue,keymap,windows_eventqueue}`／`argparse`／`platform`／
  `pstats`／`test.typinganndata.ann_module3`）——两道必检 11/11 ＋ `startup ok` ✓；
  **顺带修好 `sync_lib.py` 的 11 条无 `.py` 陈旧 SLICE 条目**（`--sync` 现 rc=0 ✓）。
- **既有发射偏差（如实记，非本轮引入、无夹具覆盖，抽例文件头注已记）** ✗：`with …: pass` 少一条
  `NOP`；方法调用 `LOAD_ATTR` 的 NULL 位形态；常量条件 `if True:` 不折叠。
- **写域如实记** ✗：为"解析器不与语义分离"，除 parser.rs 还改了 emitter.rs＋symbols.rs＋compile.rs
  （task-8 允许面，但**未先报 Lead**——事后如实说明，本轮放行 ✓）。

## task-9 侦察 ✓（round 4 弹药：`_signal`／`posix.getcwd` 面）

落点 `target/recon/cmodule_recon.md`：
- **`_signal` 导入期必需恰好两项**：`_signal.signal`＋`_signal.getsignal`（`@_wraps` 装饰期读
  `__doc__`）；其余 4 个由 `from _signal import *` 决定。**硬前置是 enum 不是 `_signal`** ✗：
  `Lib/enum.py` 是过渡假货、没有 `_convert_` ⇒ `signal.py:7` 先炸；上游真 enum.py 现在也跑不动
  （`TypeError: 'NoneType' object is not iterable`）⇒ round 4 二选一：假货补 `_convert_`（要
  `setattr` 出 `Signals`/`Handlers`）或先修 VM 换真 enum。
- **`posix.getcwd/readlink`**：60 处调用／22 文件；`readlink` 能力域槽**已存在**只是填 `None`
  （`fs.rs:117`）⇒ 三处实现即可；`getcwd` 无槽 ⇒ 最小路径 `std::env::current_dir()`（不在
  T-CX-4 禁止面，同文件有 `std::env::vars` 先例 ✓）。**实测**：补上 getcwd/readlink 后
  **sysconfig 直接导入成功** ✅。
- **依赖序实测**（round3 overlay_fixed＋stub 复测）：`_signal` 不是第一道闸——unittest 修完
  difflib 后要 `_signal`＋enum；subprocess→`_posixsubprocess`；multiprocessing/test.support→
  `os.getcwd`；pickle→内建 `dir`；urllib.request→`sys._getframe`。
- overlay 上限复测脚本已备好（`target/recon/ceiling_rerun.sh`，未跑——等树稳定 ✓）。

## Round 4（posix C 模块面第一格）落地 ✓

- **HEAD**：`f838bb3`（`dev`，**已推送** origin ✓）—— task-10（getcwd/getcwdb/readlink）＋ docs 收口
- **判据①**（重建 conformance 后实测 ✓）：**通过 344 ＋ 参照口径 17 ＝ 361 ÷ 628 ⇒ 57.5%**
  （区间 360–362；round 3 末 358 ⇒ **+3**＝`sysconfig`＋`sysconfig.__main__`＋`_aix_support` ✓）
  - 战役累计：208 → 296 → 306 → 341 → 358 → **361**
- **round 3 后的 overlay 上限复测**（`target/recon/ceiling_overlay2.txt` ✓）：**341/628**——前三大族
  ＝`_signal`（52）／`os.getcwd`（49 ⇒ 本轮已收首格）／`_socket`（42）；其余：`sys._getframe` depth（9）／
  CJK（22）／genexp 自由变量 panic（6，**decimal 一族**）／`unicodedata`（6）／内建 `dir`（6）／
  `_operator`（2）／`__classdict__` cellvars（2，calendar/mailbox）。

## task-10 落地 ✓（posix.getcwd／getcwdb／readlink ⇒ sysconfig 族直通）

- **修法** ✓：`readlink` 走能力域槽（`fs.rs:117` 的槽此前填 `None`）：`instance/fs.rs` 两段式出参
  `fs_readlink` ＋ `runtime/fs_posix.rs` provider（照 `fs_stat` 范本）＋ `posix_module.rs`
  `readlink_native`（参照签名 `(path, *, dir_fd=None)`；errno 口径照参照：ENOENT/EINVAL/ELOOP）。
  `getcwd`／`getcwdb` 走 `std::env::current_dir()` **最小路径**（不在 T-CX-4 禁止面 ✓；原则路线
  `CpFsVtable` 槽留后续轮 ✗ 如实记）。
- **顺带修** ✓：①`posix.__all__` 的**陈旧空覆盖**（末尾一段把整张表冲掉 ⇒ 实测恒 `[]`——本层
  `import *` 不看 `__all__` 所以没露出来）；②`io_module.rs` 的 `strerror` 补
  EINVAL/ELOOP/ENAMETOOLONG，`fs_error` 拆 `fs_error_with_display`（bytes 路径文案；errno⇒类名/文案
  映射仍一处真相 ✓）。
- **语料陷阱照办** ✓：conformance harness 的 fs provider 补 `harness_readlink`（否则被测侧恒
  「槽未实现」＝假阳性）＋ 一条语料 `posix_cwd_readlink.py`。
- **验收** ✓：17 行探针两侧逐行一致（含 str/bytes 路径、`/proc/self/exe`、错误形态）＋
  `posix_fs_shapes.rs` 1 例 ✓；收账 3 文件（`sysconfig/__init__.py`／`__main__.py`／`_aix_support.py`）
  逐字节与上游一致 ✓，两道必检 3/3 ✓。
- **如实记** ✗：①`sysconfig.get_paths()` 运行期仍撞 `sys.abiflags`（`sys` 面缺名——不影响 import
  判据口径，但运行期调 `get_paths` 的模块还会红）；②`readlink(dir_fd≠None)` 如实
  `NotImplementedError`（`CP-21` 未接线）；③`getcwdb` 非 UTF-8 cwd 走 lossy。
- **参照纠错** ✓：`os.readlink` **没有 `strict` 参数**（3.14.4 实测传 `strict` 报 unexpected keyword；
  任务书里那条「strict 照参照签名」按实测口径收回 ✓）。

## round 4 期间的用户交互与决策

- **enum 路线（`_signal` 族 52 格的前置）已问用户、暂未收到答复**（询问被打断）⇒ round 5 先走
  无前置的小格批（task-11）；enum 决策悬而未决，**不替用户拍板** ✓。

## Round 5（task-11 小格批）落地 ✓

- **HEAD**：`07b996d`（`dev`，**已推送** origin ✓）—— task-11 四格 ＋ 通用编译器真 bug 三处 ＋ 收账 11 模块
- **判据①**（重建 conformance 后实测 ✓）：**通过 355 ＋ 参照口径 17 ＝ 372 ÷ 628 ⇒ 59.2%**
  （区间 371–373；round 4 末 361 ⇒ **+11**＝同步的 11 个模块 ✓）
  - 战役累计：208 → 296 → 306 → 341 → 358 → 361 → **372**（本会话 +76；距 421 还差 49）
  - 进度指标：`Lib/` **439** 文件 ⇒ 356 能 import（81.1%）

## task-11 落地 ✓（四格＋顺带三处通用编译器真 bug）

- **①内建 `dir`** ✓（`builtin_objects.rs` 的 `dir_new`＋stdlib 注册）：无参（当前帧局部／模块命名空间）／
  模块／类型对象（MRO 各类型字典键）／实例（MRO＋实例字典）；排序去重、非字符串键跳过。
  **如实范围** ✗：不调自定义 `__dir__`；内建类型方法在 getattr 槽**不进类型字典** ⇒ `dir(str)` 面比参照窄
  （抽例只钉两侧一致的面 ✓）。⇒ pickle／pickletools／shelve／logging.handlers 直通 ✓。
- **②`_operator` 别名** ✓：operator 模块加 `build_underscore`（同一套函数＋`_compare_digest`，文案逐字照
  参照）⇒ hmac／secrets 通 ✓。
- **③genexp 自由变量 panic** ✓：**根因实测定**＝`collect_lambda_demands` 漏收 AssignAttr 等十余种语句臂里
  的 genexp 需求 ⇒ 参数没升 cell ⇒ `emitter.rs:6764` 的 `.expect()` 在**合法程序**上 panic
  （`import decimal` 即炸 ✗）。补齐语句臂 ⇒ decimal／_pydecimal／_pylong／statistics 通 ✓。
- **④`__classdict__` cellvars** ✓：**根因实测定**＝类体**内联推导式目标**也进 `varnames`，而
  `MAKE_CELL __classdict__` 槽号写死 0 ⇒ 撞格 ⇒ 新增 `symbols::collect_class_locals`（只收内联推导式
  目标）预收＋三处槽号动态化 ⇒ calendar 通 ✓；mailbox 的编译错已消（下一格＝`_socket` ✗ C 模块）。
- **顺带清掉的三处通用编译器真 bug** ✓（比四格本身更通用，均有回归钉住）：
  1. **裸槽超指令遇 cell 必须让路**：`LOAD_FAST_BORROW` 一族按 localsplus 槽直读 ⇒ cell（含形参 cell）
     槽里是 **cell 对象** ✗ 被当值外泄（`self.flags = flags` ＋ genexp 捕获即炸）⇒ 新增
     `plain_local_slot` 统一判据 ✓。
  2. **延迟回填 `LOAD_CONST` 下标越过 255 未补 `EXTENDED_ARG`**：`pending` 的"上界 ≤ 191"只是**发射时**
     估计 ✗（嵌套 def 的 `intern_constant(Code)` 当场入池会打破）⇒ `pickletools.py:165` 的 `-1` 真下标
     379 截成 123 ⇒ 读到**别的常量** ✗ ⇒ 改走与跳转加宽同一套 `wide_jumps` 机制 ✓（抽例含 320 常量
     加宽回归 ✓）。
  3. **import 收尾重读 `sys.modules[name]`**：`decimal.py` 的回退会把 `decimal` 换成 `_pydecimal` ⇒
     先前返回**半截模块** ✗。
- **stdlib 顺带面** ✓：`operator.truediv`；`sys.hash_info`（值逐字照 3.14.4）；`math.erfc`（**如实偏差** ✗：
  A&S 7.1.26 逼近、|误差|<1.5e-7，非 libm 正确舍入）＋`math.sumprod`；`itertools.groupby`（**如实偏差** ✗：
  参照惰性、本层急求值列表，与 map/enumerate 先例同口径）；`int.bit_length` 类型级描述符；`float.fromhex`；
  `bytes(<生成器>)`；内置函数 `__name__`/`__qualname__`。
- **`Lib/enum.py` 过渡版** ✓：`global_enum` 直接挂普通 `class Month(IntEnum)` 语法时（calendar 一族），
  没有 `_member_names_` 表就**当场从 `__dict__` 收一份**（与 `_simple_enum` 同口径）再注入模块。
  **`_convert_`（signal.py 那条）仍未动——等用户拍板** ✓。
- **收账** ✓：11 个模块同步（两道必检 11/11 ＋ `startup ok` ✓；SLICE 补 11 条、`--sync` rc=0 ✓）。
- **写域如实记** ✗：动到写域外文件（`executor/import.rs`／`instance.rs`／`builtin/{float,int}.rs`／
  `builtin_objects.rs`／`lib.rs`／stdlib 六个模块）——逐项理由在汇报与注释里，本轮放行 ✓。

## round 5 之后的队列（overlay 复测后口径）

- **`_signal`（52 格）仍等 enum 路线拍板**（用户已问未答：扩过渡版 `_convert_`／修 VM 换真 enum／暂缓）。
- 其余大族：`_socket`（42，`P3-29` 已立项）／CJK `_codecs_*`（22）／`select`／`_tracemalloc`／
  `_posixsubprocess` 等 C 模块面；`dir` 的 `__dir__` 协议与内建类型方法名进类型字典（`dir(str)` 完整面）。

## Round 6（socket 战役第一轮，P3-29）落地 ✓

- **HEAD**：`9d5b40b`（`dev`，**已推送** origin ✓）—— `_socket` 最小面 ＋ `Lib/socket.py` 同步 ＋ enum 过渡版 `_convert_`
- **判据①**（重建 conformance 后实测 ✓）：**通过 357 ＋ 参照口径 17 ＝ 374 ÷ 628 ⇒ 59.6%**
  （区间 373–375；round 5 末 372 ⇒ **+2**＝`socket`＋`logging.handlers` 顺带 ✓）
  - 战役累计：208 → 296 → 306 → 341 → 358 → 361 → 372 → **374**（本会话 +78；距 421 还差 47）
  - 进度指标：`Lib/` **440** 文件 ⇒ 358 能 import（81.4%）

## task-12 落地 ✓（`_socket` 最小面 ⇒ `import socket` 通）

- **侦察** ✓（先量后写）：上游 `socket.py` 导入期真正要的面＝`import _socket`＋`from _socket import *`、
  `class socket(_socket.socket)` **可子类化**、`hasattr` 探测（sendmsg/recvmsg 真、share 假、socketpair 真）、
  模块级默认参数、`os._get_exports_list`、**`IntEnum/_IntFlag._convert_` 四次＝硬前置**。
  参照实测：`dir(_socket)` 499 名（456 int 常量＋2 字符串＋`has_ipv6=True`＋28 函数＋类/异常面＋CAPI）。
- **实现** ✓（`_socket_module.rs` 新 582 行）：常量/`has_ipv6` **真值逐个照参照**；`socket` 类（27 方法名，
  可子类化）；`error is OSError`／`timeout is TimeoutError`／`gaierror`/`herror` OSError 子类；28 函数的
  名字＋**参照同型参数形态**（错参 `TypeError` 文案两侧一致 ✓）。**诚实边界（硬）**：I/O 面一律
  `NotImplementedError`（无 syscall、无假 fd、无假连接 ✓）；CAPI／实例属性未落地、模块头如实记 ✗。
- **抽例** ✓：`socket_surface_shapes.rs` 2 例（表面逐行一致 36 常量＋异常身份＋`_member_map_`＋16 条
  TypeError 文案；如实报错面无假成功 ✓）。
- **`Lib/enum.py` 过渡版 `_convert_`（Lead 授权项，⚠ 台账显著标注：用户可否决回退）**：
  classmethod 签名照真 `enum.py:880`，语义＝从模块名空间收 `filter` 为真的名字造 `cls` 子类写回模块；
  只为 `socket.py` 四处模块级 `_convert_` 顶包；成员值仍是普通值（与 `re._simple_enum` 同口径）；
  `_build_enum` 与 `_simple_enum` 共用；**换回真 enum 流程不受影响** ✓。
  **授权链**：用户已拍板立项 `P3-29`（socket）⇒ `_convert_` 是其硬前置 ⇒ Lead 按 round 5 `global_enum`
  回退的先例放行（同文件、同种过渡补丁）✓。
- **收账** ✓：`Lib/socket.py` 逐字节同步（SLICE 补）；两道必检 ✓（`socket 2 1 True`／`startup ok`）。
- **链侦察（overlay3 实测）** ✓：`_socket` 族 **42→0**；下一格＝`_signal`（**asyncio 族 118**！）／
  `select`（socketserver/selectors/logging.config/pty）／**enum 真成员面**（`http.client` 等 5 格要
  `__members__`＋成员实例属性——假货造不出 int 子类实例 ✗，与"换回真 enum"同一拍板）／`_ssl`。

## enum 决策的现状（⚠ 用户待拍板，影响 100+ 格）

- 假货已补过两笔过渡面（`global_enum` 回退＝round 5；`_convert_`＝round 6，均已提交、可整体回退）。
- **`_signal` 族（118，asyncio 全族）**要 `_signal` 模块本体＋enum 面；**http 一族（5+）**要真成员
  （`__members__`＋成员实例属性）——**这两族只能靠「修 VM 换回真 enum」或更深的假货**，超出 Lead
  授权口径 ⇒ **升级给用户三选一**：①换回真 enum（还清债务）；②深挖假货（不推荐：`__members__` 要
  int 子类实例，等于把真 enum 的难点在假货里重做一遍）；③暂缓，先收 CJK（22）／`select` 等。

## Round 7＋8（C 模块最小面战役 ＋ `__file__`）落地 ✓

- **HEAD**：`d89d0ff`（`dev`，**已推送** origin ✓）—— `_signal`/`_posixsubprocess`/`select` 三模块最小面
  ＋ `__file__` 修复 ＋ 收账 5 个 Lib 文件
- **判据①**（重建 conformance 后实测 ✓）：**通过 362 ＋ 参照口径 17 ＝ 379 ÷ 628 ⇒ 60.4%**
  （区间 378–380；round 6 末 374 ⇒ **+5**＝`signal`＋`selectors`＋`subprocess`＋`ftplib`/`mailbox`/`poplib` ✓）
  - 战役累计：208 → 296 → 306 → 341 → 358 → 361 → 372 → 374 → **379**（本会话 +83；距 421 还差 42）
  - 进度指标：`Lib/` **450** 文件 ⇒ 363 能 import（80.7%）

## task-13 落地 ✓（`_signal` 最小面 ⇒ `import signal` 通；asyncio 族 118 格的第一闸拆掉）

- 侦察 ✓：导入期必需＝`signal`＋`getsignal`（`@_wraps` 装饰期读 `__doc__`）；`from _signal import *`
  的**导出面**决定四个可选包装是否定义 ⇒ **收窄**（45 常量＋2 函数）⇒ 少接线 ✓。参照 `dir(_signal)` 71 名。
- 实现 ✓：45 常量**真值照参照**（`NSIG=65` 单一真相点）；`signal`/`getsignal` 完整参数校验语义（**16 条
  TypeError/ValueError 文案两侧逐字一致**，含 bool handler 边界）；**诚实边界**：校验过后一律
  `NotImplementedError`——`getsignal` **实测定案不返回 `None`**（参照对有效号码从不给 `None`）✓。
- 链侦察 ✓：`_signal` 族 118→0；asyncio／multiprocessing／subprocess 的下一格**同根＝`_posixsubprocess`**（
  multiprocessing 表层报错被吞成 `cannot import name 'context'`，用副本解开会诊钉死）；`test.support` 族
  新暴露 `__file__` 缺失（32 格）。

## task-15 落地 ✓（`_posixsubprocess`＋`select`＋posix wait 族 ⇒ `import subprocess` 通）

- 侦察 ✓（三格同收）：①`fork_exec` 只绑定不调用 ✓；②`class _del_safe` **类体导入期读** `os.waitpid`/
  `waitstatus_to_exitcode`/`WIFSTOPPED`/`WSTOPSIG`/`WNOHANG`（本仓全缺）；③`import select`＋`import selectors`。
- 实现 ✓：`_posixsubprocess`（`fork_exec` 22 参＋kwarg 拒绝文案，调用如实未接线）；`select` **收窄面**
  （只导 `PIPE_BUF`＋`select`，**刻意不导** poll/epoll ⇒ `selectors` 走 `SelectSelector` 回退——参照本机
  EpollSelector，**已知偏差**钉在抽例；导出外壳反而挡 import：`_can_use` 的 except 元组不含
  `NotImplementedError`）；posix wait 族（`WNOHANG` 常量＋`WIFSTOPPED`/`WSTOPSIG`/
  `waitstatus_to_exitcode` **纯逻辑真实现**、逐值对参照；`waitpid` 如实未接线）。
- 收账 ✓：`Lib/{selectors,subprocess}.py` 同步；**顺带解锁** `socketserver`/`logging.config`（overlay 口径，
  未同步——链上还有格）；抽例 2 例全绿 ✓。

## task-14 落地 ✓（模块帧 `__file__` ⇒ 脚本/文件模块/包三形）

- 参照口径 ✓（先实测）：脚本本体＝**绝对路径且不解析符号链接**；文件模块/包＝**加载时路径原样**；
  Rust 侧内建模块不装（参照 builtin 同理 ✓）。
- 实现 ✓：`executor/import.rs`（文件加载装 `__file__`）＋`bin/pyawa.rs`（CLI 经**既有** `pa_pushstring`/
  `pa_setglobal` 注入，**零 ABI 改动** ✓）。**`__package__`/`__spec__`/`__loader__` 未做（如实记）**：
  grep 证实导入期无人读、相对导入有 `__name__`/`__path__` 回退 ✓。
- 收账如实 ✗：**Lib 口径 0 收**（`test/support/__init__.py` 不在库且依赖 `unittest` 整包——后续轮）；
  **overlay 口径** test.support 族 **20 个里 11 个 import 通**（修前 0 ✓）。顺带收 `ftplib`/`mailbox`/`poplib`
  （socket/subprocess 链下游）✓。

## round 7/8 的过程事实（如实记）

- **并发测量坑再现** ✓：round 7 的 slowcheck 第 5 项（stability）红＝round 8 两名队员的在飞改动撞进闸门
  （conformance/lib_compile 挂在连跑里）⇒ 等两人都汇报、树稳定后重跑 ⇒ **十项全绿**（860s）✓——
  **并发时闸门读数不作数**，稳定后统一终审是对的口径 ✓。
- **ds-socket-camp 挖出的下一层（round 9 弹药，都有最小复现）** ✗：
  1. **循环相对导入缺回退**：`from . import context`（context 正在导入中）——CPython 靠 `IMPORT_FROM`
     失败时回退查 `sys.modules[完整名]`（issue #17636）；本仓没有 ⇒ multiprocessing 族 24 格卡这。
     复现 `target/recon/circprobe/`。
  2. **失败模块残留 `sys.modules`**：`import ssl` 失败后不把半截模块移除 ⇒ `try/except ImportError` 的
     `ssl=None` 分支失效 ⇒ asyncio 族 48 格卡这。复现 `target/recon/badprobe/`。
  3. `pty`⇒`termios`；`test.support.os_helper`⇒`sys.abiflags`；`sys.acquire_lock`、`set.union`、
     lexer 代理转义 `\U0000dc80`（core 可修面）。

## Round 9（task-16 core import 两格）落地 ✓

- **HEAD**：`8ed5e9f`（`dev`，**已推送** origin ✓）—— 循环相对导入回退 ＋ 失败模块清理
- **判据①**（重建 conformance 后实测 ✓）：**通过 365 ＋ 参照口径 17 ＝ 382 ÷ 628 ⇒ 60.8%**
  （区间 381–383；round 8 末 379 ⇒ **+3**＝`socketserver`＋`test.typinganndata.partialexecution`（包）＋`.a` ✓）
  - 战役累计：208 → … → 374 → 379 → **382**（本会话 +86；距 421 还差 39）
  - 进度指标：`Lib/` **448** 文件 ⇒ 366 能 import（81.7%）

## task-16 落地 ✓（IMPORT_FROM 回退 ＋ 失败模块清理——multiprocessing 24／asyncio 48 的同根闸）

- **①循环相对导入回退** ✓（CPython issue #17636）：`IMPORT_FROM` 属性失败（含模块 `__getattr__` 也没有）
  后回退查 `sys.modules[<module.__name__>.<name>]`，命中即压栈（值借用、push 自留一份 ✓）。
  复现 `target/recon/circprobe/` 两侧 `circular ok` ✓；抽例钉住 `pkg.a is sys.modules["pkg.a"]`、
  `from pkg import b` 逐行一致 ✓。
- **②失败模块残留 `sys.modules`** ✓：`load_module` 失败路径按参照 `_load_unlocked` 语义清理——**只在
  `sys.modules.get(name) is module` 时删**（执行期被换掉的不动 ✓），键/值引用都归还（新增私有
  `remove_module_entry`）。复现 `target/recon/badprobe/` 两侧 `after: False` ✓；抽例钉住重试仍失败
  （不假成功）、嵌套失败两格都清、**成功回退换模块不回归**（`decimal`→`_pydecimal`——与 task-11 那条
  是同一处收尾的两个方向 ✓）。
- **链上实测（如实）** ✗：`asyncio` 整包下一格＝**`CodeObject.replace` 未接线**（`Lib/types.py:315` 的
  `co.replace(co_flags=…)`）；`multiprocessing`／`concurrent.futures.process` 下一格＝**`array` C 模块**。
  `asyncio.sslproto` 单文件已通（②要救的那条）✓。
- **用户拍板（2026-10-17 16:11）** ✓：**enum 换回真货**——修 VM 缺口让上游真 `enum.py` 跑起来、替换
  过渡版假货（http 一族的 `__members__`/成员实例属性面）⇒ round 10 起侦察＋实施。

## Round 10（拆闸轮＋两份侦察）落地 ✓

- **HEAD**：`49b9fc4`（`dev`，**已推送** origin ✓）—— `CodeObject.replace` ＋ `array` 最小面
- **判据①**：**382 ÷ 628 ⇒ 60.8%**（区间 381–383，**持平＝预期**：本轮是拆闸轮，收益要等下一层）
  - 战役累计：208 → … → 379 → 382 → **382**（本会话 +86；距 421 还差 39）
- **⚠ 仪器问题（P0，ds-socket-camp 挖出，待修）**：conformance runner 走 ABI `pa_create` **不注入平台
  errno 常量**（CLI 在 `bin/pyawa.rs:162` 手动补了）⇒ **判据①/上限 census 有 ~50 格假红**
  （`'module' object has no attribute 'ECHILD'` 48 ＋ `ENOENT` 2）⇒ **382 是低估读数**。修法方向
  （`target/recon/type_method_descriptors.md` §8）：`pa_create` 注入（一处真相、删 CLI 手注入）／
  扩 ABI 宿主入口（需用户拍板）／只修 harness（治标）。

## task-17 落地 ✓（`CodeObject.replace`——asyncio 族的闸）

- 17 字段全支持（仅关键字／缺省＝原值／返回新对象）；跨字段校验**顺序**照参照反例实测钉住；两处必要防御
  （新对象常量表逐项 retain——第一版实测 heap 损坏 ✗；`co_lines()` 越界收尾不 panic ✓）。
- 顺带补 `co_code`/`co_exceptiontable` getter；如实偏差（`co_linetable` 未存等）已记。
- **新挖真 bug（挂账）** ✗：`function` 类型只有 getattr 无 `__code__` 写槽 ⇒ `types.coroutine` 空操作。
- 链上：`asyncio` 下一格＝`sys._clear_type_descriptors`（stdlib sys 面）。

## task-18 落地 ✓（`array` 模块最小面——multiprocessing 族的前置）

- 导入期用法只有"取类型属性"（`managers.py:50` 的 `register(array.array, …)`）⇒ 空数组薄载荷**真做**
  （14 个 itemsize 真值、构造校验文案照参照）；**诚实边界**：非空构造 `NotImplementedError`、
  16 个元素级方法不登记。
- 链上：multiprocessing 下一格＝**`type(list.append)` 类型级方法描述符**（core 缺口，8 个全 AttributeError，
  复现 `target/recon/typeattr.py`）；其后＝`heap` 链式赋值编译缺口／`mmap`／`_multiprocessing`。

## 侦察交付（round 11 弹药，全在 `target/recon/`）

- **`type_method_descriptors.md`（313 行）**：导入期硬缺恰 3 名（`list.append`/`int.__add__`/
  `float.__getformat__`）；90 项参照分类（method_descriptor 41／wrapper_descriptor 31…）；
  修法两案（A＝逐名照 `int.bit_length` 先例；B＝真描述符类型，影响 `types.py`/`copyreg` 类型常量，
  专项）；风险评估（dir 抽例只钉宽松面 ⇒ 预计不带红）。
- **`enum_real_campaign.md`（28KB，用户拍板「换真货」的弹药）**：真 `enum.py` 能执行到第 36 条
  顶层语句；缺口三层**严格依赖序**——**G1**（emitter：`if` 块内循环带 `break` ⇒ 退出被换成作用域
  return；根因 `emit_rest_and_tail` 排序）→ **G2**（对象模型：内建类型 `__new__` 全塌成
  `object.__new__` ⇒ `int.__new__(子类,值)` 不可用；**http 的 `HTTPStatus.__new__` 同根**）→
  **G5**（类创建：`__set_name__` 里 `type(owner)` 不是元类）。副作用族 G6（元类 property 不生效——
  **http.client/xmlrpc 当前红在 `__members__` 的最后一格**）／G4（类 repr）／G3（内建 `__dict__` 缺
  dunder）／G7（元类实例调用 UB）。**换货风险面**：13 个 enum importer 逐个核过，**没有靠假货行为
  才能过的绿模块**；切批 B1→B7 每批带验收命令；工具（`enum_sim_bisect.py`）会自动接力钻后续格。
- **顺带发现（单独立项）** ✗：import 在候选模块**编译失败时静默回退**到下一个 `sys.path` 项
  （CPython 是 SyntaxError 直接失败）——会掩盖 bug。

## Round 11（errno 仪器修复 ＋ enum 换真货 B1–B3）落地 ✓

- **HEAD**：`f3a5754`（`dev`，**已推送** origin ✓）
- **判据①**（重建 conformance 后实测，**提交态** ✓）：**通过 369 ＋ 参照口径 17 ＝ 386 ÷ 628 ⇒ 61.5%**
  （区间 385–387；round 10 末 382 ⇒ **+4**＝errno 假红 3 格＋`webbrowser`＋1 顺带）
  - 战役累计：208 → … → 382 → 382 → **386**（本会话 +90；距 421 还差 35）
  - 进度指标：`Lib/` **449** 文件 ⇒ 370 能 import（82.4%）

## task-20 落地 ✓（`pa_create` 注入 errno——P0 仪器修复，用户拍板）

- **一处真相** ✓：`HOST_ERRNO` 表**逐字节**搬 `pyawa-runtime`→`pyawa-abi`（依赖图查过：`pa_create`
  只能依赖 abi 层，未造环；`gen_errno.py` 路径同步、重跑字节不变）；`pa_create` 成功路径注入
  （**ABI 签名未动**）；删 CLI／`paL_newstate`／`PaState::new` 三处手注入 ⇒ CLI 与 runner 同源 ✓。
- **仪器护栏** ✓：conformance runner（`pa_create` 路径）断言 `errno.ECHILD/ENOENT` 必在、
  `errorcode` 非空——**钉住测量仪器不再回退** ✓。
- **归因如实** ✓：判据① 只 +3（`subprocess`/`optparse`/`logging.config`＝旧 errno 假红恰三格）；
  "~50 格"是 **census 族计数**（多为 upstream-only 模块）⇒ overlay census `errno` 两族**归零**
  （377→382），其余前进到真缺口（`sys.abiflags` 43／`_clear_type_descriptors` 36／`type.append` 24／
  `sys._getframe` 11）。
- **措辞同步 5 处** ✓（DESIGN §9.20／REQUIREMENTS／SPEC-c-modules／stdlib README／instance.rs 注释）：
  「由 `pa_create`（`pyawa-abi`）创建实例时注入生成表」——全库旧措辞**归零** ✓。

## task-21 落地 ✓（enum 换真货 B1–B3 ⇒ **上游真 enum.py 整文件 import 成功 53/53**）

- **B1（emitter）** ✓：`if` 块内循环带 `break` ⇒ `LoopFrame` 记块深度、抄余部后跳**块尾**；**次序坑**
  （侦察没列）：该判断必须排在「外层循环回跳」**之前**（否则夹层 `if` 跳过块后代码／死循环 ✗）⇒ 修后
  `min_break` 族与参照逐行一致、**compile 逐字节 499 条全绿** ✓。
- **B2（对象模型）** ✓：7 个数据类各挂**自己的** `__new__`（造子类型实例、错参文案逐字照参照）；
  构造分派改比**原生函数指针**（`is_default_new`）。顺带修 `str/bytes` 子类按值比较、`set` 子类 repr
  （真 enum 语义必需的同根）。
- **B3（类创建）** ✓：元类型先落到类对象再跑 `__set_name__`；**#37 真因**＝类字典填充与 `__set_name__`
  同一循环交错 ⇒ 改**两遍走**（照 CPython `type_new_set_attrs`+`set_names`）。
- **另拆三道同类闸** ✓：类对象 `repr/str` 按元类 MRO、类对象特殊方法（迭代）按 `type(obj)` MRO、
  `function.__get__` 描述符。
- **战果** ✓：上游真 `enum.py`（只读副本）**53/53 条顶层语句 import 成功**（`enum_prefix_bisect.py`
  无可二分失败）；`Enum`/`IntEnum`/`auto`/`repr` 基本语义两侧对拍一致 ✓。
- **下一层真格（如实挂账）** ✗：**G6 元类 property**（`__members__`——http 一族最后一格）／
  `dict.update(mappingproxy)`／`staticmethod.__func__`／int 子类算术 dunder 覆写／G3 数据类 dunder
  字典。**B6（换 `Lib/enum.py` 真本）等 G6 族处理完再做** ✓（假货本轮未动，还挂着
  socket/signal/calendar 的绿）。
- **收账** ✓：`webbrowser.py` 同步（SLICE 已补）；`enum_real_shapes` 抽例两侧逐行一致 ✓。

## Round 12（enum 真本落地＋类型级描述符）落地 ✓

- **HEAD**：`23a7532`（`dev`，**已推送** origin ✓）—— enum B5＋B6（换真本）＋描述符/sys 面
- **判据①**（重建 conformance 后实测，提交态 ✓）：**通过 371 ＋ 参照口径 17 ＝ 388 ÷ 628 ⇒ 61.8%**
  （区间 387–389；round 11 末 386 ⇒ **+2**＝`mimetypes`＋`http.client` ✓）
  - 战役累计：208 → … → 382 → 386 → **388**（本会话 +92；距 421 还差 33）
  - 进度指标：`Lib/` **450** 文件 ⇒ 372 能 import（82.7%）；**CX-8 例外清单归零**（enum.py 换真本 ✓ 清债点）

## task-22 落地 ✓（enum 换真货 B5＋B6——**`Lib/enum.py` 已是上游逐字节真本**）

- **B5 四格** ✓：G6 元类 property（类属性访问补 `type.__getattribute__` 次序——元类数据描述符优先）⇒
  `Color.__members__` 给映射内容（http/re/socket/calendar/signal/pstats/inspect/py_compile 全转绿 ✓）；
  `dict.update` 补映射协议；**B5-3 真凶**＝`type.__new__` 缺「类体 `__new__` 自动包 staticmethod」⇒ 补包＋
  三条分派路径剥壳；int 子类算术 dunder 优先（`IntFlag` 组合伪成员正确）；G3 数据类 `__str__`/`__repr__` 面。
- **B6 换本** ✓：13 个 importer 换前全绿、换后 **14 个（+http.client，无减少）** ✓；假货的 `_convert_`/
  `global_enum` 两笔过渡债随文件消失 ✓。
- **顺带真 bug** ✓：①链式赋值给 `global` 名字误发 `STORE_NAME`（`mimetypes` 形状）⇒ 统一 `emit_store_name`；
  ②`object.__str__` 只走 repr 槽 ⇒ 改走 `__repr__` 覆写＋原生函数指针防自递归。
- **http.server 立据（⚠ 新挖内存安全 bug，未修）** ✗：`BaseHTTPRequestHandler` 类体执行期 **SIGSEGV**
  （gdb：`iterator_type_for → is_subtype(毒化指针) → TypeObject::mro`——有 TypeObject 被引用时被释放）；
  触发链刚被 mimetypes 修通 ⇒ 按红灯停手，**单列下一轮内存安全格**。
- **收账** ✓：`mimetypes.py`＋`http.client`（`HTTPStatus.OK.phrase` 等语义两侧对拍 ✓）；`enum_shapes`
  抽例按真本重钉＋改名（不再叫 `fake_enum` ✓）。

## task-23 落地 ✓（类型级方法描述符 3 名＋sys 两格）

- **方案 A**（照 `int.bit_length` 先例）：`list.append`（receiver-first 转调 `list_append_native`，子类可绑）／
  `int.__add__`（右非 int ⇒ `NotImplemented`）／`float.__getformat__`（纯函数真实现
  `'IEEE, little-endian'`）——16 条错误文案逐行对参照 ✓。
- **sys 两格**：`abiflags=''`＋`_clear_type_descriptors(type,/)`（幂等 no-op 如实标偏差）。
- **链上下一格（如实）**：`multiprocessing` ⇒ **`dict.update` 要接 (k,v) 可迭代**（`__init__.py:23`）；
  `asyncio` ⇒ **`itertools.chain.from_iterable`**；`test.support` ⇒ **`_imp.acquire_lock`** 族
  （前缀二分钉到 `check_sanitizer` → `sysconfig.get_config_var` → `_bootstrap._setup(sys,_imp)`）。
- **方案 A 已知偏差** ✗：`type(list.append)` 仍是 `builtin_function_or_method`（参照 method_descriptor）⇒
  `_extra_reducers` 两 key 合并——真描述符＝方案 B 单独立项。

## round 12 过程事实

- **换本带红了一个老测试** ✓：`enum_shapes.rs` 的 `fake_enum_surface_used_by_re` 钉的是假货面 ⇒ 换本后
  slowcheck 第 3 项红 ⇒ 按**参照实测**重钉（非按我们输出凑）＋改名，顺带修出 `object.__str__` 真语义差 ✓
  ——**换真本必须连带清掉钉假货的旧断言**，这条经验入档。
- 两任务的 `arithmetic.rs` 改动交叠 ⇒ 合一笔战役提交（消息分列两单元）✓。
