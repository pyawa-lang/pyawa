# NEXT（会话 11 进行中 —— M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407、已知差异 0、新差异 0；`MS-19` 补充随 `P3-38` surrogate epic 收口
> （`Lib/test` 语法错 0／可编译 397/397＝100%）；判据① 477/628＝76.0%；`Lib/` 逐字节同步 739 文件；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役（会话 11）只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。

- **HEAD**：`dev` 见 `git log -1`（round 45：嵌套 `with` 退出路径修复 ⇒ `test_frame` 状态 5 解除，官方套件「未实现」90→83）；`main` ＝ M6 合并提交（tag `m6`）。
  **批次进度**：7 轮交付已全部落地（round 32–38）；**追加批**：round 39＝`test_frame` 慢因（GC O(n²) ⇒ 自适应水位；超时 9⇒3），
  round 40＝`test_userstring`（`str.startswith/endswith/count` 的 `start/end` ＋ `find/count` 的 `None` ＋ `split/rsplit` 的 `None`/`maxsplit`；
  **`weakref.ref` 相等/哈希按目标** ⇒ `WeakSet` 去重、`_abc_cache` 不再一次长一条）。该测试：941 failures／9 errors／>900 s ⇒ **38 failures／4 errors／42 s**（仍是 runtime、不超时）。
  round 41＝`map`/`filter` **真惰性迭代器**（`ItStateKind::Map`/`Filter` ＋ 类型注册 ＋ 推进分支）＋ `dict.fromkeys` 通用可迭代：
  **`test_tempfile` 真 hang 解除**（无限挂死 ⇒ 118 测试／0.75 s 跑完），官方套件超时 **3⇒2**（只剩 `test_dict`/`test_weakref`）。
- **工作区**：干净（`git status --porcelain` 空）。
- **下一条命令**（续做时）：`git fetch && git reset --hard origin/dev`（历史会被重写 ✗，先核对）；
  **round 45 已清 ③**：`test_frame` 的状态 5（"指令 70 …既不是内建迭代器"）根因是**嵌套 `with` 两处
  编译器出口**（内层正常出口跳过外层退出调用 ✗；`break`／`continue` 不退循环体内的 `with` ✗）⇒
  修好后 test_frame **跑到完**（17F／19E／12 skip，剩余是 FrameLocalsProxy 语义缺口）、官方套件
  「未实现」90 → **83**。下一轮候选：FrameLocalsProxy 语义缺口（17F/19E，可试清一批）／
  `test_weakref`（形参所有权）／NEXT 队列的 `KeyError` 上下文污染与 `P3-35` 5 形态。
  **round 44 已清 ②**：`test_userstring` **38 failures／4 errors ⇒ OK（skipped=2）** ✓ —— 根因
  `==`／`!=` 没走 `__eq__`／`__ne__`（P3-42 核心格）＋ `str` 一族口径（fillchar／replace count／zfill
  符号／expandtabs 换行列／rpartition 空分隔符／搜索族 arity 报文／下标错型与大整数饱和／切片饱和
  算术／`%` 星号宽度与嵌套键／`str.__mod__`／生成的 title 映射表）；官方套件跑通 **21 → 22**。
  **另外 6 个 hang 的剩余账**：`test_dict` 已由 round 42 的 dict 哈希索引解除（超时⇒3.59 s）；
  **只剩 `test_weakref`**——round 43 用**对拍 refcount 增量**定位到四类"每次操作漏一份引用"并
  修掉（名字加载／`CALL` 的 `bound_self`／返回值 `push` 多 retain／`__call__` 转发），但**位置形参
  仍漏 1 份/调用**（`f(a)` ⇒ +100）：它是 `test_weakref` 剩余超时的主因；**下一轮（round 45）**＝
  "帧拥有形参槽"的所有权审计（round 43 试过 ⇒ refcount 全归零，但 `__del__`＋`gc.collect()`＋属性写
  暴露潜伏 UAF：test_dict/test_weakref SIGABRT/SEGV、gdb `release_object ← instance_attribute_set`
  ⇒ 已撤回；需同时审计属性写／终结器／异常回溯三条重入路径，再动 `frame_clear`）。
  另一格：`d.values()`/`d.items()` 是列表快照（不是视图对象）⇒ 迭代中改字典不报错。
  然后（按用户取活顺序）：**`P3-42` 剩余两格**——(a) 特殊方法派发缺口（`==`/`!=` 不走 `__eq__`、
  `__add__` 等不派发）；(b) `unittest` contextmanager 里 `KeyError` 污染断言异常 `__context__`。
  然后 **`P3-35` 剩余 5 形态**：`\N{…}`／`except*`／`import *`／async 推导式／PEP 695 `*Ts`。**`P3-35` 剩余 5 形态（未清，见 round 36 台账）**：`\N{…}` 15（要 Unicode 名字表
  生成进 core）／`except*` 7（PEP 654）／`import *` 4（模块帧失败导入静默成 None 的 import 机制缺口，
  证据在台账）／async 推导式 4／PEP 695 `*Ts` 2。**`P3-42` 剩余两格（未清）**：
  `KeyError` 族 12 行 = (a) **特殊方法派发缺口**（`==`/`!=` 不走 `__eq__`、`__add__` 等不派发，
  `executor/values.rs::values_equal` ＋ `protocol::compare_public`）＋ (b) `unittest` 的 contextmanager
  生成器里 `KeyError` 在测试体执行期间仍处"正在处理"⇒ 断言异常的 `__context__` 被污染；
  以及 `time.struct_time` 不是 `tuple` 子类（`test_structseq.test_tuple`）。
