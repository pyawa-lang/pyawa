# NEXT（会话 11 进行中 —— M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407、已知差异 0、新差异 0；`MS-19` 补充随 `P3-38` surrogate epic 收口
> （`Lib/test` 语法错 0／可编译 397/397＝100%）；判据① 477/628＝76.0%；`Lib/` 逐字节同步 739 文件；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役（会话 11）只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。

- **HEAD**：`dev` 见 `git log -1`（round 70：**性能族**（字典/集合哈希索引＋超长字面量＋槽缓存）⇒ `namedtuple` 千字段 6.9 s⇒0.75 s；round 69：`compile()` 收 `__index__`＋原生函数 `__module__` ⇒ 跑通 35 ⇒ 37）；`main` ＝ M6 合并提交（tag `m6`）。
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
  **round 46 起手 ④**：真值协议（`__bool__` 必须恰返回 `bool`、`__len__` 整数且 `>=0`、类型对象恒真）
  修掉 `test_bool` 的 **Rust 栈溢出 abort**（abort ⇒ 6F／2E 跑完）；该文件剩余 8 格已列档：
  `bool` 不可继承、`int`／`bool` 的 `.real`／`.imag`、一元／位运算的 bool→int 结果类型、
  `bool.from_bytes` 单例、`bool(**kw)` TypeError、`BytesIO.getbuffer`（pickle）——**round 47 已收**
  **round 48 已清**：`test_bool` **全绿（OK）** —— `int.__new__`／`bool.__new__` 显式登记
  （`type.__new__` 的 new 槽桥接收口）＋ 核心侧 `DeprecationWarning` 通道（`~bool`，走
  `sys.modules['warnings'].warn`，惰性不主动 import）；官方套件跑通 **23 → 24**。
  **round 49 起手**：doctest 的导入前置面（`sys.gettrace`／`sys.stdin`／`sys.displayhook`／
  `settrace`／`setprofile`、隐式异常 `__traceback__`（含 `compile()` 的 SyntaxError）、
  `backslashreplace` 四档编码器）⇒ `test_extcall`／`test_genexps`／`test_unpack_ex`／
  `test_pep646_syntax` 从**导入即死**推进到真正的 doctest 用例。**下一轮（round 50）**：
  `DICT_MERGE` 两件事（`**映射` 走映射协议；同名键 TypeError 带被调函数 qualname）＋
  `test_genexps` 生成器 `throw` 的 `StackUnderflow` ＋ `test_pep646_syntax` 断言差异；
  **round 50 已清**：`DICT_MERGE`（映射协议＋同名键报文的被调对象限定名）与 `CALL_FUNCTION_EX`
  （任意可迭代 `*`、`keywords must be strings`、`str` 子类关键字名）⇒ `test_unpack_ex`／
  `test_pep646_syntax` 跑完但各 1F（**"Got nothing"＝`compile(..., "single")` 的交互式 repr 未接线**）；
  `test_extcall` 跑完 doctest 后**在 GC 回收时堆损坏**（`PYAWA_NO_GC=1` 下能跑完 ⇒ 疑似 round 49
  的"异常↔回溯↔帧"环；`test_extcall` 即最小复现）——**下一轮（round 51）优先级**：① GC 双重释放
  （正确性 bug，压一切）② `single` 模式交互 repr（一次可翻 `test_unpack_ex`／`test_pep646_syntax` 两文件）
  ③ `test_genexps` 生成器 `throw` 的 `StackUnderflow`。**round 52 进展**：用 `PYAWA_NO_TRACEBACK_ATTACH=1`
  诊断开关确认 `test_extcall` 的堆损坏由「异常↔回溯↔帧」环 × GC 回收触发（关掉就不崩）⇒ 下一轮直接修
  GC／回溯环；另：以 `*` 打头的链式目标已可**解析**（剩"绑定次序"）；`DICT_UPDATE` 的
  `'int' object is not a mapping` 报文已对齐。**round 53 进展**：回溯改为**链式**（旧 tb 接新 tb 的
  `tb_next`）后 `test_extcall` 从"必崩"降为"偶发 10/20"（关回溯 0/20）——GC 环仍要修；另修
  **带星解包值的等长窥孔**（`*a, b, c = x, y, *z` 静默不赋值）与**推导式里 `*`／`**` 的 SyntaxError**。
  **round 54 已清 ②**：`Call` AST 增 `positional_order`（源码交错保序）✅、内建类型 `__iter__` 属性面 ✅、
  `**生成器` 的 SyntaxError ✅ ⇒ `test_unpack_ex` 仅剩"运行器上下文"差异（doctest 期望
  `test.test_unpack_ex.f()`，CLI 按脚本跑给 `__main__.f()`；非语义差）。
  **round 55 已清 ②**：`*args`／`**kw` 函数注解＋PEP 646 星号注解 ✅ ⇒ `test_pep646_syntax` 跑通、套件 24 ⇒ 25。
  **round 56 已清 ②**：生成器**重入守卫**（`ValueError: generator already executing`）＋ `in range(...)`
  的**算术成员判定** ⇒ `test_genexps` 从**崩溃**变正常跑完、套件 25 ⇒ 26。
  **round 57 已清 ①**：`**` 通用映射路径的 **UAF**（`mapping_entries` 成功路径多放一次键引用
  ⇒ 死指针进字典 ⇒ 堆损坏；`test_extcall` 的「GC 堆损坏」根因就是它）⇒ 最小复现 3000 轮必崩 ⇒ 正常跑完。
  **round 58 已清 ①＋②**：`iterable_items` 改 `Result`（迭代协议异常原样传播、区分「不是可迭代」
  与「协议抛错」）＋ `*` 实参同一条路 ✓ ⇒ `iteration_error_shapes.rs` 8 形态对拍全绿。
  **round 59 进展**：真弱引用底座落地（回调／失效／哈希缓存／`_weakref` 三函数）✅；并用逐测试计时＋
  refcount 探针定位到 `test_weakref` 超时的**真正根因＝函数调用实参引用泄漏**（`f(k)` 后 `getrefcount+1`）。
  **round 60 进展**：`test_genexps` 的 4 条语法错报文＋`gi_*` 属性面 ✅ ⇒ 差异 6 ⇒ 2（剩元组复用、
  内建方法 `__doc__`）。**「形参所有权·最小面」再试**：修好后 `getrefcount`／`WeakKeyDictionary`
  当场正确，但 `unittest.main()` 冒烟仍 UAF（`builtin_function_or_method` 在 `shouldIncludeMethod`
  释放后又被类实参见释放）⇒ 已回退，现场进台账。
  **round 61 已清（P3-42 的一支）**：异常状态**每帧保存／恢复**（`execute` 的 `ExceptionStateGuard`）
  ⇒ `import enum`／`import unittest` 之后 `sys.exc_info()` 不再残留 `KeyError` ✓；套件 26／83／287／1 ⇒
  26／84／286／1（一类从运行期错迁到未实现，下一轮核实是哪一份）。
  **round 62 进展**：星号目标语法错报文一组＋`LIST_EXTEND`／解包／重复关键字报文 ✓（`test_unpack_ex` 导入名
  上下文 12 ⇒ 5；`test_extcall` 剩实参个数措辞）；`type.__call__` 的两条 `__init__` 转交路径修成**转移语义**，
  但 `unittest.main()` 冒烟还有 UAF ⇒ 形参所有权仍按「形参槽自有／借用标记」重修。
  **round 63 已清 ②**：`test_extcall`／`test_unpack_ex` **按导入名跑通** ✅；同时新增官方派发口径的
  `tools/libtests_named_runner.py` 并在 `libtests_survey.py` 里做上下文复核 ⇒ 官方套件
  **26／84／286／1 ⇒ 30／84／282／1**（`test_datetime`／`test_extcall`／`test_getopt`／`test_unpack_ex`）。
  **round 64 已清**：`test_keyword`／`test_generator_stop`／`test_dynamicclassattribute` 三份（各只差一条）⇒
  官方套件 **30／84／282／1 ⇒ 33／84／0｜279｜1**。
  **round 65 完成**：PEP 678 `__notes__`（`add_note`＋`__set_name__` 自动 note）＋ `Generic` 判据对齐
  ⇒ `test_subclassinit` 2 errors 清零（余 1F＝菱形 `__init_subclass__` 次序；`super` 的类对象 MRO
  试后回退，`import typing` 会红 ⇒ 下一轮先对齐 typing 再做）；套件 33／84／279／1 形态不变。
  **round 66 完成**：`abs(大整数)`／`float.real`·`imag`／`float(obj)` 走 `__float__`／集合去重按完整相等协议
  ⇒ `test_numeric_tower` 1F3E ⇒ 1F2E；官方套件 **33／84／279／1 ⇒ 33／80／283／1**（未实现 −4 ✓）。
  **round 67 已清 ①**：`float.as_integer_ratio` 精确化＋大整数 `bit_length`／`operator.index`／
  `math.gcd`·`lcm`·`isqrt`＋二元运算双方 `NotImplemented` ⇒ `TypeError` ⇒ `test_numeric_tower` **OK**
  （跑通 **33 ⇒ 34**）；另修 `test_pow` 的 50000 位模幂慢路（`u128` 小模数快路，3~13 s ⇒ 0.0 s）。
  **round 68 已清 ①**：零参 `super()` 对 `__init_subclass__` 用「类自己的 MRO」（只放开这一格，
  `typing` 等导入全绿 ✓）⇒ `test_subclassinit` **OK**（跑通 **34 ⇒ 35**）。
  **round 69 完成**：`compile()` 的 flags/optimize 收 `__index__`＋原生函数 `__module__` ⇒ `test_call`、
  `test_itertools` **转 OK**（跑通 **35 ⇒ 37**）；但 `test_collections` 的千字段 `namedtuple` 用例 >90 s
  ⇒ 套件**超时 1 ⇒ 2** ✗（见下一轮第一顺位）；套件 **37／82／276／2**。
  **round 70 完成**：字典／集合哈希索引＋超长字面量＋槽／名字收集去 O(n²) ⇒ `namedtuple` 千字段
  6.9 s⇒0.75 s；`test_extcall` 的 `str.__hash__` 也修回 ✓；套件 37／82／276／2（跑通不变）。
  **下一轮（round 71）**：① `test_collections` 剩余的失败／慢点（`dict(可迭代)`／`STORE_SUBSCR` 的
  `__eq__` 协议等 ⇒ 目标消掉超时）② `test_metaclass` 无参 `DocTestSuite()`（`sys._getframemodulename` 返 None）
  ③ `test_genexps` 剩 2 条 ④ `test_builtin` 编译器 panic／`test_calendar` 的 UAF ⑤ 形参所有权
  （`test_weakref` 超时扳机）／`P3-35`／`P3-29`／`P3-37`。：`compile(..., "single")`
  交互式显示（内建 `__pyawa_displayhook__`）＋ `print`／`displayhook` 每次读**当前** `sys.stdout`
  （模块属性走属性通道，修掉 SIGBUS）＋ `repr(Ellipsis)` ⇒ `test_unpack_ex` 剩 1F（星号目标的链式赋值
  `a, *b, c = *d, e = range(5)`）、`test_pep646_syntax` 剩 1F（`*args` 的函数注解）。
  **下一轮（round 52）优先级**：① **GC 双重释放**（`test_extcall`，正确性 bug，压一切）② 星号目标的
  链式赋值（可翻 `test_unpack_ex`）③ `*args` 注解（可翻 `test_pep646_syntax`）④ 生成器 `throw` 的
  `StackUnderflow`（`test_genexps`）。之后：`test_weakref`（形参所有权）／`KeyError` 上下文污染／
  `P3-35` 的 `\N{…}`／`P3-29`／`P3-37`。
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
