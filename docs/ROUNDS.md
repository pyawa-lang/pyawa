# 逐轮台账（**非规范**）—— 索引

> 台账已**分卷**存放在 `docs/rounds/`（每卷 ≤ 约 2000 行，按 `####` 边界切）。本文件只是索引。
> 本族是**过程记录**、**不是判据**：里程碑判据的唯一出处是 `docs/PLAN-milestones.md` §6，编号契约在各 `SPEC-*.md`，
> 工作队列与指示见 `PLAN-milestones.md` §9。各卷**只增不改**（原文照录，便于追溯）。
> **提交口径**：增量**必须随当轮的实质改动一起提交**——**禁止**纯台账提交（`agents-rules/commit-rule.md` §1）✓
> **新记录追加到当期卷**（下表最后一卷）；一卷满了再开新卷，并在本表加一行 ✓。

| 卷 | 行数 | 内容 |
|---|---|---|
| `docs/rounds/01-plan-9x-logs.md` | 1762 | `PLAN` §9.1 留档 ＋ §9.4 的逐轮日志（提纯时搬出） |
| `docs/rounds/02-block-model-01.md` | 2010 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/02-block-model-02.md` | 2027 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/02-block-model-03.md` | 2010 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/02-block-model-04.md` | 2016 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/02-block-model-05.md` | 2025 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/02-block-model-06.md` | 2038 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/02-block-model-07.md` | 1402 | 「块结构模型」及其后续逐轮记录 |
| `docs/rounds/03-m3-progress-01.md` | 2011 | 「M3 完成度」及其后续逐轮记录 |
| `docs/rounds/03-m3-progress-02.md` | 3226 | 「M3 完成度」及其后续逐轮记录 |
| `docs/rounds/03-m3-progress-03.md` | 1976 | 「M3 完成度」及其后续逐轮记录（第 579–720 轮收口归档 ＋ task-17 两轮 ＋ task-18 第 1 轮 ＋ task-14 两轮 ＋ task-19 三轮 ＋ task-20 三轮 ＋ task-21 三轮 ＋ 新墙第 1 轮 ＋ task-22 三轮 ＋ task-23 ＋ task-24 轮 1／轮 2-1／轮 2-①／步②前置） |
| `docs/rounds/03-m3-progress-04.md` | 1016 | 「M3 完成度」路线 2 枢纽战役（task-24 外壳/收口各轮 ＋ task-25 第 1 轮；判据① 208→296；会话交接收口归档） |
| `docs/rounds/03-m3-progress-05.md` | 2199 | 「M3 完成度」会话 05（task-26 形参 cell ⇒ importlib.resources 一族 ＋ task-27 行内 suite EOF panic ＋ task-28 C ABI 边界＋CX-11 守卫 ＋ task-29 Lib 语料第一批 30 条 ＋ round 2：task-4 `\|=` 就地族 ⇒ 34 模块 ＋ task-6 嵌套 boolop ＋ task-5B 语料第二批 22 条 ＋ task-7 解析缺口侦察 ＋ round 3：task-8 P3-31 解析器缺口批 10 格 ⇒ +17 ＋ task-9 `_signal`/`posix` 侦察 ＋ round 4：task-10 posix.getcwd/readlink ⇒ sysconfig 族 +3 ＋ round 5：task-11 小格批（dir/_operator/genexp panic/__classdict__＋三处通用编译器 bug）⇒ +11 ＋ round 6：task-12 `_socket` 最小面 ⇒ import socket 通（+2，P3-29 第一轮）＋ round 7+8：task-13 `_signal`／task-15 `_posixsubprocess`+`select`／task-14 `__file__`（+5）＋ round 9：task-16 循环相对导入回退＋失败模块清理（+3）＋ round 10：task-17 CodeObject.replace／task-18 array（拆闸持平）＋task-19 enum 换真货侦察 ＋ round 11：task-20 pa_create errno（P0 仪器）／task-21 enum B1–B3（真本 53/53）＋ round 12：task-22 enum B5+B6 换真本（CX-8 清零）／task-23 类型级描述符＋sys 面 ＋ round 13：task-24 http.server UAF（chain 引用计数）／task-25 dict.update+chain+imp（+30，multiprocessing/unittest 整包）＋ round 14 冲线轮：task-26 sys.version/tuple/链式赋值/star __all__（+33，asyncio 整包）——**判据① 达标 451/628＝71.8%** ＋ 判据② 冲刺：task-27 语料第三批（235→407，Lib 覆盖 150 模块）／task-28 覆盖率仪器＋闸门＋探针归一——**判据② 裁定达成** ＋ M3 收尾：task-29 CM-4 合约 23 节＋阶段边界（tag m3）——**M3 达成** ＋ **M4-A 批 round 4**：TS-46 可空标注 `T?` ＋ TS-47 运行期地基（`BC-65`／`CHECK_ASSIGNMENT`／`TypeAssignmentError`）＋ socket 侦察（P3-29 待拍板）＋ **会话 07：M5 开局**（P3-28 体量三档／T-CP 矩阵 10 绿 3 红＋两 provider 验收／B 批 `BC-62`／`BC-63`+`TS-49`／类型级描述符＋`P3-17`／socketpair 首片＋`PA_ABI_VERSION` 1.1／`_codecs_jp`；判据① 459/628＝73.1%）＋ **会话 07 round 11**（`CP-31` 实例能力表 ⇒ `T-CP-3`／`T-CP-7` 收口／编码委派＋`_codecs_cn`／`select.select`＋`os.dup`/`dup2`；矩阵 12 绿 1 红；判据① 461/628＝73.4%）＋ **会话 07 round 12**（`T-CP-10` 收口 ⇒ 矩阵 **13/13**・`CP-21`/`CP-32` VM 侧路径解析＋NaiveFs 反证・`CP-30` 注册版本校验＋vtable version 对齐・`_codecs_tw`/`_codecs_kr`；判据① 466/628＝74.2%）＋ **会话 07 round 13：M5 收口**（判据③ 阻塞阈值经用户拍板＋状态同步；`dev` `--no-ff` 收录 `main` ＋ tag `m5`）＋ **会话 09 round 14：M6 开局**（`P3-32`／`P3-33` 落地：`sys.exit`＋`SystemExit` 退出码／`PYTHONPATH` 进 `sys.path`／官方套件**首次跑通 5/397**（可编译 311/397）；`Lib/` 同步 544/628；判据① 466/628 未变；新量出 else+with 全局读 `None` 与 `test_decorators` 内存损坏两格）＋ **会话 10 round 15：M6 续**（修 `else` 尾位 `with` 误发作用域收尾 ⇒ 135 个官方测试签名从 `'NoneType'…get'` 推进到 `strftime` 缺名；`setattr` 少 incref 崩溃＋`classmethod(A().foo)` 多层方法崩溃一并修；`test_decorators` 不再崩；跑通数仍 5/397）＋ **会话 10 round 16：M6 续**（`time` 面解锁：`clock` 表尾三槽 `localtime`/`strftime`/`mktime`＋`asctime`/`ctime`＋`sys._jit`，**`import test.support` 通过**；`classmethod`/`staticmethod` 包装体面；编译器 `try: return <表达式>`/`finally` 顺序＋`return`-in-finally 栈溢出＋`BaseException.with_traceback`；ABI 1.3；跑通 7/397（1.8%）；缺口头名换成 `test.support.os_helper` ×83 ⇒ 下一格 P3-35 surrogate escape）＋ **会话 10 round 17：M6 续**（**P3-35 语法缺口批 A**：行内套件体/for-star/链式元组与注解非名字目标/表达式语句；**`sys.exc_info()`/`sys.exception()`** 真实现＋`PUSH_EXC_INFO` 重复修＋帧属性，unittest 错误恢复打通；**`gc` 模块**诚实面（P3-37 第一片，gc ×10 归零）；4 个 `Lib/test` 骨架同步（`Lib/` 548、`--check` 552）；**可编译 347/397＝87.4%**、语法错 86→50、判据① **468/628＝74.5%**）＋ **会话 10 round 18：M6 续**（**P3-34 complex 全链路**：1j 虚部字面量/常量折叠/pyac/真 complex 类型/构造器/算术/`hash()`/abs·int·float 面；**`Lib/unittest/mock.py` 同步**（`Lib/` 549、`--check` 553）；**try/with「余部重放」指数爆炸修**（连续 ≥16 块 StackUnderflow／挂死 → 全绿）；`deque` 真值/`list.pop(index)`/`getattr` 三参；判据① **469/628＝74.7%**、可编译 338/397、超时 132→22）＋ **会话 10 round 19：M6 续**（**P3-36 `.pyac BadCodeSection`** 修：Ellipsis 的 pyac 标签与 Bool 撞车 ⇒ 独占标签 20，10→0；**match 全形模式**：序列/映射/子模式或/`MATCH_CLASS` 位置形参，36 档 0 diff；`sys.monitoring` 最小面；**`unicodedata` 模块**（复用 unicode_tables）⇒ **`import bdb`/`pdb`/`doctest` 全通**；`sync_lib` 第二来源根机制；判据① **470/628＝74.8%**）＋ **会话 10 round 20：M6 续**（**doctest/pdb/bdb/_pyrepl 同步链**（14 文件，doctest ×20→0）；**批 B/C 语法**（60→40，非 surrogate 仅剩 10；`~x`/star 表达式/PEP 646 星号下标/多切片/注解表达式重构等）；**`builtins.compile()`**＋**`sys._getframe` f_back**（跑通 7→9）；**`hashlib`** 最小面（Rust md5/sha1/sha256，×4→0）；判据① **477/628＝76.0%**）＋ **会话 10 round 21：M6 续**（**非 surrogate 语法 10→0**（class `**`/星基类、f-string PEP 701、推导式下标目标、值模式 as、非 BMP 标识符等；语法错 40→30 全 surrogate、可编译 **92.4%**）；**core 小格批**（异常子类 MRO、`bytes.justify`、`set.union` 迁 core、函数 `__dict__`、dict 不可哈希键）＋ 裸元组语句 `x,` ⇒ **官方 `test_decorators` 16/16 OK**；MS-14 报告 407/407、新差异 0；**M6 收口待用户拍板（surrogate epic 硬 gate）**）＋ **会话 10 round 22：M6 续**（用户拍板**启动 P3-38 surrogate epic** ⇒ PLAN 立项（S1…S5＋§9.4 开工指示）＋ **S1 WTF-8 基础模块**（`wtf8.rs`，23 测试全绿、零行为改动）＋ **S2/S3 落地审计**（28KB：`new_str` 387／`text_of` 170／名字面 682／S2a…S3e 切片与风险、§7 六问待拍板））＋ **会话 10 round 23：M6 续**（P3-38 **S2a-1**：`StrObject` 存储升 WTF-8 表示 `StrRepr{Utf8,Wtf8}`、`new_wtf8`／`Instance::new_str_wtf8`／`wtf8_of`／`as_utf8`；`value()` 守卫、`text_of` 只对合法 UTF-8 给值 ⇒ 零可观测行为改动；新 `tests/str_wtf8.rs` 8 passed）＋ **会话 10 round 24：P3-38 续**（**S2a-2 批 1**：repr/相等/len 面 WTF-8 支持 —— `str_code_point_len`/`str_equals`/`str_is_empty`、`quote_wtf8`（孤立代理⇒`\udXXX`）＋`str_repr`/容器嵌套、`==`/len/真值逐点迁；合法 UTF-8 输出逐字节不变；`str_wtf8` 13 passed）＋ **会话 10 round 25：P3-38 续**（**S2a-2 批 2**：码点操作面 —— 迭代/下标/切片/拼接/重复/`in`/`len`/`str()`/`format` 直通（WTF-8 字节层）＋ **`chr`/`ord` 支持孤立代理**；e2e `chr(0xD800)` 全链与 python3 逐行一致；`str_wtf8` 18、`surrogate_chr_ord` 4 passed）＋ **会话 10 round 26：P3-38 续 ＋ 闸门提速**（**S2b**：lexer/AST/emitter/pyac 走 WTF-8 ⇒ `'\ud800'` 字面量可编译，**语法错 30→1、可编译 396/397＝99.7%**；release 档两处测试门控；**`slowcheck --release` 提速 ≈4.2×**（89min→~21min）＋ release 全量 survey 2m24s（跑通 10/397）；判据① 待下轮复核）＋ **会话 10 round 27：P3-38 续**（**S3a** surrogateescape/pass 四套 codecs 统一；**S3b** 输出边界写 WTF-8＋`sys.getfilesystemencodeerrors` 真 bug；推导式星号目标 ⇒ 语法错 1→0、**可编译 397/397＝100%**；**S3e-1** 同步 `os_helper`/`libregrtest/utils`/`multibytecodec_support`（`import test.support.os_helper` ✓）；跑通 15/397）＋ **会话 10 round 28：P3-38 续**（编译器块尾条件落点预存 bug 修（`UnboundLocalError: lines` 归零）；**S3b-2** fs 路径 surrogateescape（open/stat/listdir/readlink/rename/symlink/access）；**S3e-2** 小格批（`path_importer_cache`/`faulthandler`/`math.isclose`/`operator`/`_imp`）；跑通 17/397、MS-14 407/407 新差异 0）＋ **会话 10 round 29：P3-38 续**（含代理 str 排序/比较码点序（`value()` panic 归零）；**`RERAISE` 栈效应真 bug**（`__traceback__` ×11→0、`test_raise` 转真跑）；`getcwdb` 字节/`getcwd` PEP 383（顺修启动 panic）；`sys.stdout.buffer`/`getsizeof`/`thread_info` 最小诚实面；运行期错 288、超时 7、release 闸门 869s 全绿）＋ **会话 10 round 30：P3-38 收口轮**（**S2c** `.value()` panic 清零＋码点层格式化；**S3b-4** argv/`__file__`/readlink 字节口径；**Lib/test 差集同步 168 个**（`--check` 739、`lib_compile` 绿、`mapping_tests` 可导入、892 未入库逐类记因）；可编译 397/397、跑通 18/397、判据① 477/628＝76.0%、MS-14 407/407 新差异 0；release 闸门 1172s 全绿）＋ **会话 10 round 31：M6 收口**（`MS-14` 407/407、已知差异 0、新差异 0；`MS-19` 补充随 `P3-38` 收口（语法错 0／可编译 100%）；判据① 477/628＝76.0%；`Lib/` 同步 739 文件；PLAN §6 新增「M6 的要求」＋修订区 M6 阶段边界；README M0…M6 均成立；**`main` `--no-ff` 收录＋tag `m6`**）） |
| `docs/rounds/04-post-m6-01.md` | 6523 | 「M6 后 P3 批次」会话 11（round 32：`P3-41` 第 1 轮 —— `test_frame` SIGABRT 修（`repr` 递归守卫加深度计＋RAII，硬要求）＋ `for`／`while` 末尾 `if` 回边丢失的 `StackUnderflow` 崩溃族修（列表断言失败不再中止整个测试文件）＋ 6 个"hang"逐条裁决（只有 `test_tempfile` 是真 hang：`map()` 急求值消费无限 `_RandomNameSequence`；其余 5 个是慢：dict O(n²)／`subTest` 不释放／`startswith` 忽略 start·end）及其最小复现；跑通 18→18、`test_frame` 134⇒状态 5 ＋ 二轮 `P3-39`：`_io.StringIO` 真实现（码点位置／`writelines`／`detach`／两档关闭态文案／文本流 seek 规则／`readlines` EOF＋sizehint 真 bug；对拍探针 0 diff、`test_memoryio` 跑通 20⇒43、分诊里 `cannot create 'StringIO' instances` 归零）＋ 三轮 `P3-40`：`os.mkdir`（mode 真生效、`FileExistsError` 分类）／`os.pipe`（`fs` vtable 表尾追加槽、`PA_ABI_MINOR` 3→4、真 fd 走 `CP-31` 表）／`codecs.unregister`（更正分诊标签：不是 `os`）；对拍探针 0 diff，分诊里 `mkdir`(8)／`pipe`(6)／`unregister`(2) 三签名归零 ＋ 四轮 `P3-42`：`int.from_bytes` 缺省 byteorder／`time.struct_time` 真类型（localtime/gmtime 返回它）／`compile()` 收 codeop 与 `__future__` flags／`Exception.__setstate__`／`_opcode.is_valid` 五组签名归零，跑通 18⇒20（`test_baseexception`／`test_strftime` 转 ok）；`KeyError` 12 行留下（根因：特殊方法派发缺口＋unittest 异常链污染，已量出）＋ 五轮 `P3-35`（1/6）：`DELETE_SUBSCR` 协议补齐（`__delitem__` 派发／`bytearray` 下标与切片删除／不可删文案两档照参照）⇒ 未实现 100⇒87、分诊里"下标删除"签名 7⇒0；其余 `\N{…}`／`except*`／`import *`／async 推导式／PEP 695 如实未接（证据进台账）＋ 六轮 `P3-29` socket I/O 第二片：AF_INET 的 bind/listen/accept/connect/getsockname/getpeername/shutdown/setsockopt 全链真接线（三层：net provider→Instance→`_socket`；`CP-39` v1 线格式；IPv6/UNIX ⇒ `OSError(EAFNOSUPPORT)`；超时只做值面）＋ 连带修 `super()` 的属性描述符协议；TCP 回环对拍 0 diff；官方套件 20⇒21（`test_xml_dom_xmlbuilder` 转 ok）、socket I/O 签名 18⇒0）＋ 七轮 `P3-37` 剩余：`pyexpat` 形状面（模块级常量／`ErrorString` 44 条码表／`errors`·`model` 两个子模块／`ExpatError`／`XMLParserType`＋`ParserCreate`）——`Parse`／`ParseFile` 如实 `NotImplementedError`（整块 expat C 扩展未移植），形状面对拍 0 diff、`No module named 'pyexpat'` 签名 4⇒0（跑通 21⇒21）＋ `P3-41` 慢因（round 39）：`test_frame` 的 26.9 s 单测＝15 万层嵌套 mapping 被**单代 GC 固定阈值**拖成 O(n²)（每 2000 次分配做一次 O(存活) 全堆扫描）⇒ `alloc` 改**自适应水位**（回收后至少等 `max(阈值, 存活数)` 次分配再收）＋ `gc_collections()` 诊断计数；`test_frame` release 32.46⇒4.10 s／debug 196⇒41.22 s，规模扫描 O(n²)⇒线性；官方套件超时 9⇒3、跑通 21⇒21）＋ `test_userstring`（round 40）：① `str.startswith/endswith/count` 的 `start`/`end` 真生效 ＋ `find/count` 收 `None` 边界 ＋ `split/rsplit` 收 `None`／`maxsplit` ＋ `bytes` 同口径（对拍 0 diff；`test_find_periodic_pattern` 904 个假失败清除）② **`weakref.ref` 相等/哈希按目标**（`WeakSet` 才去重）——`_py_abc` 的 `_abc_cache` 先前一次 `isinstance` 长一条 ⇒ 成员查询 O(n²)：`isinstance` 5.4 ms⇒12.3 µs 恒定、`test_count` >900 s⇒6.9 s；`test_userstring` 941 failures／9 errors／>900 s ⇒ **38 failures／4 errors／42.3 s**（跑通 21⇒21）＋ `map`／`filter` 改**真惰性迭代器**（`ItStateKind::Map`／`Filter` ＋ 类型注册 ＋ 推进分支；`test_tempfile` 的唯一真 hang 解除：无限挂死 ⇒ 118 测试／0.75 s 跑完）＋ `dict.fromkeys` 收通用可迭代（顺修重复键不合并、多键 value 计数欠账）；官方套件超时 3⇒2、跑通 21⇒21 ＋ 七轮 `P3-41` 慢因（round 42）：`DictObject` 加**惰性哈希索引**（`values::key_hash` 与 `values_equal` 同源；`insert_hashed` 增量维护、改键作废）⇒ 下标读·写·删／`dict_set`·`dict_get`·`dict_set_int`·`dict == dict` 全走 O(1) 桶查；`dict_keyiterator` 加**键集版本守卫**（迭代中改键 ⇒ `RuntimeError`，文案两档照参照）。建 10 万项 **>180 s⇒0.275 s**、`d == d2` ⇒0.023 s、`test_dict` **超时⇒3.59 s**；官方套件**超时 2⇒1**（只剩 `test_weakref`）；`dict` 对拍 0 diff；另用 `PYAWA_GC_LOG=1/2` 量出 `test_weakref` 的下一处病因（GC 在 weakref／线程上下文 `freed≈0`、`live` 涨到 66.7 万）＋ `P3-41` round 43：用**对拍 `sys.getrefcount` 增量**量出四类"每次操作漏一份引用"的真 bug——名字加载（`lookup_in_mapping` 改借用）／`CALL` 漏还 `bound_self`／返回值经 `push` 多 retain／`__call__` 把借用当实参转交 ＋ 终结器语义（临时自持／至多终结一次／清引用前先摘 GC 链）；四类增量与 python3 一致归零、终结器只跑一次、`test_dict` 3.21 s、官方套件 21/90/285/1 **无回归**；**形参所有权改动试后撤回**（refcount 全零但暴露潜伏 UAF：test_dict/test_weakref SIGABRT/SEGV，`release_object ← instance_attribute_set`），留作下一轮独立审计；round 44：`==`／`!=` **特殊方法派发**（用户类 `__eq__`／`__ne__`、容器逐元素、`NotImplemented` 下一跳、`object` 默认实现跳过防递归）＋ `str` 一族参照口径（fillchar／`replace` count／`zfill` 符号／`expandtabs` 换行列／`rpartition('')`／搜索族 arity 报文／下标错型与大整数饱和／切片饱和算术／`%` 的 `*` 宽度与嵌套键与 `str.__mod__`／**生成的 title 映射表**）⇒ `test_userstring` **38F／4E ⇒ OK（skipped=2）**、官方套件跑通 **21 → 22**（未实现 90 → 89）；round 45：**嵌套 `with` 退出路径**（内层正常出口不再跳循环续点、`break`／`continue` 退出循环体内各层 `with`；`LoopFrame` 记 `with_levels`）⇒ `test_frame` **状态 5 解除**（跑到完，17F／19E／12 skip）；官方套件"未实现" 90 → **83**、跑通 22 不变（无回归）；round 46：**真值协议**（`__bool__` 必须恰返回 `bool`／`__len__` 整数且 `>=0`／类型对象恒真）修掉 `test_bool` 的 **Rust 栈溢出 abort**（改前退出 134，改后 6F／2E 正常跑完）；套件形态 22／83／291／1 不变；round 47：`bool`／`int` 面收口（`bool` 不可继承、`.real`／`.imag`／`.numerator`／`.denominator`／`.conjugate`、一元／位运算的 bool→int、`bool.from_bytes`、`bool(**kw)` TypeError、`BytesIO.getbuffer` ⇒ `test_pickle` 转绿、`eval` 结果先查 locals）⇒ `test_bool` **6F／2E ⇒ 2F**、官方套件跑通 **22 → 23**；round 48：`int.__new__`／`bool.__new__` 显式登记（`type.__new__` 桥接收口）＋ 核心侧 `DeprecationWarning` 通道（`~bool`）⇒ `test_bool` **全绿（OK）**、官方套件跑通 **23 → 24**；round 49：doctest 导入前置面（`sys.gettrace`／`stdin`／`displayhook`／`setprofile`、隐式异常 `__traceback__`、`backslashreplace`）⇒ 4 个 doctest 文件从**导入即死**推进到真正的 doctest 用例（`test_extcall`／`test_unpack_ex` 到 `DICT_MERGE` 缺口、`test_pep646_syntax` 到断言级、`test_genexps` 到生成器 `throw` 的 `StackUnderflow`）；跑通 24 不变、"未实现" 83→86（同一批文件从"运行期错"推进到"未接线"）；round 50：`DICT_MERGE`／`CALL_FUNCTION_EX` 协议面（`**映射` 走 `keys()`＋`__getitem__`、同名键报文带被调对象限定名、`*` 收自定义 `__iter__`、关键字名 `keywords must be strings` 且收 `str` 子类）⇒ 套件 24／83／289／1 不变；doctest 文件剩 `single` 模式交互 repr、`test_extcall` 的 **GC 双重释放**、生成器 `throw` 的 `StackUnderflow`；round 51：`compile(..., "single")` 交互式显示（内建 `__pyawa_displayhook__`）＋ `print`／`displayhook` 每次读**当前** `sys.stdout`（模块属性走属性通道，修 SIGBUS）＋ `repr(Ellipsis)` ⇒ `test_unpack_ex`／`test_pep646_syntax` 各剩 **1F**（星号目标链式赋值／`*args` 注解），套件 24／83／289／1 不变；round 52：以 `*` 打头的链式目标可解析（`a, *b, c = *d, e = range(5)`）＋ `DICT_UPDATE` 的 `'int' object is not a mapping` 报文 ＋ 用 `PYAWA_NO_TRACEBACK_ATTACH` 诊断开关把 `test_extcall` 堆损坏锁定到「异常↔回溯↔帧」环 × GC 回收（默认仍挂回溯）；`test_unpack_ex`／`test_pep646_syntax` 各剩 1F（星号目标绑定次序／`*args` 注解）；round 53：带星解包值不再误走"等长窥孔"（`*a, b, c = x, y, *z` 静默不赋值 ✗ ⇒ 修 ✓）、回溯改**链式**（旧 tb 接到新 tb 的 `tb_next`；`test_extcall` 堆损坏必崩 ⇒ 偶发 10/20）、推导式里 `*`／`**` 报参照 SyntaxError（先前静默生成结果 ✗）；套件 24／83／289／1 不变；round 105（行为变更轮·小步）：**`type_mro_native` 漏 retain 真 bug 修**（`TypeObject::mro()` 借用指针直接进
`new_list` ⇒ `list_clear` 对每个类型多减一次 ⇒ `test_descr` 的 `对已释放对象 decref：类型 type` 根治）；
记录新发现：`test_set` 在 round 104 融合存取旧值归还下出现 glibc 堆损坏（二分确认来自旧值释放的别名/重复
所有者），下一轮主线；套件空闲基线 70/78/245/4；`docs/rounds/04-post-m6-01.md`｜6523｜
round 104（行为变更轮）：**用户类类型 GC 收尾**——`STORE_FAST_STORE_FAST` 旧值归还正式落地＋`list.__setitem__`
借用值双放修（retain）＋`PYAWA_ORPHAN_RECLAIM` 默认开 ⇒ **`test_dict` 整档转绿**、`test_list` 由 1F/1E⇒0F/1E
（free_after ✓，剩 CM-8 域 error）；套件 **70/78/245/4**（ok 68⇒70）；新问题 `test_set` 因回收扫描变 timeout
（下一轮节流）；scout 定位 `type_mro_native` 漏 retain（test_descr 的 type 双放，1 行修复留 round 105）；
`docs/rounds/04-post-m6-01.md`｜6479｜
round 103（行为变更轮·小步）：**`STORE_FAST_LOAD_FAST` 旧值泄漏修复**（`a = b = …`／`while` 融合形态每次漏旧值）
＋`PYAWA_REF_TRACE_DEAD_BT`（第二次释放 Rust 回溯）；`STORE_FAST_STORE_FAST` 试改让 reclaim=1 下 test_dict
free_after 转 OK 但 test_list 新悬垂（死释放来自 tuple_clear，元素计数少一份 alias）⇒ 撤回留 round 104；
**计数口径纠偏**：test_userstring 单跑 100 s（预算 120）为临界，round 102 的 69 是边界抖动 ⇒ 稳定 68/78/247/4；
roundcheck 默认改串行、survey `--jobs 2`；`docs/rounds/04-post-m6-01.md`｜6423｜
round 102（行为变更轮）：**真 bug 修：`CALL_FUNCTION_EX` 成功后漏还 `callable`**（`f(*args)`／`A(*args)` 每次漏被调对象
一份）；`PYAWA_REF_TRACE_CLASS=A` 对账：类对象 `rc 2⇒1` 闭合；套件跑通 **68⇒69**、超时 **4⇒3**（`test_userstring`
转 ok）；scout 定位下一环：`STORE_FAST_STORE_FAST` 不归还被覆盖槽旧值（试改让 reclaim=1 下 test_dict free_after
转 OK，但 reclaim=0 时 test_list 新悬垂 ⇒ 已撤回，留 round 103）；`docs/rounds/04-post-m6-01.md`｜6382｜
round 101（提速基建，用户批准）：**验收并行化＋分档节奏**——`tools/roundcheck.sh`（十项闸门＋survey 并行）、
`tools/fastcheck.sh`（32 s 提交前快档）、`slowcheck.sh` ③ 由 ⑤ 覆盖、⑨⑩ 并行、heap 扰动诊断并行；
整轮验收 **23m15s ⇒ 12m45s**（-45%），纯诊断轮 32 s；判定口径不变（首跑十项全绿＋survey 68／78／247／4）；
round 100（续期后第 20 轮／目标轮次收尾）：诊断六——注册类型的 `-1` 行带计数（类对象对账）；“类对象少一次
decref”**只出现在 `assertRaises` 构造**（模块级/函数内类对账都齐：rc=2=注册表+命名空间／rc=1=只剩注册表）；
阶段总账：套件跑通 26⇒68、未实现 246⇒78、test_dict 166 格⇒1 格；形参所有权 round 98 翻默认；
套件 68／78／247／4；
round 99（续期后第 19 轮）：诊断五——**`PYAWA_REF_TRACE_CLASS`（按类自己的名字跟踪类型对象）**＋`REF_TRACKED`
登记条件补上类跟踪；`assertRaises(StopIteration, next, it)` 构造下类对象时间线 `+1(+build_class)→+1(LOAD_FAST)→-1`
⇒ 结束 `rc=2`（少一次 decref，与 `test_descr` 的 type 双放同源）；套件 68／78／247／4 持平；
round 98（续期后第 18 轮）：**形参所有权（帧拥有形参）翻成默认**（`PYAWA_FRAME_OWN=0` 可回退）——round 43 起的
"每次 Python 调用漏 1 份实参"旧账收尾；翻默认前实测 `PYAWA_FRAME_OWN=1` 全量套件 ok 68 持平、十项闸门全绿；
＋孤儿用户类回收原型（`PYAWA_ORPHAN_RECLAIM=1`，默认关；不释放类型 Box，简单夹具 done=True ✓，
`assertRaises` 构造下类对象多一份引用未收）；套件 68／78／247／4（唯一差异 test_descr unp→runtime）；
round 97（续期后第 17 轮）：**修 `dict` 相等比较的「清空操作数」UAF**（比较期间两侧键值各 incref、字典被清空时
退回快照查找）；`PYAWA_FRAME_OWN=1` 全量套件首测：**68 持平、无 ok 回退**（两档 tsv 唯一差异 `test_descr`
unp→runtime，撞 `对已释放对象 decref：类型 type`，`list_clear` 归还提前归零的 type）；套件 68／79／246／4；
round 96（续期后第 16 轮）：**修 `list` 定序比较的活体双游标（真 UAF）**——元素的 `__lt__` 清空操作数时
不再用到悬垂元素（每轮重读当前元素、比较前各 incref、比完归还）；`PYAWA_FRAME_OWN=1` 实验路径下 `test_list`
不再崩（1F/1E 同基线），默认路径套件 68／79／246／4 持平；`test_weakref` 超时另有 O(n²) 源（入 NEXT）；
round 95（续期后第 15 轮）：**形参所有权实验开关**（`PYAWA_FRAME_OWN=1`，默认关）——`frame_clear` 释放形参槽、
`created`/`name_object` 补偿改为条件执行；默认路径逐字节同 round 94（套件 68／79／246／4 持平）；实验路径下
UAF 断言可逐个点名"借用实参"站点（当前第一现场 `ABCMeta.__new__` 类创建的命名空间键；随息机浮动需确定性夹具）；
round 94（续期后第 14 轮）：**诊断四**——`PYAWA_ZOMBIE_TRACE=1` 下对**已释放对象的 incref** 当场点名
（附 Rust 回溯；判据用释放登记 `freed_sites`，引导期单例不误报）；形参所有权第三处现场：
`build_class_from_parts_with_keywords` 搬运类命名空间时 `incref(key)` 的键**早已被释放**；三处现场已点名，
形参所有权再次撤回（只留诊断）；套件 68／79／246／4 持平；
round 93（续期后第 13 轮）：**诊断三**——`PYAWA_REF_TRACE_BT`（首次释放的 Rust 回溯）；形参所有权两处现场点名：
`instance_attribute_delete_inner` 的 `name_object`（帧移交后又 `release`）与比较协议里的 `evil` 实例
（`call_object_method→call_callable→release_object` 二次释放）；两处修完仍有余站 ⇒ 形参所有权再次撤回
（只留诊断）；套件 68／79／246／4 持平；
round 92（续期后第 12 轮）：**诊断二**——`-1 live=false` 现场安全化（`REF_TRACKED` 地址集合，release 不解引用
死指针）；形参所有权第二次现场：`delattr` 删除类命名空间键时拿到**悬垂键**（`delattr_native →
instance_attribute_delete_inner → release(key)`，键释放于 `EnumType.__new__@569`）⇒ `STORE_SUBSCR` 的
`_EnumDict.__setitem__` 协议分支 ＋ 形参所有权叠加的**键计数缺口**；套件 68／79／246／4 持平；
round 91（续期后第 11 轮）：**诊断增强**（alloc 事件／`live=` 标记／通配符）＋**形参所有权第五次尝试的现场收敛**：
重落帧拥有形参后 lambda 实参漏计消失，但 `test_dict` 的 str 悬垂经时间线核对**账是平的**（alloc→+1/-1→rc=0
正常释放）⇒ 问题不是"双放"而是**str 释放后仍被裸借用**（`text_of` 一族 `&'static str`）⇒ 段错误；
形参所有权再次撤回（只留诊断）；套件 68／79／246／4 持平；
round 90（续期后第 10 轮）：**`PYAWA_REF_TRACE` 引用事件跟踪**（环境门控诊断）——A 族"环外引用"归因到
**已知的"每次 Python 调用漏 1 份实参"**（`lambda d: iter(d.keys())` 的实参 +1 无配对）；形参所有权第四次
落地试后撤回（修好环外引用后 `PYAWA_DANGLING=1` 立刻暴露类创建期 **str 双放**——释放于 `EnumType.__new__@569`，
与 round 83 同一现场，配合时间线可逐拍锁定第二个持有者）；套件 68／79／246／4 持平；
round 89（续期后第 9 轮）：**A 族"孤儿用户类回收"实现后撤回**（不释放类型 Box 的保守变体：rc==1×无裸引用×
动态可达性×实例全不可达 ⇒ 先跑实例终结器再清字典；顶层 `global done` 写法能收、**函数内闭包写法**
（`done=[False]`）失败：实例 rc=2 而候选持有者只剩 `dict_keyiterator` ⇒ **一条环外引用未归因**）；
套件 68／79／246／4 持平；
round 88（续期后第 8 轮）：**动态类类型 GC（A 族组合拳）实现后撤回**（事实入账：类型候选/标记边/注册表
不算根/专用 clear/free 全套；但四变体连跑时环外引用未归因 ⇒ 实例成根 ⇒ 类型永不可收；DANGLING 下
`test_dict` `free(): invalid pointer`）＋ **`dict(<任意映射>)` 走 `keys()` 协议**（复用 `mapping_keys`；
`os.environ.copy()` 不再落进"可迭代对"分支）；套件 68／79／246／4 持平；
round 87（续期后第 7 轮）：**`test_rlcompleter` 转绿（ok 67⇒68）**：`builtins` 命名空间去污染（惰性
视图/迭代器类型不再塞 `builtins`）／`dir` 名字面补全（object 24 dunder＋`__slots__` 名＋`dir(type)` 动态名）／
`object` 的 `__lt__` 一族/`__sizeof__`/`__getstate__`/`__reduce__`/`__dir__`／**用户 `__getattribute__` 派发**
（覆写优先、守卫防重入）／类型/内建方法文本签名扩展（`type.mro`、`staticmethod/classmethod/property`）；
survey **68／79／246／4**；
round 86（续期后第 6 轮）：**`inspect.signature` 文本签名通路**——`compile(..., PyCF_ONLY_AST)` 接
最小 AST（签名语法：Module/FunctionDef/arguments/arg）、内建 `__text_signature__` 旁表（object.__new__/__init__/
__init_subclass__）、类型对象 `object.__text_signature__='()'` 特例 ⇒ `signature(object)`／无自定义 init 的类给
`()`；survey 67／79／247／4（未实现 −2、运行期 +2：两个文件前进一个接线面）；
round 85（续期后第 5 轮）：**`iter(callable, sentinel)`**（新内部类型 `callable_iterator`）＋**序列协议
惰性 `iterator`**（修无限序列死循环）＋ **itertools 迭代器 `__next__`/`__iter__` 属性面** ＋ **tokenizer 接受 `$`**
（`tokenize` 通路打开）＋ `compile(_feature_version=…)` ⇒ `test_iter` 从 unimplemented 到跑完（9F/14E）；
survey **67／81／245／4**（4 个文件 runtime→unimplemented：跑到下一个未接线面才停）；`advance` 的
TypeError 口径撤回（会把 test_itertools 的 10M 空链推到超时）；
round 84（续期后第 4 轮）：**回溯逐帧展开**（`call_callable` 退出处给调用者补 tb ⇒ `tb_next` 真链）
⇒ `test_frame` 24 格 ⇒ **21 格**（clear 一族 3 格转绿）；顺带保/恢复 `attach_traceback` 的
`pending_exception` 槽（TS-47 用例红复绿）；形参所有权第四轮/`__text_signature__`/`LOAD_ATTR` 方法位
三档试后撤回（事实入账）；survey **66／77／250／4**（`test_userstring` 在 11 并发下超 120 s，单跑 89 s、
7 路负载 140 s、基线同样 140 s）——如实记录；
round 83（续期后第 3 轮）：**`f_generator` 反向引用**（帧↔生成器双向 GC）＋ **`StopIteration.value`**
（args[0]/None）＋ `f_locals` 快照扩到 localsplus（cell/free）⇒ `test_frame` 25 格 ⇒ 24 格；
**形参所有权第三轮再次撤回**：`test_weakref` 类创建期 `delattr` 双放，僵尸追踪定位到 str 首次释放于
`build_class_from_parts`（EnumType.__new__@569）——类创建路径的借用 vs 接管缺口；套件 67／77／250／3 持平；
round 82（续期后第 2 轮）：**终结局序真 bug 修**——`release_object` 待处理栈里的延迟释放让 `__del__`
帧的 `self` 形参成为"临时引用"，旧代码误判复活 ⇒ `__del__` 无限重复（5s 128 万行）；改为终结后保持
`FINALIZING`＋队首延后判定、重放前查活表；并加 **Python 级 `__del__` at-most-once** 旁表（照参照
`tp_finalize`，宿主终结器槽保持 `OM-20` 语义）⇒ 用例/夹具全绿；套件 67／77／250／3 持平；
round 81（续期后第 1 轮，目标扩至 100 轮）：并行只读侦察（weakref/GC 根因＝实参所有权泄漏＋单代全表；
test_frame 29 格分组）＋**`frame.clear()`／`f_builtins`**（执行中/挂起帧拒绝、保留 code/globals/内建；
test_frame 29⇒25 格）＋**形参所有权第二次尝试撤回**（终结局序：`__del__` 帧 self 形参释放晚于 finalize 计数
判定 ⇒ 复活误判 ⇒ `__del__` 无限重复；事实入账，P0 留给下一轮）；套件 67／77／250／3 持平；
round 125（目标续期后第 20 轮／最后一轮）：**`test_dict` 2F/0E ⇒ 1F/0E**：**视图 `.mapping` 活视图**
（`Instance.mappingproxy_live` ＋ `sync_mappingproxy`；`.mapping` 登记来源、`dict.__eq__`／
`equality_public` 的 dict 分支入口先同步 ⇒ `d["foo"]="bar"` 后 `m == d` ✓）；套件 67／77／250／3 持平；
round 124（目标续期后第 19 轮）：**A 族真 bug：`dispatch_raise` 未归还 `Err(Raised)` 那份引用** ⇒
**每个被处理的异常多留一份引用** ⇒ 异常→`__traceback__`→帧→局部变量整条链泄漏；修后 `try/except`
后 `__del__` 正常触发、帧与局部按时回收（`PYAWA_DANGLING=1` 干净）；套件 67／77／250／3 持平；
round 123（目标续期后第 18 轮）：**`test_dict` 3F/0E ⇒ 2F/0E**：视图键迭代改**原始 `dict_keyiterator`**
（修 `__iter__ = keys` 子类的自我递归）／`dict(子类)` 走 `keys()` 协议（顺序保真：`OrderedDict` 的
`move_to_end`／`CustomReversedDict` 逆序）⇒ `test_dict_copy_order` 转绿；套件 67／77／250／3 持平；
round 122（目标续期后第 17 轮）：**`test_dict` 4F/1E ⇒ 3F/0E**：更新序列错误的 `__notes__`（PEP 678；
`with_update_note` 挂 `Cannot convert dictionary update sequence element #i to a sequence`）／**`dict.fromkeys`
真类方法**（`classmethod` 包装 ＋ 按绑定类正常实例化；`dictlike.fromkeys` 交 dictlike 实例、`mydict.__new__`
覆写生效；逐键 `subscript_write`）⇒ `test_update_type_error`／`test_fromkeys` 转绿；套件 67／77／250／3 持平；
round 121（目标续期后第 16 轮）：**`test_dict` 6F/1E ⇒ 4F/1E**：**逐项缓存键哈希**（`DictObject.hashes`
＝与 entries 平行的 `Vec<u64>`，哨兵 `u64::MAX`；`ensure_index` 优先缓存 ⇒ 参照「每键只算一次 `__hash__`」）／
字面量建表哈希复用（`map_lookup_with_protocol` 交回 `(位置, 哈希)`）／`dict_setitem` 复用查询哈希 ⇒
`test_setdefault_atomic`／`test_setitem_atomic_at_resize` 转绿；套件 67／77／250／3 持平；
round 120（目标续期后第 15 轮）：**`test_dict` 13F/4E ⇒ 6F/1E**：可失败查找面（`key_hash_checked`／
`dict_lookup_key_checked`／`dict_lookup` 改 Result；pop/setdefault 走协议、pop 的不可哈希与 KeyError args）／
**整数哈希与参照同值**（`hash_i64` 修真 bug）／`dict.__eq__` 真实现／视图比较按 `==` 协议传播异常／
`d.update` 末尾变异守卫／`kwargs` 非 str 名报 TypeError／`gc.is_tracked` 近似实现；套件 67／77／250／3 持平；
round 119（目标续期后第 14 轮）：**`test_dict` 16F/4E ⇒ 13F/4E**：`items()`／`values()` 交引用字典的
真迭代器（版本守卫覆盖三种 dict 迭代器）／`update` 源 dict 变异检测（retain 快照＋逐对比对）／视图迭代器
pickling（列表快照口径）／推进按 `entry(index)` 借用直取（修 O(n²)，`test_userstring` 恢复）⇒ 变异检测
4 格全绿；套件 67／77／250／3 持平；round 118（目标续期后第 13 轮）：**`test_dict` 18F/9E ⇒ 16F/4E**：dict 类级方法面（10 个适配器，
`dict.items(d)` 未绑定调用 ✓）／`__ior__` 属性面／`clear`·`copy`·`popitem` 参数口径／**`text_of`
接受 `str` 子类**（`getattr(obj, StrSub(...))`／`d[StrSub('k')]` ✓）；套件 67／77／250／3 持平；
round 117（目标续期后第 12 轮）：**编译器两层闭包 ＋ 隔着类体的 `nonlocal`**（真 bug：类作用域是
`nonlocal` 的通道；深层闭包需求逐层上浮）＋ **`unicode_escape`／`raw_unicode_escape` 真编解码**
（`escape_decode`/`escape_encode`；`test_builtin` 模块级 `str(bytes,'raw-unicode-escape')` 当场炸）＋
**`_socket.gethostname`**（`platform.system()` 可用；取值由 runtime 读 `/proc` 注入 `PYAWA_HOSTNAME`，
保 `CX-4`）⇒ **`test_dict` 47F/17E ⇒ 18F/9E**（`test_str_nonstr` 16 格全绿）、`test_builtin` ok 恢复；
套件 67／77／251／2 ⇒ **67／77／250／3**（`test_socket` 真网络阻塞，已登记）；round 116（目标续期后第 11 轮）：`test_dict` 47F/17E ⇒ **34F/14E**：`dict | dict`（`__or__`/`__ror__`）／
视图集合式大小比较／`reversed(view)`／`__missing__` 派发／dict 方法参数字面口径／**用户键的 `==` 查找语义**
（先结构快路再完整 `==` 协议；`dict_lookup` 的 str 快路只认存的也是 str；`in dict` 走哈希索引）⇒
`test_str_nonstr` 剩 16 格待查；套件 67／77／251／2 持平（收敛轮）；round 115（目标续期后第 10 轮）：**`dict` 视图真类型**（`dict_keys`／`dict_items`／`dict_values`：
len／iter／contains／repr／`.mapping`；keys·items 的集合运算与 `==`（含反射）；集合运算先 retain 实参）
⇒ **`test_dict` 错误 119 ⇒ 17**（失败 47 持平；快照式实现如实登记）；套件 67／77／251／2 持平（收敛轮）；
round 114（目标续期后第 9 轮）：**`test_pprint` 转绿**（ok 66⇒67）：`defaultdict` 真方法面
（`len`/`items`/`keys`/`values`/`get`/`__iter__`/`__repr__` + 初始映射）／`deque` 的 `maxlen`·`__len__`·
`__iter__`／`mappingproxy` 的来源类型（`copy`/`repr` 还原 `OrderedDict`）／dataclass 换行（绑定方法
`__wrapped__` 委托）／排序"不小于即相等"／类型级 `__repr__` 直调槽位（+ 无槽退回默认）／CLI 默认递归
上限 160（大栈线程内）；套件 66／77／252／2 ⇒ **67／77／251／2**；round 113（目标续期后第 8 轮）：**`types.SimpleNamespace` 真类型 ＋ `sys.implementation` 真实例**
（`Lib/types.py` 的 `SimpleNamespace = type(sys.implementation)` ✓）＋ **实例 `__dict__` 读实例字典**
＋ **子类 `__repr__` 与声明 repr 槽的基类同对象**（`dict2.__repr__ is dict.__repr__` ✓ ⇒ pprint 子类
dict 换行）＋ `sys` 夹具读通道随之更新；`test_pprint` 15 格 ⇒ 9 格（6F/3E）；套件 66／77／252／2
持平（收敛轮）；round 112（目标续期后第 7 轮）：**`mappingproxy` 真类型**（`type.__dict__` 交只读视图的实例；修
`pprint._dispatch` 条目被覆盖 → **每个 dict 的 pformat 无限递归**）＋**大栈执行线程**（512 MiB，
默认递归上限仍 64）＋**扩展切片赋值先物化右值再算边界**（`list_index_modifing_operand`）⇒
**`test_builtin` 转绿**、`test_pprint` 26 格 ⇒ 15 格、`test_list` 3 格 ⇒ 2 格；套件 65／77／253／2 ⇒
**66／77／252／2**（跑通 65⇒66）；round 111（目标续期后第 6 轮）：**`test_list` 4F/3E ⇒ 2F/1E**（7 格 ⇒ 3 格）：内建迭代器属性面＋pickle
（`__reduce__`／`__setstate__`；`object.__reduce_ex__` 先认类型 `__reduce__`；内建函数 `__reduce__`
返回名字串）／真 `list_reverseiterator`／子类覆写 `__iter__` 走迭代协议（`LyingTuple`）／
`list.__eq__` 改表后按当前长度判／`-c` 命令行／内置布局子类装终结器（`__del__`）；**`test_heapq` 转绿**；
套件 64／77／253／3 ⇒ **65／77／253／2**（跑通 64⇒65、超时 3⇒2）；round 110（目标续期后第 5 轮）：**`test_list` 17F/6E ⇒ 4F/3E**（23 格 ⇒ 7 格，无翻页）：序列 `in`／`index`／
`count`／`remove` 走富比较协议（比较期间持引用＋每轮重读长度）／`list.__init__` 真接线／sort 的
`Some(Equal)` 与 `ValueError: list modified during sort`／迭代器耗尽哨兵／`tuple*1` 恒等／
`list.__imul__` 真就地／`list.__repr__` 改表安全／`list()` 关键字拒绝／`staticmethod` 特殊方法；
套件 64／77／253／3 持平；round 109（目标续期后第 4 轮）：**ABI 追加 `lutime`／`utime_ns`**（纳秒级 utime、`AT_SYMLINK_NOFOLLOW`）＋
**`copy.copy` 的 `__reduce_ex__` state**＋**`bytes.partition`**＋**`types.MethodType` 构造**＋
**`stat(follow_symlinks=False)`／`_have_functions`**＋**`with` 体内 `return` 的异常表切分**＋**尾斜杠 open
语义**＋**绑定报文用 `qualname`** ⇒ `test_filecmp`、`test_shutil` 转绿；套件 62／76／256／3 ⇒
**64／77／253／3**（跑通 62⇒64）；round 108（目标续期后第 3 轮）：**`OSError` 的三元组属性面＋`str()` 格式**（`errno`／`strerror`／
`filename`／`filename2`；`display` 反转义回原名、`b'…'` 交 bytes）⇒ `test_unicode_file_functions`
转绿（另 `test_shutil` 3F/87E⇒20F/21E、`test_tempfile` 5F/48E⇒5F/35E ✓）；套件 61／76／258／2 ⇒
**62／76／256／3**；round 107（目标续期后第 2 轮）：**`unicodedata.normalize` 真实现**（生成组合表 1393 行；四形态＋Hangul
＋`is_normalized`）＋**`os.utime` 全链路**（fs 槽＋provider＋posix）＋**WTF-8 串安全面**
（`_path_normpath`／`str.split`／`rfind`／`rindex`／`translate`／`_sre` 哨兵映射）⇒ `test_unicode_file` 转绿；
套件 60／76／258／3 ⇒ **61／76／258／2**（跑通 60⇒61、超时 3⇒2）；
round 106（目标续期后第 1 轮）：**下标 `__index__` 协议**（写／删下标）＋**`list` 的 dunder／`copy` 面**＋
**序列重复收子类**＋**`cell_clear` 借用跨越修**＋**推导式元组字面量融合修** ⇒ `test_listcomps` 转绿、
`test_list` 从状态 5 到跑完 114 测试；套件 59／81／255／2 ⇒ **60／76／258／3**；round 105（**无翻页**，如实记；目标末轮）：**`__iter__` 返回值校验**（`IterNoNext` 的 TypeError 面）＋
**序列 `*=` 就地重复**（`list`／`bytearray`，含上限守卫防容量溢出 panic）⇒ `test_list` 从状态 5
中止到跑完 114 测试；套件 59／85／250／3 ⇒ **59／81／255／2**（未实现 85⇒81、超时 3⇒2）；
round 104：**`int(bytes)` 十进制形态**＋**序列拼接收子类**（`list`／`tuple`／`bytes` 的 `+`）＋
**`list` 族的 `__reduce_ex__` 可重建形状**（copy／deepcopy 空列表修）⇒ `test_xml_dom_minicompat` 转绿；
套件 58／85／251／3 ⇒ **59／85／250／3**；round 103：**类型级 `__repr__` 的每类型包装器＋身份哈希**（`pprint` 的 `_dispatch[type(obj).__repr__]`
真 bug：任意 list 被当 deque）＋**包优先于同名模块**＋**模块 `__dir__`／`__doc__`**＋**`import *` 的
`__all__` 子模块** ⇒ `test_pkg`、`test_difflib` 转绿；套件 56／85／254／2 ⇒ **58／85／251／3**
（`test_collections` 并行调查下超时、单跑 63 s ✓）；round 102（**无翻页**，如实记）：**Python importlib 的 fresh 子模块导入面真接线**（finder 父包
`__path__` 定位重接——round 99 的撤回真实原因是 `__package__` 口径、已修；`loader.exec_module` 的
实参／命名空间／带点名字三处缺陷；`locate_source` 带点名字走父包 `__path__`）＋**子类切片**
（`bytes`/`bytearray`/`str` 子类）⇒ `test_xml_etree` 族从 0 测试到跑完 228 测试；套件
56／83／255／3 ⇒ **56／85／254／2**（**超时 3⇒2** ✓）；conformance 全量复核仍全绿；round 101：**冻结模块测试面**（`_imp._override_frozen_modules_for_tests` 真接线：开关 1 时
`__hello__`／`__phello__`／`__phello__.spam` 的 `__spec__.loader` 标成
`importlib.machinery.FrozenImporter`）⇒ `test_frozen` 转绿＋`from pkg import 子模块` 的装载回退；
套件 55／83／256／3 ⇒ **56／83／255／3**；round 100：**比较协议三处真语义**（`str`/`bytes` 子类序比较、`bytearray` 等值、`!=` 的子类优先级＋
`object.__ne__` 默认语义／身份兜底）⇒ `test_compare` 转绿；套件 54／83／258／2 ⇒ **55／83／256／3**
（新增 `test_decimal` 超时：单跑 137 s ✗，如实记）；round 99：**`io.BufferedReader`／`BufferedWriter` 真实现**（`socket.makefile` 的底座；`test_urllib_response`
的 setUp）＋**`pow` 的浮点／负指数面**（`f64::powf`、0.0 负指数 `ZeroDivisionError`、三参含浮点
`TypeError`）⇒ `test_urllib_response`、`test_pow` 转绿；套件 52／83／260／2 ⇒ **54／83／258／2**；
round 98：**`import a.b.c as x` 的链式 `IMPORT_FROM` 真 bug**（编译器；三段以上名字先前只取最后一段）
＋**`sys.modules[name]=None` 阻断导入**＋**模块 `__spec__`／`__package__`**（fresh 导入面；`__package__`
口径修正后 conformance 新差异 0）＋**旧式 4 元组／变长 charmap codec** ⇒ `test_charmapcodec`、
`test_sundry`、`test_stat` 转绿；套件 49／86／260／2 ⇒ **52／83／260／2**；round 97（**无翻页**，如实记）：`io.open` 的 **`opener` 关键字**（`tempfile.NamedTemporaryFile`／
`TemporaryFile` 由 `IsADirectoryError` 转通）、内建 **`format`**（含整数格式化）、**`os.chmod`**
（＋ runtime `fs_posix` 的 `chmod` 槽）、**`os.stat(fd)`**、**`function.__call__`**（`unittest.mock`
的 `_callable` 判据）、`FileIO.writelines`、`os.symlink(target_is_directory=…)` ⇒ `test_tempfile`
2F/52E⇒5F/48E；套件 49／87／260／1 ⇒ 49／86／260／2（`test_collections` 8 路并行下越过 120 s，
单跑 66 s ✓）；round 96：**元组 freelist 复用**（`Instance::tuple_pool`：释放的 `tuple` 留在池里、`new_tuple` 复用并
重新登记）＋**`map` 真泄漏修**（`advance_map` 没还 `advance_iterator` 交的新引用）＋`g.__next__.__doc__`
⇒ **`test_genexps` 转绿**（id 复用 doctest：显式／手工／`map` 三路 id 差全 0）；套件跑通 48⇒49；
round 95：**`del obj.attr` 按类型 `__delattr__` 分派**（真缺口；`unittest.mock` 的 `MagicMock.__delattr__`
不生效 ⇒ `_mock_set_magics` 报 `'MagicMock' object has no attribute '__lt__'`）⇒ **`test__colorize` 转绿**；
顺带：`sys.__stdout__` 一族、标准流 `fileno()`／`isatty()`、`os.DirEntry` 模块属性导出、`stat_result`
必有名（`st_nlink`/`st_uid`/`st_gid`＋时间戳补 0）、`_io` write 收缓冲协议、`posix._path_normpath` 收
`bytes`；套件跑通 47⇒48；round 94：**`bytes(str, enc)` 只接 UTF-8 的真缺口**（`fnmatch` 的 bytes 模式全断）＋**`os.lstat` 真实现**
（断链／`os.path.lexists`；含"先跟随叶子再 readlink"的二级真 bug）⇒ **`test_glob`（24E⇒OK）与
`test_fnmatch` 转绿**，套件跑通 45⇒47；round 93：**编译器真 bug**（`for: try: with:` 的正常出口跳回循环续点、跳过 `try` 之后语句）＋
`os.scandir`/`os.DirEntry`/`os.ScandirIterator`（AttributeObject＋类型字典挂原生方法，含
`follow_symlinks`／`is_symlink`／`is_junction`）＋ `path_argument` 支持 `os.PathLike`（PEP 519）
⇒ **`test_mimetypes` 转绿**（ok 44⇒45）、`test_glob` 24E⇒10E、`test_tempfile` 1F/80E⇒2F/58E、
`os.walk`/`shutil.rmtree` 探针由断转通；round 92：补 `gc.get_debug`/`set_debug`/`get_count`（按实例调试位；单代 ⇒ `(分配数,0,0)`）＋
`os.putenv`/`unsetenv`（核心 `host_env` 封装；`os._Environ` 直接调模块级名）＋`os.isatty`（标准流走
`IsTerminal`）＋`set.intersection`/`difference`/`issubset`/`isdisjoint`；面向 `test_gc`/`test_mimetypes`/
`test__colorize` 的首因，本轮**无翻页**（剩余：`os.scandir`、`MagicMock` 的 dunder 面）——如实记；
round 91：**`test_bufio` 转绿**——`os.fstat`（新增 `Instance::fs_fstat`，`stat_result` 组装抽一处）＋
`os.chdir`（实例 cwd，`getcwd`/`getcwdb` 改读实例 cwd）＋`int.__index__`/`__int__`（`_pyio` 直接取
`size.__index__`）＋`os.write` 收 `bytearray`/`memoryview`＋**`bytearray.extend`/`+=` 缓冲协议快路**
（`_pyio.BufferedWriter.write` 的 100 KB 从 0.27 s 降到毫秒级 ⇒ `test_bufio` 4 用例 >200 s ⇒ 5.7 s OK）；
round 90：**撤回轮**——「帧拥有形参」统一所有权实验：实参泄漏 +100⇒0、conformance 双模式绿，但
`__del__` 帧的释放**晚于** `python_level_finalize` 返回 ⇒ 延迟 decref 被误判成复活 ⇒ `__del__`
无限重复（6.5M 次后超时）⇒ 撤回（`frame.rs`／`call.rs` 复原），留下新事实（A 族新顺序：先钉帧延迟
释放 → 再统一形参所有权 → 最后删补偿释放）＋ `PYAWA_DEL_TRACE` 诊断；行为与 round 89 相同；
round 89：修 `itertools.groupby` 的**键双持有真双放**（`groups` 与 `current` 各持一份 ⇒ 换组/收尾时
提前打掉 `groups` 里的键引用，结果元组释放时再放一次）⇒ **悬垂全量 memory 1 ⇒ 0（硬门①首次达成）**；
如实记：本层 `groupby` 仍是急求值 ⇒ `test_issue30347_2` 仍以状态 5 结束（不再是内存安全事件）；套件
**43／85／268／1 全部持平**（无回退）；round 88：**内存安全专项启动**——新增 [`docs/MEMORY_SAFETY.md`](docs/MEMORY_SAFETY.md)（口径／三条硬门／
A-B-C 三族清单／按轮次路线）＋ `tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 全量基线）；修
**`filter` 迭代器「让出的元素先释放再返回」真 UAF**（`advance_filter`）⇒ 悬垂全量 memory **5 ⇒ 1**
（`test_platform`／`test_random`／`test_urlparse`／`test_webbrowser` 清掉；只剩 `test_itertools` 的
groupby 可重入）；补 `SystemExit.code`；`getrandbits(k)` 大 `k` O(n²)⇒O(n)（顺带修自己引入的遮位顺序
bug）⇒ `test_random` 从超时回到 10.7 s 跑完；套件 **43／83／270／1 ⇒ 43／85／268／1**（跑通持平、
超时回 1 ✓）；round 87：**`float.conjugate()`** ＋ **`divmod` 浮点面与协议回退** ⇒ `test_abstract_numbers` 转绿；
**`LOAD_GLOBAL` 的 `__missing__` 回退**（全局映射是 dict 子类时未命中先调它）⇒ `test_dynamic` 转绿；
连带修：`SetObject.index` 改存**元素地址**（删除不作废 ⇒ `discard`／`in` 交替不再 O(n²)，
`set_clear` 一并清索引）＋ `LOAD_GLOBAL` 的 `__missing__` 只对非纯 dict 生效（纯 dict 快速路径，
避免每次内建名加载走属性通道）⇒ `test_collections` 从 110 s 回到 65 s、`test_userstring` 回到 80 s（OK）；
套件 **41／83／272／1 ⇒ 43／83／270／1**（跑通 41⇒43 ✓）；round 86（恢复轮次后第 1 轮）：**`set` 删除后索引未作废**真 bug（`SetObject::remove`／`remove_at`；
`discard`／`pop`／`remove` 后 `in` 会漏判）⇒ `test_baseexception` 转绿；**`in` 的迭代回退**（`__contains__`
缺失时按 `__iter__`／旧式 `__getitem__`／内建可迭代逐个比，`__contains__ = None` 阻断回退）＋ **`deque`
迭代/相等/构造与 extend 走迭代器协议** ⇒ `test_contains` 转绿；套件 **39／84／273／1 ⇒ 41／84／272／1**
（跑通 39⇒41 ✓）；round 85（**最后一轮**）：**`test_metaclass` 转绿（ok 38 ⇒ 39）** —— 自定义元类的 **`mro` 覆写**接线
（参照 `type.__new__` 用 `metaclass.mro(cls)` 的结果当 MRO：`WeirdClass.value` 沿 `Base` 取到 ✓，与参照逐字一致）
＋ `C.mro()` 优先元类自定义实现；套件 **39／84／273／1**（唯一超时 `test_weakref`）；round 84：**`super().__prepare__` 兜底**（`type.__prepare__` 在属性通道特殊名分支实现、不在类型字典里 ⇒
super 沿 MRO 查字典拿不到 ⇒ 元类方法里的 `super().__prepare__` 报 AttributeError；现在走到内建 `type`
条目时交回默认实现）⇒ `test_metaclass` 剩 3 条示例（仍未翻）；**实参所有权试点**三种加严变体（全部
函数／只裸函数／再排除默认值与 `*args`／`**kwargs`）探针 refcount 都能归零但 conformance 均触发堆损坏
（`import_os_surface`／`cell_teardown`／`lib_json` 等）⇒ 全回退；套件持平；round 83（**撤回轮**，如实记）：**普通调用的位置实参泄漏已量化**（`f(x)`×100 ⇒ refcount +100，参照 0；
这就是 `test_weakref` 百万级活对象的根因）；两种全局所有权方案（帧持有形参／借用语义＋各还各的）
都在 conformance 触发 `tcache` 堆损坏 ⇒ 回退；生成器形参归属补丁探针有效（refcount 增量 0）但套件
`ok 38 ⇒ 37`（未定位文件）⇒ 也回退。计划：先加 debug 所有权断言，再分三批迁移调用点；round 82：**函数当元类**（`class C(metaclass=meta)` 的 `meta` 是普通可调用对象时照参照**不建类型**：
先 `meta.__prepare__`（关键字照传）再用 `meta(name, bases, ns, **kwds)` 的返回值当类 ⇒ `type(C) is dict` ✓）
＋ **`type.__prepare__()`**（默认元类给空 dict ✓）⇒ `test_metaclass` 的 doctest 差异从 6+ 条缩到 2 类
（`super().__prepare__` 的 super 属性通道／`Meta.mro` 覆写，文件仍未翻）；套件 **38／84／274／1** 持平；round 81：**ABI 错误边界归还未捕获异常**（`pa_exec_string` 一族取完消息后调 `discard_raised`：还
`Err(Raised)` 那份＋摘 `pending_exception` 槽同款；`SystemExit` 例外保留 ✓）——宿主每执行一次抛异常
脚本不再漏「异常＋回溯链＋整帧」；`dispatch_raise` 真修复的**双重释放**再进一步：本地用 conformance 的
`pyawa_side_runner` 稳定复现 SIGABRT，`ZOMBIE_TRACE` 查明被双放的是**异常实参一枚 `str`**
（首次释放于 `<module>@50`）⇒ 多放点在 `RAISE_VARARGS`／`exception_new` 实参引用之间；
套件 **38／84／274／1** 持平；round 80：`attribute_optional` 吞 `AttributeError` 时清 `pending_exception` 槽（同 round 77 口径）；
**`dispatch_raise` 的异常引用泄漏真 bug 已定位并复现**（`def raiser(): big=[3000 对象]; raise` ＋
`try/except` 后 3000 个对象全活；诊断：异常 rc=1、无任何引用者 ⇒ `Err(Raised)` 所有权没被消费；
`dispatch_raise` 用 `push()` 加一份又不还）——但修法（release 或接管式 `frame.push`）在 conformance
触发 `tcache` 堆损坏（`libtest_hashlib_missing` SIGABRT）⇒ 说明另有调用点多放一份，**已撤回**；
套件 **38／84／274／1** 持平；round 79：**实例化实参泄漏修掉**（`call.rs`：`__init__` 分支给实参表 `incref(created)` 后按契约归还；
守卫＝Python 函数形态 `__init__` ＋ 类无自有 `__del__`——否则终结器＋GC 重入下早放会双重释放）
＋ `POP_EXCEPT` 清 `pending_exception` 槽 ＋ `__call__` 探针改用 `attribute_lookup_swallowing`；
**`test_weakref` 超时真正大头钉死**：被 `except` 捕获的异常**回溯链留住整帧**（3000 对象实测不释放、
`traceback` 每次 raise ＋2；持有者未钉住）⇒ 超时仍未解；套件 **38／84／274／1** 持平；round 77：**修掉探针式属性查表的异常泄漏**（新增 `attribute_lookup_swallowing`：失败时归还 `Err(Raised)`
那份引用＋摘掉 `pending_exception` 同款；`state.rs` 三处、`iter.rs` 一处改用它）⇒ 被吞 `AttributeError`
泄漏从每次 +2 个对象降到 **0**；`test_weakref` 峰值 live **265 万 ⇒ 12.6 万**（但仍 >280 s ⇒ 超时未解）；
**实例化实参泄漏**（每次带 `__init__` 的实例化漏 1 格实例引用，7 万条 ⇒ 百万级 live）已钉死但朴素修法
（补 `release(created)`）实测双重释放 ⇒ 撤回，留下轮做形参槽所有权重构；套件 **38／84／274／1** 持平；round 76：**`__bases__` 保留显式冗余基类**（真 bug：`class C(B, object)` 先前给 `(B,)` ✗、参照 `(B, object)` ✓）
＋ **重复关键字实参 ⇒ `SyntaxError: keyword argument repeated`**（调用与类定义两处；先前静默收下后一个 ✗）
＋ 调查器默认发现兜底；套件 **38／84／274／1** 持平（如实记：`test_weakref` 超时真因已缩到——**被吞的
`AttributeError` 每组泄漏「异常＋回溯＋帧」**——行为面已定位、持有者尚未钉住；`pending_exception` 补丁
实测无效且拖慢 `test_collections`，已撤回）；round 74：内建 **`locals()`**（模块层 `is globals()`、函数层快照 dict，偏差如实记）＋ `sys.float_repr_style='short'` ＋
**类型对象 `__class__` 改回元类型**（真 bug：`class Meta(type)` 的 `C.__class__ is Meta` 先前 False ✗）＋调查器补
「默认 `loadTestsFromModule`」兜底 ⇒ `test_threading_local` **转绿**、套件 **37／82／277／1 ⇒ 38／84／274／1**
（跑通 **37 ⇒ 38** ✓，超时仍 1）；`test_metaclass` 的 `C.__class__ is M` 现已 True（该文件余其它差异）；round 73：**`isinstance` 的 ABC 快速通道**（只对元类恰是 `ABCMeta` 的类；键含
`_abc_invalidation_counter` ⇒ `register` 后自动失效；未命中仍走 Python `__instancecheck__`）⇒
`isinstance(x, Sequence)` **45 µs ⇒ 6.7 µs**、`UserString.find` 53.9 ⇒ 13.9 µs、`test_userstring`
101 s ⇒ **80 s**（`ok` 恢复 ✓）；套件 **36／82／277／2 ⇒ 37／82／277／1**（**超时只剩 `test_weakref`** ✓）；round 72：**多字节 `EXTENDED_ARG`**（真 bug：下标 ≥65536 截断成第 0 项 ⇒ 65536 形参 lambda 的末参变成 `_cls`）
＋ **`type_lookup_owner` 走 `dict_get` 哈希索引**（150k 字段类的 `getattr` 从 >200 s 降到 1.6 s）⇒
`test_collections` 整份 **超时 ⇒ 59.5 s 跑完**（timeout 变 runtime ✓）；但 **`test_userstring` 101 s 临界**
被判 timeout ✗ ⇒ 套件 **37／82／276／2 ⇒ 36／82／277／2**（ok −1，超时成分换成 userstring／weakref）；round 71：**千参函数编译链路去 O(n²)**（`analyze_cells` 后无条件重建槽缓存／`intern_name` 缓存／lambda 形参
名 `HashSet`）＋ `bind_arguments` 关键字形参名索引 ＋ `dict_position` 哈希＋用户 `__eq__`（`dict(可迭代)`／
`d[k]=v` 共用）⇒ 64k 形参 lambda 编译 **4.25 s ⇒ 0.20 s**、`dict(可迭代)` 2.7 s ⇒ 0.03 s；套件 **37／82／
276／2** 持平（`test_collections` 仍超时，剩余 `random.choice` 1.5M 次的状态成本与元组子类比较缺口）；round 70：**性能族**：`MAP_ADD`／`BUILD_MAP` 字典推导与显示走**哈希索引**（O(n²)⇒O(n)）＋ `set` 全族（`BUILD_SET`／`SET_ADD`／`SET_UPDATE`／`set_new`／`CONTAINS_OP`）加**惰性哈希索引** ＋ **超长元组／列表字面量（>255 项）接线**＋局部槽／名字收集去 O(n²)；`namedtuple` 8000 字段 6.9 s⇒0.75 s、`{name:i}` 2.57 s⇒0.03 s、`name in set` 2.52 s⇒0.04 s；期间抓出并修 **`key_hash` 与 `hash()` 不同源**（FNV vs SipHash ⇒ `test_numeric_tower` 一度回退，改 CPython 口径后恢复 ✓）与 **`str.__hash__` 缺失**（`class Name(str)` 的 `__hash__` ⇒ `test_extcall` 一度回退，已修回 ✓）；套件 **37／82／276／2**（跑通不变、超时仍 2 —— `test_collections` 其余差异未清）；round 69：`compile()` 的 `flags`／`optimize` 按参照走 `__index__`（`IntWithDict` 用例 ✓）＋ 原生可调用对象补 `__module__`（`doctest.DocTestSuite(模块)` 的 `_from_module` 不再 `ValueError` ✗）⇒ `test_call`、`test_itertools` **转绿**（跑通 **35 ⇒ 37**）；`test_collections`／`test_metaclass` 也恢复加载、跑到用例面（`test_collections` 的千字段 `namedtuple` 用例 >90 s ⇒ 套件**超时 1 ⇒ 2** ✗，已列下一轮第一顺位）；套件 **35／82／279／1 ⇒ 37／82／276／2**；round 68：零参 `super()` 对 **`__init_subclass__`** 也用「类自己的 MRO」（`super_lookup` 类对象判据从 `__new__` 扩到 `__init_subclass__`；**只放开这一格**——round 65 的「全面放开」会让 `import typing` 红 ✗）⇒ 菱形 `__init_subclass__` 链 `['middle']` ⇒ `['right','middle']` ⟹ `test_subclassinit` **OK**（跑通 **34 ⇒ 35**）；套件 **34／82／280／1 ⇒ 35／82／279／1**；round 67：`float.as_integer_ratio` **精确化**（任意精度 BigInt，修 i64 饱和 ✗ ⇒ `_pydecimal` 的哈希恢复）＋ `int.bit_length`／`operator.index` 收大整数（后者带 `__index__`）＋ `math.gcd`／`lcm`／`isqrt` 收大整数（isqrt 牛顿迭代）＋ 二元运算**双方 `NotImplemented` ⇒ `TypeError`**（`binary_override` 返回"有无覆写"标记）⇒ `test_numeric_tower` **OK**（跑通 **33 ⇒ 34**）；`operator.index` 放开后 `test_pow.test_big_exp` 的 50000 位随机指数暴露 `BigInt` 模幂慢路（3~13 s）⇒ 加 `u128` 小模数快路（0.0 s）后 `test_pow` 回运行期错、超时回 1；套件 **33／80／283／1 ⇒ 34／82／280／1**；round 66：数值塔三件（`abs(任意精度整数)`／`float.real`·`imag`／`float(obj)` 走 `__float__`）＋ 集合去重按**完整相等协议**（用户 `__eq__`，`has_custom_eq`；试过塞进 `values_equal` ⇒ 栈溢出 ⇒ 回退到去重点 ✓）⇒ `test_numeric_tower` 1F3E ⇒ 1F2E（`test_hash_normalization` 翻 ✓）；官方套件 **33／84／279／1 ⇒ 33／80／283／1**（未实现 −4、运行期错 +4，类目迁移 ✓）；round 65：**PEP 678 异常 `__notes__`**（`BaseException.add_note`；非 `str` 报 `add_note() argument must be str, not int`）＋ **`__set_name__` 失败自动 note**（`Error calling __set_name__ on 'Descriptor' instance 'attr' in 'C'`）＋ `Generic` 的「裸 Generic」判据按参照对齐（`Generic in cls.__bases__` 且非 Protocol／TypedDict 才报）⇒ `test_subclassinit` 2 errors 清零（余 1F＝菱形 `__init_subclass__` 次序，`super` 口径试后撤回、下一轮随 `typing` 对齐再做）；套件 **33／84／279／1 形态不变**；round 64：**硬关键字不能当名字**（`keyword.kwlist` 全覆盖；软关键字仍可用 ✓，语句开头的 `not`／`lambda`／
`None` 仍按表达式语句 ✓）＋ **PEP 479 包装带 `__cause__`／`__context__`／`__suppress_context__`**
（走 `ExceptionObject` 载荷的新入口 ✓）＋ **抽象类实例化拦截**（`Can't instantiate abstract class …`，
名字字典序 ✓）⇒ 跑通 `test_keyword`／`test_generator_stop`／`test_dynamicclassattribute`，官方套件
**30／84／282／1 ⇒ 33／84／0｜279｜1**（语法错仍 0 ✓）；round 63：交互式显示**递归进模块层复合体**（`try:` 块里的表达式也显示 ✓）＋ `id(**kw)` 报
`takes no keyword arguments`＋实参过多报文的「有几个报几个」与 `(and N keyword-only argument(s))` 尾巴＋
星号解包 >255 上限＋`(*x)` 的 `cannot use starred expression here`；**套件调查按官方导入名复核**
（新增 `tools/libtests_named_runner.py`；`libtests_survey.py` 对直接跑失败的文件按导入名再跑）⇒
官方套件 **26／84／286／1 ⇒ 30／84／282／1**（`test_datetime`／`test_extcall`／`test_getopt`／
`test_unpack_ex` 跑通 ✓）；round 62：星号目标的**参照语法错报文**一组（`multiple starred expressions in assignment`／`starred assignment target must be in a list or tuple`／`can't use starred expression here`／`cannot use starred expression here`）＋ `LIST_EXTEND` 的 `Value after * must be an iterable, not X` ＋ 解包的 `cannot unpack non-iterable X object` ＋ 重复关键字键的 `str(键)` 引号口径；新增 `star_syntax_shapes.rs`；`test_unpack_ex` 在导入名上下文 12 ⇒ 5，套件 26／84／286／1 形态不变；round 61：**异常状态改每帧保存／恢复**（`execute` 的 `ExceptionStateGuard` 在全部退出路径截回入口深度）⇒ `import enum`／`import unittest` 之后 `sys.exc_info()` 不再残留 `KeyError('keep')`／`KeyError(512)`（P3-42 的 KeyError 污染根因），新增 `exception_state_shapes.rs` 对拍；套件 26／83／287／1 ⇒ 26／84／286／1（一类从运行期错迁到未实现：不再被残留异常遮住，后面真正的未接线路径暴露 ✓）；round 60：`test_genexps` 剩余**语法错报文**收口（`return … for` ⇒ `invalid syntax`、`dict(a = i for …)` ⇒ `Maybe you meant '==' or ':='`、genexp 作赋值／增强赋值目标 ⇒ `cannot assign to generator expression`／`'generator expression' is an illegal expression for augmented assignment`）＋ 生成器 `gi_running`／`gi_frame`／`gi_code`／`gi_suspended`／`gi_yieldfrom` 属性面（`dir(g)` 同步补名）⇒ `test_genexps` 差异 6 ⇒ 2（剩元组复用优化与内建方法 `__doc__`）；「形参所有权·最小面」再试仍引出 `unittest.main()` 的 UAF（`builtin_function_or_method` 二次释放）⇒ 按规矩回退并写清现场；套件 26／83／287／1 形态不变；round 59：**真弱引用**（`WeakRefObject` 目标不持引用／回调触发／失效登记表 `Instance::weakrefs`／两套哈希缓存／`_weakref` 的 `getweakrefcount`·`getweakrefs`·`_remove_dead_weakref`；`release_one` 与 GC 两条失效路径）＋ 用逐测试计时与 refcount 探针定位到 `test_weakref` 超时的**真正根因＝函数调用实参引用泄漏**（`f(k)` 后 `getrefcount+1` ⇒ `WeakKeyDictionary` 的键全被钉住）；「帧拥有形参」能修好但它在本轮引出新的 UAF（`unittest.main()` 冒烟）⇒ 按规矩**只提交安全的弱引用底座**，形参所有权留下一轮；套件 26／83／287／1 形态不变；round 58：**迭代协议里的异常不再被吞**（`Instance::iterable_items` 改 `Result<Option<…>>`；`list(g)`／`sum(g)`／`f(*g)` 里生成器抛的错原样传播，并区分「不是可迭代」与「`__iter__` 自己抛错」；顺带统一 `list.extend`／`set.update`／`str`-族首参的引用纪律）⇒ 新增 `iteration_error_shapes.rs` 8 形态对拍；套件 26／83／287／1 形态不变；round 57：**修掉 `**` 通用映射路径的 use-after-free**（`mapping_entries` 成功路径多放一次键引用 ⇒ 死指针进字典 ⇒ 堆损坏；`test_extcall` 的「GC 堆损坏」其实是它）⇒ 最小复现 3000 轮必崩 ⇒ 正常跑完、`test_extcall` 稳定不再崩；套件 26／83／287／1 形态不变；round 56：生成器**重入守卫**（`ValueError: generator already executing`）＋ `in range(...)` 的**算术成员判定**（`1.0 in range(3)` ⇒ True、`'a' in range(3)` ⇒ False）⇒ `test_genexps` 从**崩溃**变为正常跑完（剩 4 条语法错报文＋元组复用优化），官方套件 **25 ⇒ 26 跑通**（`test_type_annotations` 转跑通）；round 55：`*args`／`**kw` 的函数注解**不再丢弃**（`__annotations__` 次序：位置 → `args` → 仅关键字 → `kw` → `return`）＋ PEP 646 星号注解按"恰好解包一项"求值（含零项／多项的参照 `ValueError`、带默认值 ⇒ `SyntaxError: invalid syntax`）⇒ **`test_pep646_syntax` 跑通**、官方套件 **24 ⇒ 25 跑通**；round 54：`Call` AST 增 `positional_order`（调用实参**源码交错保序**，`print(*[1], *[2], 3)` 的 `3 1 2` ⇒ `1 2 3`，四种形状与参照逐字节一致）＋ 内建类型 `__iter__` 属性面（`dict`／`list`／… 显式 `.__iter__()` 可调）＋ `**生成器` 的 `SyntaxError: invalid syntax`；`test_unpack_ex` 剩最后 1F 是**运行器上下文**（doctest 期望 `test.test_unpack_ex.f()`、CLI 按脚本跑给 `__main__.f()`） |