- **未修 bug＋判据**（如实，非 M6 gate）：
  - **官方套件（release）**：跑通 **20/397＝5.0%**（可编译 397/397、未实现 100／运行期错 269／超时 8）；语料 `MS-14` 407/407 **未动** ✓。
  - **真 hang（1）**：`test_tempfile` —— `map()` 急求值（`type_runtime.rs:485`）消费无限 `_RandomNameSequence`
    （`Lib/tempfile.py:373`）⇒ 无限循环＋无界内存；复现 `target/recon/hang2/repro_tempfile_min.py`。
    修法方向：`map`／`filter`／`zip` 改正惰性迭代器（未修 ✓）。
  - **慢（不是 hang，已实测）**：`test_dict`／`test_weakref`＝dict 线性扫描 O(n²)（`dict.rs:75`）；
    `test_long`／`test_peepholer`＝`unittest.subTest` 机器开销＋对象不释放；`test_userstring`≈15 min
    （次因 `startswith/endswith` **忽略 `start`／`end`**）。复现脚本：`target/recon/hang1/`、`hang2/`。
  - **`P3-42` 剩余（如实）**：`KeyError` 族 12 行（两层根因见上）；`time.struct_time` 非 tuple 子类；
    `str.startswith/endswith` 的 `start`/`end` 仍不收；`IntLike`（`int.__index__`）实参族。
  - **`P3-39` 剩余（如实）**：`StringIO` 的 `newline` 换行翻译、`__getstate__`／`__setstate__`／pickle。
  - **`P3-40` 剩余（如实）**：`os.close` 已关 fd 仍静默（参照 `EBADF`）、`get/set_inheritable`、
    `os.chmod`／`os.scandir`（顺带暴露的新层缺口）。
  - **`P3-35` 剩余**：`\N{…}` 15／`except*` 7／`import *` 4／async 推导式 4／PEP 695 `*Ts` 2（下标删除已落地 round 36）。
  - **`P3-29` 剩余（如实）**：IPv6／AF_UNIX 地址线、真超时语义（`settimeout` 只记值）、
    `_socket.poll`／`select` 的 socket 分流、`sendto`／`recvfrom`／`getsockopt`、`getaddrinfo` 一族（A2）。
  - **`P3-37` 剩余（如实）**：`pyexpat` 的**解析本体**（整块 expat C 扩展：XML/DTD/实体/命名空间/错误码语义）未移植；
    形状面已落（round 38），`test_pyexpat` 仍到不了断言层。
  - 其余旧账：`zipimport`／`urllib.request` 等缺模块、3 个编译器 cell/free 表 panic 用例、
    `FAILED(errors=1)` 泛化、"`tb_next` 链"、fs/io 其余（标准流 `dup`／`sys.prefix`）、`P3-38` 的 4 条有损输出通道边界。
- **闸门命令**：`tools/slowcheck.sh --release`（十项 ≈15–20 min；round 32–35 均全绿）＋
  `python3 tools/libtests_survey.py --release`（≈2.5 min）＋ `python3 tools/lib_import_ratio.py`（≈5–10 min）＋
  `python3 tests/ci/check.py`（14/14）。

## 本战役的关键口径（新会话必读）

- **一轮一落地**（`agents-rules/round-rule.md`）：每轮一笔提交（代码＋台账＋本页同笔）；闸门用 `&&` 只看 exit code；
  **slowcheck 十项全绿才提交**；提交 `git commit -F - -- <显式路径>`。报告三行：① 改了哪个 P3-xx ② 官方套件跑通数（前→后）③ 分诊里该签名是否消失。
- **`Lib/test` 体检口径**：`tools/libtests_survey.py`（397 个顶层 `test_*.py`；目录
  `target/upstream/extracted/usr/lib/python3.14/test`）＋ `tools/libtests_triage.py`。两个比例是**报告级**、不得当验收/完成度（`CM-15`）。
  **改编译器后必须清 `.pyac` 缓存**：`find target Lib -name __pyawa__ -type d -prune -exec rm -rf {} +`
  ＋ `find target/upstream -name '*.pyac' -delete`（陈旧判定只看源字节，看不到编译器版本 ✗）。
- **阶段边界**：`main` 只在阶段完成时以 `--no-ff` 收录（现 tag `m1`…`m6`）；**不 reset／不重写历史／不 force-push**
  （`agents-rules/branch-rule.md`）；历史已被重写过 ⇒ 续做先 `git fetch && git reset --hard origin/dev`（有未推送活儿则 `git rebase origin/dev`）。
- **ABI／接口版本**：`PA_ABI_VERSION`＝**1.4**（round 34 的 `fs.pipe` 表尾追加 ⇒ `PA_ABI_MINOR` 3→4；`AB-41`）；
  指令集版本 `INSTRUCTION_SET_VERSION`＝**2**。
- **`fd` 语义（`CP-31`）**：`fd`＝实例能力表下标，动态段从 **3** 起（0／1／2 标准流不占槽）；fs／net 共用一张表。
- **路径解析（`CP-21`/`CP-32`）**：VM 侧 guest 绝对路径空间解析；`sys.path` 最终归 `site.py`（`IM-24`），现在是过渡半。
- **编号现最大**：`BC-65`／`TS-50`／`T-BC-22`／`CP-39`／`CX-22`／**`P3-42`**（新增前先查已定义；墓碑算定义）。
- **`__pyawa__` 缓存坑**：量之前 `rm -rf Lib/__pyawa__`（＋ `target/**/__pyawa__` 与 `target/upstream/**/__pyawa__`）。
- **共享工作区**：代码＋台账＋本页**同笔**提交；提交前核对本页 HEAD／工作区两行；成员上限 8；`pyawa-core` 同时至多**两名**写手（不同子目录）。
