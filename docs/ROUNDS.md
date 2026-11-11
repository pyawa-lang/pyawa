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
| `docs/rounds/04-post-m6-01.md` | 11579 | 「M6 后 P3 批次」会话 11（round 32：`P3-41` 第 1 轮 —— `test_frame` SIGABRT 修（`repr` 递归守卫加深度计＋RAII，硬要求）＋ `for`／`while` 末尾 `if` 回边丢失的 `StackUnderflow` 崩溃族修（列表断言失败不再中止整个测试文件）＋ 6 个"hang"逐条裁决（只有 `test_tempfile` 是真 hang：`map()` 急求值消费无限 `_RandomNameSequence`；其余 5 个是慢：dict O(n²)／`subTest` 不释放／`startswith` 忽略 start·end）及其最小复现；跑通 18→18、`test_frame` 134⇒状态 5 ＋ 二轮 `P3-39`：`_io.StringIO` 真实现（码点位置／`writelines`／`detach`／两档关闭态文案／文本流 seek 规则／`readlines` EOF＋sizehint 真 bug；对拍探针 0 diff、`test_memoryio` 跑通 20⇒43、分诊里 `cannot create 'StringIO' instances` 归零）＋ 三轮 `P3-40`：`os.mkdir`（mode 真生效、`FileExistsError` 分类）／`os.pipe`（`fs` vtable 表尾追加槽、`PA_ABI_MINOR` 3→4、真 fd 走 `CP-31` 表）／`codecs.unregister`（更正分诊标签：不是 `os`）；对拍探针 0 diff，分诊里 `mkdir`(8)／`pipe`(6)／`unregister`(2) 三签名归零 ＋ 四轮 `P3-42`：`int.from_bytes` 缺省 byteorder／`time.struct_time` 真类型（localtime/gmtime 返回它）／`compile()` 收 codeop 与 `__future__` flags／`Exception.__setstate__`／`_opcode.is_valid` 五组签名归零，跑通 18⇒20（`test_baseexception`／`test_strftime` 转 ok）；`KeyError` 12 行留下（根因：特殊方法派发缺口＋unittest 异常链污染，已量出）＋ 五轮 `P3-35`（1/6）：`DELETE_SUBSCR` 协议补齐（`__delitem__` 派发／`bytearray` 下标与切片删除／不可删文案两档照参照）⇒ 未实现 100⇒87、分诊里"下标删除"签名 7⇒0；其余 `\N{…}`／`except*`／`import *`／async 推导式／PEP 695 如实未接（证据进台账）＋ 六轮 `P3-29` socket I/O 第二片：AF_INET 的 bind/listen/accept/connect/getsockname/getpeername/shutdown/setsockopt 全链真接线（三层：net provider→Instance→`_socket`；`CP-39` v1 线格式；IPv6/UNIX ⇒ `OSError(EAFNOSUPPORT)`；超时只做值面）＋ 连带修 `super()` 的属性描述符协议；TCP 回环对拍 0 diff；官方套件 20⇒21（`test_xml_dom_xmlbuilder` 转 ok）、socket I/O 签名 18⇒0）＋ 七轮 `P3-37` 剩余：`pyexpat` 形状面（模块级常量／`ErrorString` 44 条码表／`errors`·`model` 两个子模块／`ExpatError`／`XMLParserType`＋`ParserCreate`）——`Parse`／`ParseFile` 如实 `NotImplementedError`（整块 expat C 扩展未移植），形状面对拍 0 diff、`No module named 'pyexpat'` 签名 4⇒0（跑通 21⇒21）＋ `P3-41` 慢因（round 39）：`test_frame` 的 26.9 s 单测＝15 万层嵌套 mapping 被**单代 GC 固定阈值**拖成 O(n²)（每 2000 次分配做一次 O(存活) 全堆扫描）⇒ `alloc` 改**自适应水位**（回收后至少等 `max(阈值, 存活数)` 次分配再收）＋ `gc_collections()` 诊断计数；`test_frame` release 32.46⇒4.10 s／debug 196⇒41.22 s，规模扫描 O(n²)⇒线性；官方套件超时 9⇒3、跑通 21⇒21）＋ `test_userstring`（round 40）：① `str.startswith/endswith/count` 的 `start`/`end` 真生效 ＋ `find/count` 收 `None` 边界 ＋ `split/rsplit` 收 `None`／`maxsplit` ＋ `bytes` 同口径（对拍 0 diff；`test_find_periodic_pattern` 904 个假失败清除）② **`weakref.ref` 相等/哈希按目标**（`WeakSet` 才去重）——`_py_abc` 的 `_abc_cache` 先前一次 `isinstance` 长一条 ⇒ 成员查询 O(n²)：`isinstance` 5.4 ms⇒12.3 µs 恒定、`test_count` >900 s⇒6.9 s；`test_userstring` 941 failures／9 errors／>900 s ⇒ **38 failures／4 errors／42.3 s**（跑通 21⇒21）＋ `map`／`filter` 改**真惰性迭代器**（`ItStateKind::Map`／`Filter` ＋ 类型注册 ＋ 推进分支；`test_tempfile` 的唯一真 hang 解除：无限挂死 ⇒ 118 测试／0.75 s 跑完）＋ `dict.fromkeys` 收通用可迭代（顺修重复键不合并、多键 value 计数欠账）；官方套件超时 3⇒2、跑通 21⇒21 ＋ 七轮 `P3-41` 慢因（round 42）：`DictObject` 加**惰性哈希索引**（`values::key_hash` 与 `values_equal` 同源；`insert_hashed` 增量维护、改键作废）⇒ 下标读·写·删／`dict_set`·`dict_get`·`dict_set_int`·`dict == dict` 全走 O(1) 桶查；`dict_keyiterator` 加**键集版本守卫**（迭代中改键 ⇒ `RuntimeError`，文案两档照参照）。建 10 万项 **>180 s⇒0.275 s**、`d == d2` ⇒0.023 s、`test_dict` **超时⇒3.59 s**；官方套件**超时 2⇒1**（只剩 `test_weakref`）；`dict` 对拍 0 diff；另用 `PYAWA_GC_LOG=1/2` 量出 `test_weakref` 的下一处病因（GC 在 weakref／线程上下文 `freed≈0`、`live` 涨到 66.7 万）＋ `P3-41` round 43：用**对拍 `sys.getrefcount` 增量**量出四类"每次操作漏一份引用"的真 bug——名字加载（`lookup_in_mapping` 改借用）／`CALL` 漏还 `bound_self`／返回值经 `push` 多 retain／`__call__` 把借用当实参转交 ＋ 终结器语义（临时自持／至多终结一次／清引用前先摘 GC 链）；四类增量与 python3 一致归零、终结器只跑一次、`test_dict` 3.21 s、官方套件 21/90/285/1 **无回归**；**形参所有权改动试后撤回**（refcount 全零但暴露潜伏 UAF：test_dict/test_weakref SIGABRT/SEGV，`release_object ← instance_attribute_set`），留作下一轮独立审计；round 44：`==`／`!=` **特殊方法派发**（用户类 `__eq__`／`__ne__`、容器逐元素、`NotImplemented` 下一跳、`object` 默认实现跳过防递归）＋ `str` 一族参照口径（fillchar／`replace` count／`zfill` 符号／`expandtabs` 换行列／`rpartition('')`／搜索族 arity 报文／下标错型与大整数饱和／切片饱和算术／`%` 的 `*` 宽度与嵌套键与 `str.__mod__`／**生成的 title 映射表**）⇒ `test_userstring` **38F／4E ⇒ OK（skipped=2）**、官方套件跑通 **21 → 22**（未实现 90 → 89）；round 45：**嵌套 `with` 退出路径**（内层正常出口不再跳循环续点、`break`／`continue` 退出循环体内各层 `with`；`LoopFrame` 记 `with_levels`）⇒ `test_frame` **状态 5 解除**（跑到完，17F／19E／12 skip）；官方套件"未实现" 90 → **83**、跑通 22 不变（无回归）；round 46：**真值协议**（`__bool__` 必须恰返回 `bool`／`__len__` 整数且 `>=0`／类型对象恒真）修掉 `test_bool` 的 **Rust 栈溢出 abort**（改前退出 134，改后 6F／2E 正常跑完）；套件形态 22／83／291／1 不变；round 47：`bool`／`int` 面收口（`bool` 不可继承、`.real`／`.imag`／`.numerator`／`.denominator`／`.conjugate`、一元／位运算的 bool→int、`bool.from_bytes`、`bool(**kw)` TypeError、`BytesIO.getbuffer` ⇒ `test_pickle` 转绿、`eval` 结果先查 locals）⇒ `test_bool` **6F／2E ⇒ 2F**、官方套件跑通 **22 → 23**；round 48：`int.__new__`／`bool.__new__` 显式登记（`type.__new__` 桥接收口）＋ 核心侧 `DeprecationWarning` 通道（`~bool`）⇒ `test_bool` **全绿（OK）**、官方套件跑通 **23 → 24**；round 49：doctest 导入前置面（`sys.gettrace`／`stdin`／`displayhook`／`setprofile`、隐式异常 `__traceback__`、`backslashreplace`）⇒ 4 个 doctest 文件从**导入即死**推进到真正的 doctest 用例（`test_extcall`／`test_unpack_ex` 到 `DICT_MERGE` 缺口、`test_pep646_syntax` 到断言级、`test_genexps` 到生成器 `throw` 的 `StackUnderflow`）；跑通 24 不变、"未实现" 83→86（同一批文件从"运行期错"推进到"未接线"）；round 50：`DICT_MERGE`／`CALL_FUNCTION_EX` 协议面（`**映射` 走 `keys()`＋`__getitem__`、同名键报文带被调对象限定名、`*` 收自定义 `__iter__`、关键字名 `keywords must be strings` 且收 `str` 子类）⇒ 套件 24／83／289／1 不变；doctest 文件剩 `single` 模式交互 repr、`test_extcall` 的 **GC 双重释放**、生成器 `throw` 的 `StackUnderflow`；round 51：`compile(..., "single")` 交互式显示（内建 `__pyawa_displayhook__`）＋ `print`／`displayhook` 每次读**当前** `sys.stdout`（模块属性走属性通道，修 SIGBUS）＋ `repr(Ellipsis)` ⇒ `test_unpack_ex`／`test_pep646_syntax` 各剩 **1F**（星号目标链式赋值／`*args` 注解），套件 24／83／289／1 不变；round 52：以 `*` 打头的链式目标可解析（`a, *b, c = *d, e = range(5)`）＋ `DICT_UPDATE` 的 `'int' object is not a mapping` 报文 ＋ 用 `PYAWA_NO_TRACEBACK_ATTACH` 诊断开关把 `test_extcall` 堆损坏锁定到「异常↔回溯↔帧」环 × GC 回收（默认仍挂回溯）；`test_unpack_ex`／`test_pep646_syntax` 各剩 1F（星号目标绑定次序／`*args` 注解）；round 53：带星解包值不再误走"等长窥孔"（`*a, b, c = x, y, *z` 静默不赋值 ✗ ⇒ 修 ✓）、回溯改**链式**（旧 tb 接到新 tb 的 `tb_next`；`test_extcall` 堆损坏必崩 ⇒ 偶发 10/20）、推导式里 `*`／`**` 报参照 SyntaxError（先前静默生成结果 ✗）；套件 24／83／289／1 不变；round 239（行为变更轮·异常对象计算属性的写/删校验）：a) instance_attribute_set 的异常分支：args 接受任意可迭代转 tuple
（不可迭代 => iter_object 原话 'int' object is not iterable）、__cause__/__context__ 只接受 None 或 BaseException 子类
（否则参照原话 exception cause/context must be None or derive from BaseException）；载荷新引用转移、旧值逐个还
b) instance_attribute_delete_inner：异常只读属性名单 => TypeError: <名字> may not be deleted c) 真坑：binascii.Error
这类用户建类异常子类实例不走 ExceptionObject 载荷，直接强转 => SIGSEGV（binascii_module 入库测试当场红）=> 补载荷形状
核验（instance_size == size_of::<ExceptionObject>()），否则回退实例字典路 => test_exceptions 117F/30E（总147）=>
113F/30E（总143）、test_contextlib_async 13F/19E=>8F/19E（总32=>27 顺带收益）、binascii_module 3/3；
test_collections/test_dict 仍绿；分块十项全绿、fastcheck OK（50s）、check 16/16、0 warning；
`docs/rounds/04-post-m6-01.md`｜11579｜
round 238（行为变更轮·UnicodeError 属性面＋add_note 类型校验）：a) exception_getattr 新增 UnicodeError 属性面
（encoding/object/start/end/reason，args 布局同形 5 项；纯 UnicodeError 实参不足 5 一律 None）b) add_note_native：
__notes__ 不是 list => TypeError: Cannot add note: __notes__ is not a list（先前静默重建 list、覆盖用户值）=>
test_exceptions 120F/30E=>117F/30E（test_notes 3 个 subtest 转绿）；test_collections/test_dict 仍绿、test_iter 4F 基线；
分块十项全绿、fastcheck OK（39s）、check 16/16、0 warning；`docs/rounds/04-post-m6-01.md`｜11536｜
round 237（行为变更轮·SyntaxError 属性面）：exception_getattr 新增 SyntaxError 属性面——msg（args[0]）＋filename/lineno/
offset/text/end_lineno/end_offset（从 args[1] 六元组按位取，缺位给 None）；filename 是 SyntaxError 与 OSError 共有名字 =>
单独一条臂（SyntaxError 优先，否则 OSError 的 args[2] 口径；同 match 写两条 filename 臂会得 unreachable pattern 警告，
本轮实测并修）=> test_exceptions 不再中途中止：完整跑 107 个测试（FAILED 120F/30E/13S；绝大多数要再起一个解释器的
子进程支路，本层没有该能力；但能跑到收尾本身是可诊断面跃迁）；test_collections/test_dict 仍绿、test_iter 4F 基线；
分块十项全绿、fastcheck OK（43s）、check 16/16、0 warning；`docs/rounds/04-post-m6-01.md`｜11501｜
round 236（行为变更轮·协程前言＋del 名字作用域＋__debug__）：a) 纯协程也发 RETURN_GENERATOR/POP_TOP 前言（参照 async def
反汇编同形；POP_TOP 吃掉首次 send(None) 压的 None；先前只给有 yield 的函数发 => 纯协程帧底永久多一格 => 异常表 depth
差 1）b) emit_delete_name：函数里删非局部非 global 名 => DELETE_GLOBAL（先前 DELETE_NAME => 执行器需要命名空间帧中止）；
补 DELETE_DEREF（清空 cell）c) 内建 __debug__ = True（test_exceptions 导入即 NameError）=> test_exceptions 从导入即错
推进到能跑约半数用例（仍状态 1 停在 MySyntaxError.filename）；既有绿档 test_collections/test_dict/test_genexps/
test_userstring 无回归、test_iter 4F 基线；分块十项全绿、fastcheck OK（38s）、check 16/16、0 warning；
`docs/rounds/04-post-m6-01.md`｜11464｜
round 235（行为变更轮·三处 ABC/描述符真 bug）：a) classmethod/staticmethod/property 的 __isabstractmethod__ 委托底层函数
（参照 3.10+；Lib/abc.py 的 abstractclassmethod/abstractstaticmethod/abstractproperty 全靠它判抽象；先前一点没有 =>
test_abc 三格 AttributeError）b) super().__new__(cls) 的 staticmethod 语义（object.__new__/type.__new__ 在类字典里是静态的，
super().__new__(cls) 只传 cls 一个实参；先前 super 的"内建方法一律绑 self"多绑一份 => object.__new__ 收到两个 => TypeError
（test_all_new_methods_are_called））=> test_abc 6F/10E（总 16）=>6F/6E（总 12）；test_collections 保持整档绿 OK (skipped=3)、
test_dict OK；分块十项全绿、fastcheck OK（39s）、check 16/16、0 warning；`docs/rounds/04-post-m6-01.md`｜11424｜
round 234（诊断轮·async 推导式试接→撤回）：把 P3-35 的 async 推导式试接到能编译（Generator.is_async / parser 收 async for 层 /
生成器表达式外层 GET_AITER + co_flags 0x200 / 内联推导式 GET_ANEXT+SEND 循环 / .0 跳过 GET_AITER / aclose 忽略守卫），
但发现两件事决定整体撤回：1) async for 语句本身既有实现是坏的（最小探针 async for i in agen(3): out.append(i) 得 []，
循环体一次没跑）2) test_asyncgen（88 测试）在试接版下从「未实现状态 5」变成能跑到约第 74 格后挂死（timeout 90/120/250
都 rc=124）=> 会把超时带进 survey（比未实现更难查更慢）；整轮撤回（工作区回 HEAD，代码零改动；test_collections 仍
OK (skipped=3)、test_asyncgen 回状态 5）。改变计划的事实：先把 async for 语句修到真跑一遍（判据：最小探针得 [0,1,2]
且 test_asyncgen 不超时）再重落 async 推导式；线索：参照 async for 需要异常表条目 GET_ANEXT..YIELD_VALUE ->
END_ASYNC_FOR [1]，且 asend 的「完成即跳 END_SEND」与 async for 需要的「继续走 END_SEND/NOT_TAKEN/存目标」要分清；
撤回后 build 0 error/0 warning、fastcheck OK（41s）、check 16/16；`docs/rounds/04-post-m6-01.md`｜11384｜
round 233（行为变更轮·异步生成器 aclose/athrow）：先前异步生成器没有这两个方法（round 219 只在类型字典放存在性占位，
实例 getattr 槽不认）=> await agen.aclose() 落到占位原生（返回 repr 字符串）=> TypeError str object can't be awaited
（contextlib._AsyncGeneratorContextManager.__aexit__ 正卡这两条）。修法：AsendObject 加模式（0 正常/1 aclose/2 athrow）
+ 两个原生入口 + SEND 按模式分派；aclose 抛 GeneratorExit 推进到底并吞 GeneratorExit/StopAsyncIteration 后 await 得
None；athrow 抛异常、让出值照 asend 一步完成口径交出 => test_contextlib_async 13F/21E（总 34）=>13F/19E（总 32）；
test_collections 保持整档绿 OK (skipped=3)、test_dict OK；分块十项全绿、fastcheck OK、check 16/16、0 warning；
`docs/rounds/04-post-m6-01.md`｜11346｜
round 232（行为变更轮·it.__reduce__ 的内建函数缓存）：参照里 it.__reduce__() 交出的 iter/reversed 是内建函数对象本身
（C 层持有，用户换掉 builtins.iter 也不影响）；先前每次按名去 builtins 字典查 => 换掉后 RuntimeError: 内建
iter/reversed 不可用。修法：引导期缓存原生函数（AtomicUsize 借用指针，与内建字典同寿），__reduce__ 优先用缓存。
=> test_iter 3F/1E=>4F/0E（总数 4 不变、错误清零；test_reduce_mutating_builtins_iter 已到断言层，只剩耗尽形态
依赖 CPython 字典比较副作用那一差，如实登记）；test_dict OK、test_collections OK (skipped=3)、test_contextlib_async
13F/21E 不回退；分块十项全绿、fastcheck OK（50s）、check 16/16、0 warning；`docs/rounds/04-post-m6-01.md`｜11309｜
round 231（行为变更轮·async 三处收口）：a) 新增内建 aiter/anext（contextlib.py 的 await anext(self.gen)；先前该名字不存在
=> test_contextlib_async 的 AsyncContextManagerTestCase 整族 NameError）b) async with/async for 的等待循环改成与 await
同批形状（标签在 SEND、无 POP_TOP）c) 不再给 async def 补合成 yield（round 219 起协程有真 coroutine 类型，调用路径
自己 suspend；合成 yield 反而让每个协程结尾多挂起一次 => run_no_yield_async_fn 的 coroutine did not complete）
=> test_contextlib_async 28F/19E（总 47）=>13F/21E（总 34）；test_collections 保持整档绿 OK (skipped=3)；test_contextlib
8F/4E、test_iter 3F/1E 不回退，test_dict/test_userstring/test_genexps OK；test_asyncgen 的 async 推导式仍是既有缺口；
分块十项全绿、fastcheck OK（41s）、check 16/16、0 warning；`docs/rounds/04-post-m6-01.md`｜11274｜
round 230（行为变更轮·await 循环的 POP_TOP + try 体内 await 真落地）：真开 try 体内 await 后逐指令跟踪发现
await 循环 RESUME 后多了一条 POP_TOP——参照 3.14 是 SEND; YIELD_VALUE 1; RESUME 3; JUMP_BACKWARD_NO_INTERRUPT L_send，
没有 POP_TOP；多弹一格 => 跳回 SEND 时接收者被弹掉 => 第二次 send 在 SEND 处 StackUnderflow（off27 SEND depth=2 ->
off29 YIELD -> off30 RESUME -> off31 POP_TOP(depth2->1) -> off32 JUMP -> off27 depth=1）；删 POP_TOP 后 test_collections
整档转绿 OK (skipped=3)（总失败 0）；try 体内 await 的透明回退取消（只 except 处理块体内保留）；test_genexps/test_dict OK、
test_iter 3F/1E、test_contextlib_async 28F/19E 不回退；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜11234｜
round 229（行为变更轮·await 循环标签真 bug）：参照 3.14 await 字节码是 GET_AWAITABLE; LOAD_CONST None;
L_send: SEND L_done; YIELD_VALUE 1; RESUME 3; JUMP_BACKWARD_NO_INTERRUPT L_send; L_done: END_SEND——恢复后要跳回 SEND
重送；先前标签打在 YIELD_VALUE 上、跳回 YIELD_VALUE => 恢复时重复 yield 而非重送 => 驱动方收多余值/StackUnderflow
（test_AsyncGenerator 的 IgnoreGeneratorExit().aclose()）；修后 await 一次送完（g: return (await h())+1 一次驱动得 3、零让出）；
test_collections 保持 0F/1E；try 体内 await 仍走透明回退（本轮试过真接线 => 第二次 send 时 SEND@27 StackUnderflow，
下一步补 CLEANUP_THROW 段）；分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜11201｜
round 228（行为变更轮·type.__call__/import unittest 真因收口）：查明并修「元类型 HAS_INSTANCE_DICT 位」——类型对象的实例
字典就是 TypeObject.dict，这一位必须一开始就置；先前只是靠某条路径碰巧先调 type_namespace（顺手置位）才正常。
round 227 装 type.__call__ 时 set_type_attribute 先把 type 命名空间惰性建出来（那条路不置位）=> 之后类型对象的实例字典
判断失败 => 类属性删除（del C.x/dataclass delattr(cls,field)）报 'type' object has no attribute 'x' => import unittest
整条链红（_colorize.Theme 的 dataclass 处理）。修好这一位后 type.__call__ 正式装进类型字典 => Callable 检查过 =>
test_Callable 转绿、import unittest OK；test_collections 1F/1E=>0F/1E（总数 2=>1，只剩 test_AsyncGenerator）；
test_dict OK、test_call OK（skipped=182）、test_userstring OK、test_genexps OK；abi 测试 47 passed（先前通用置位版会
让 dict 实例长字典 => tables_support_field_and_subscript_access 红，改成只给元类型置位）；分块十项全绿、fastcheck OK、
check 16/16；`docs/rounds/04-post-m6-01.md`｜11168｜
round 227（行为变更轮·同一族真 bug＋泄漏）：a) dunder_call_native 实参双重释放（与 round 224 generic_alias_call 同一坑：
call_callable 转移语义、却把借来的 rest.to_vec()/kwargs.to_vec() 裸交 => 每条实参净 -1）改为逐项 retain＋move
b) 三处新引用泄漏补还：generic_alias_mro_entries 的 origin（多 retain）、generic_alias_getattr 转发的 origin、
typing.rs 的 __args__ args c) type.__call__ 仍不装（装上后 import unittest 立刻红 AttributeError: 'type' object has no
attribute 'argparse'；两处属性读错误构造点加调试都没命中 => 不是普通缺名，差额如实登记）=> 无新增红：test_collections
1F/1E 不变、test_dict OK、test_userstring OK、test_genexps OK；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜11128｜
round 226（行为变更轮·三处 ABC 面）：a) array.array 惰性注册 MutableSequence（array build 时机 collections.abc 是包、
桥未接包导入 => 改为第一次被 isinstance/issubclass 问到时补注册，OnceLock 一次性）b) type.__flags__（HEAPTYPE 1<<9
＋按 MRO 名判 SEQUENCE 1<<5/MAPPING 1<<6）c) __abc_tpflags__ 合法性校验（同时声明序列+映射 => 参照 TypeError
"a class cannot be both a mapping and a sequence"）=> test_collections 2F/2E=>1F/1E（总失败 4=>2；
test_MutableSequence 与 test_illegal_patma_flags 转绿）；test_dict OK、test_abc 总数与基线一致；分块十项全绿、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜11091｜
round 225（行为变更轮·两处真缺口）：a) math.prod(iterable, *, start=1)（走迭代器协议+任意精度核心；Counter.elements
doctest 因它 AttributeError）b) dict 子类的 __reduce_ex__ 第 5 项给 iter(obj.items())（先前 None => copy.copy(Counter)
得空 Counter）=> test_collections 4F/2E=>2F/2E（总失败 6=>4；test_copying 与 elements doctest 转绿）；试过并撤回：
array 模块 build 里注册 MutableSequence（该时机 collections.abc 是包、import_through_bridge 未接包导入 => None；
且返回借用、外层 release 会错）留作下轮惰性注册；test_dict OK、test_userdict 基线；分块十项全绿、fastcheck OK、
check 16/16；`docs/rounds/04-post-m6-01.md`｜11053｜
round 224（行为变更轮·真因落地）：查明 round 222/223 堆损坏真凶 = generic_alias_call 实参双重释放（call_callable
是转移语义：接管 Vec 每项引用、调用完逐项 release；round 216 误信"只借不接管"注释写了 retain+clone 传入+再 release
=> 每条实参净 -1 => namedtuple 保存的引用悬垂 => malloc unaligned tcache 崩）；修法：retain 后 move 进 call_callable、
删 release（同 call_value 形状）；重落 __args__ 恒 tuple；另修 type.__module__ 读法（模块对象原样给）+ bytes/bytearray/
memoryview 补 __buffer__ 占位 => test_collections 7F/2E=>4F/2E（总失败 7=>6，连跑 7 次零崩；test_module_parameter/
test_Buffer/test_non_generic_subscript 转绿）；test_funcattrs 与基线一致；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜11016｜
round 223（行为变更轮·撤回＋真修）：撤回 round 222 的 GenericAlias.__args__ 包一元组（实测偶发堆损坏
malloc(): unaligned tcache chunk detected，同二进制连跑 4 次 2 次崩）=> 连跑 4 次全稳；保留算术反射回退/dir 恒含 __class__/
别名相等；另修 alias __eq__ 四个属性引用漏还；代价如实：test_collections 6F/2E=>7F/2E（test_non_generic_subscript 重新红），
用 1 格换不再堆损坏；分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10974｜
round 222（行为变更轮·四处真 bug）：a) 左操作数用户覆写返回 NotImplemented 后仍问右侧原生反射（Evil+1 => int.__radd__ => 1）
b) dir(x) 恒含 __class__ c) GenericAlias.__args__ 恒是 tuple（单参数时先前是 int 本身）d) GenericAlias 相等比较
（values_equal 按 __origin__/__args__ 比 + 类型字典幂等装 __eq__）=> test_collections 8F/3E=>6F/2E（总失败 11=>8）；
test_dict OK、test_binop/richcmp OK；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜10942｜
round 221（行为变更轮）：hash(frozenset) 照抄 _collections_abc.Set._hash（u64 环绕、-1 特例）+ hash(object()) 身份哈希
（_Py_HashPointer 形态）=> test_Set_hash_matches_frozenset 转绿、test_collections 8F/4E=>8F/3E（总数 12=>11）；
13 组样例 hash(fs)==Set._hash(fs) 全部一致；test_dict OK、test_iter 3F/1E、test_genexps OK；分块十项全绿、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10905｜
round 220（行为变更轮·await 真接线）：Expression::Await 新增，发射照参照形状 GET_AWAITABLE/SEND/YIELD_VALUE/
RESUME/JUMP_BACKWARD_NO_INTERRUPT/END_SEND（与 async with/for 同一套）；连带补 collect_names_in_expression_raw 与
find_lambda_demands 下钻 Await 子表达式（否则 await 实参里的 genexpr 引用的外层局部不标 cell => import asyncio 报
"生成器表达式引用 self：cell/自由变量槽缺失"）；如实登记：try 体里的 await 暂时退回透明（try_depth>0），真等待的合成
指令要把异常表 depth 算对，当前算不准 => 处理块展开 StackUnderflow；顶层 await 已真等待（await h()+1 => 3）；
test_collections 11F/3E=>8F/4E（总数 12 持平）、test_genexps OK、test_contextlib_async 28F/19E（35F/12E，总数持平）、
import asyncio OK；分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10873｜
round 219（行为变更轮·async 类型化）：async def 真正变成 coroutine/async_generator 类型——Statement::Def 记
is_async/async_generator（parser 在补合成 yield 前判）、嵌套单元发射前改 co_flags（清 0x20、按需 0x80/0x200）、
call.rs wrapped_type 判定顺序 0x200/0x80/0x20、coroutine 类型字典补 __await__、async_generator 补 __aiter__/__anext__/
asend/athrow/aclose（存在性给 ABC _check_methods）；清缓存后 test_collections 11F/3E=>9F/3E（总失败 14=>12）；
co_flags 0x83/0x203 与 CPython 一致、普通生成器不再被注册成协程；test_contextlib_async 35F/12E 不退、test_dict OK；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10847｜
round 218（行为变更轮）：空 __slots__ 的只读语义——register_type_slots 空表也登记 + classes.rs 去掉非空守卫 +
属性写判据改"类型命名空间里有 __slots__"（namedtuple 是 type() 三参造的不在用户类登记表）=> namedtuple p.x=5 / 
del p.x 报 AttributeError；另补不可变序列 del x[0] 的参照 TypeError（'tuple' object doesn't support item deletion）
（先前落未接线中止）；修后 test_collections 12F/3E=>11F/3E（总失败 15=>14）；test_dict/descrtut/binop OK、test_list 1E
（CM-8 基线）；分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10806｜
round 217（行为变更轮）：二元运算反射优先级判据——右侧类型自己定义反射 dunder 才先问右侧（type_lookup_owner(right_type,
reflected).owner == right_type）=> ChainMap()|Subclass() 得 ChainMap、ChainMap()|SubclassRor() 得 SubclassRor（与 CPython
一致）；先前固定先左后反射 => test_union_operators 末格红；修后 test_collections 13F/3E=>12F/3E（总失败 16=>15）；
test_dict/genexps/userstring OK；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜10771｜
round 216（行为变更轮·4 个缺面）：a) set 的 __sub__/__or__/__and__/__xor__ 类型字典项（走 executor::set_operation）
b) function.__builtins__（取 __globals__["__builtins__"]，没有给空 dict）c) GenericAlias.__parameters__（非泛型给 ()）
d) GenericAlias.__call__ 转调 origin(*args)（实参各 retain 一份再用、用完还；先前裸指针转交 => namedtuple 别名调用
提前释放实参 => malloc 崩）；修后 test_collections 12F/6E=>13F/3E（总失败 18=>16）；test_dict OK、test_iter 3F/1E、
test_userstring/genexps/richcmp/compare OK；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜10737｜
round 215（行为变更轮）：a) dict.__eq__ 槽位/快照混用（dict_entries 活快照按表槽位索引 => 有删除的 Counter 比较错位
=> == 假）b) 富比较内建一侧被跳过（Counter.__eq__ 给 NotImplemented 后应问右侧 dict.__eq__，本层 continue + Fallback
身份兜底 => Counter==dict 判 False）c) function/builtin_function_or_method/method 类型字典补 __call__（Callable 的
_check_methods 查 C.__dict__；type.__call__ 那格会打坏 import unittest => 只加前三个）；修后 test_collections
13F/6E=>12F/6E（总失败 19=>18）；test_dict OK、test_iter 3F/1E、test_richcmp/test_compare OK；分块十项全绿、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10699｜
round 214（行为变更轮）：两个类型字典缺项——a) tuple/list/str/bytes 的 __getitem__（Point.__getitem__ ==
tuple.__getitem__，test_factory 钉住）b) dict.__reversed__（Reversible 的 _check_methods 要看 dict.__dict__）=>
test_collections 14F/7E=>13F/6E（总失败 21=>19）；新增 container_getitem_native/dict_reversed_native 并装进 bootstrap；
test_dict OK、test_iter 3F/1E、test_userstring/test_genexps OK；test_ordered_dict 的 NoneType 中止是既有；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10662｜
round 213（行为变更轮·3 个真 bug）：a) 仅位置形参+**kwargs：def f(self,/, **kw) 的 f(self=1) 应把 self 收进 kw（先前一律
报 TypeError => Counter(self=42) 挂）b) 不可哈希容器的 __hash__：list/dict/set/bytearray 类型字典要放 None（Hashable 的
_check_methods 靠 is None 判不可哈希 => 先前 isinstance(bytearray(), Hashable) 判 True）c) compact_if_needed 容量下溢
panic：hashes 为空的 dict（mappingproxy_new 一类）里 hashes.len()-deleted 下溢 => capacity overflow；修后 test_collections
15F/11E=>14F/7E（总失败 26=>21）；test_dict OK、test_iter 3F/1E、test_userstring/test_genexps/test_call 均不退；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10629｜
round 212（行为变更轮）：dict 子类的 __contains__/__getitem__ 覆写——class lowerdict(dict) 覆写时 in 与 d[k] 都走内建
哈希快路 => 覆写被跳过 => ChainMap.get 返回默认值（test_new_child 100!=20）；修法：in 与 d[k] 的 dict 快路只在"有效
__contains__/__getitem__ 就是 dict 那份"时走 + 给 dict/list/tuple/set/str 类型字典补 __contains__、dict.__getitem__
（专用内建语义 native，避免子类覆写里 dict.__contains__(self,k) 递归）=> test_collections 16F/11E=>15F/11E（总数 27=>26，
test_new_child 转绿）、test_dict OK、test_iter 3F/1E 不退；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜10594｜
round 211（行为变更轮）：tuple 子类的 protocol 0/1 还原形状（_reconstructor 的 base=tuple、第三参数=__getnewargs__()、
外层 state=None；不动 tuple_new——本层 tuple.__new__(cls,iterable) 静态调用本来就成立，round210 段错误正是"在 tuple_new
里猜类实参"引起）=> NT.__reduce__ 与参照逐项一致、copy.copy/pickle(0) 重建成功、test_collections 16F/11E（总失败 27）、
test_iter 3F/1E 不退；分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10557｜
round 210（行为变更轮）：tuple 子类的 protocol>=2 __reduce_ex__ 形状（copyreg.__newobj__ + __getnewargs__）=>
namedtuple copy.copy/pickle 重建成功、test_collections 16F/13E=>17F/11E（总失败 29=>28）；顺手修 tuple_items 借用
顺序（先 retain 再放 newargs，否则 test_iter 段错误）；试过并撤回 proto 0/1 tuple 形状与 tuple.__new__ 静态形态
（致 test_iter.test_builtin_tuple 段错误）；test_iter 3F/1E 不退、test_dict OK；分块十项全绿、fastcheck OK、
check 16/16；`docs/rounds/04-post-m6-01.md`｜10524｜
round 209（行为变更轮）：内建族子类的"中间层 Python 覆盖"属性解析——class MyCounter(Counter) 的 m.update 先前解析成
dict.update native（先命中继承来的 getattr 槽）=> m.update(None) 报 NoneType not iterable；修法：内建族快路先记命中内建
基类 base，type_lookup_owner 的 owner==object_type 或 owner!=base（中间层）且可调用 => 绑 Counter.update =>
test_collections 16F/15E⇒16F/13E（TestCounter.test_copy_subclass/test_helper_function 转绿，总失败 31⇒29）；
test_dict OK、test_iter 3F/1E、test_genexps OK、test_userstring OK、test_abc 4F/14E 不退；分块十项全绿、fastcheck OK、
check 16/16；`docs/rounds/04-post-m6-01.md`｜10483｜
round 208（行为变更轮）：type('C',(ABC,),stubs) 的元类转交——type() 三参创建时最派生元类不是 type 且带 Python 级 __new__
就转交（先前直接 build_class => 元类 __new__ 不跑 => __abstractmethods__ 继承基类那份 => validate_abstract_methods
报"抽象类不能实例化"）；两次收紧：先判 winner==被请求元类（否则 EnumType.__new__ 里 super().__new__ 再转交回自己=>
无限递归）、再只对 ABCMeta 一族转交（否则 import enum 报 'dict' has no attribute '_member_names'）；验证 import enum OK、
type('C',(Sequence,),stubs) __abstractmethods__=frozenset()、test_collections 13F/32E=>16F/15E（总失败 45=>31）、
test_abc 4F/14E 与基线一致；分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10450｜
round 207（行为变更轮）：修 try/except 处理块异常表区间段尾——体以 break/continue/return 结束时出口会就地发"余部副本"
（POP_EXCEPT+循环后代码），先前段尾取体后 POP_EXCEPT 之前 => 副本被圈进 handler 段 => 副本里抛异常派发到清理三连
（栈只有 [lasti,exc] 2 格）=> COPY 3 下溢；按参照把段尾回退到最后一条 POP_EXCEPT => 最小复现不再下溢、handler 段
[23,34) 与参照同形、test_collections 中止=>FAILED(13F/32E/3s)；test_dict OK、test_iter 3F/1E、test_genexps OK、
test_userstring OK 不回退；分块十项全绿（conformance 两档 5/5 => 字节码夹具未破）、fastcheck OK、check 16/16；
另给 PYAWA_DUMP_CODE 补 [exctable] 转储；`docs/rounds/04-post-m6-01.md`｜10414｜
round 206（诊断轮）：新增 PYAWA_COPY_DEBUG（COPY 越界打出栈上各类型）；最小复现 try/except 里 break 走清理三连 COPY 3
时栈为 ["int","ValueError"]（lasti+异常、缺 PUSH_EXC_INFO 的 prev）=> 该次进入不是从处理块内部，而是从处理块段异常表
条目之外的路径（break 后余部副本被某条处理块段区间误盖，或 RERAISE 0 再派发走错）；结合 round 205（flip lasti
能过最小复现但全套件挂"指令 70"）=> 真因在处理块段的区间边界/目标，不在 lasti 位本身；test_dict OK、test_iter
3F/1E 不退；small gate OK、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10378｜
round 205（诊断轮）：StackUnderflow 来源钉到"异常表 lasti 位 vs 清理三连 COPY 3"——新增 PYAWA_FRAME_ERR_DEBUG（帧错误
打 code@ip/栈深 + f_back 链），现场 Logs=Sequence.index@203 stack=2、最小复现=while True: try: v=seq[i] except
IndexError: break（清理三连 COPY 3 要栈深 3，实际 2，PUSH_EXC_INFO 前少 lasti）；试把 body→handler 异常表 lasti
位 false→true => 最小复现通过但所有套件挂"指令 70 未接线" => 撤回（配套口径未对齐，记入下轮）；test_dict OK、
test_iter 3F/1E 不退；small gate OK、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10343｜
round 204（性能轮）：Frame 的 pop/peek/swap_from_top/peek_from_top 里的 PYAWA_STACK_DEBUG 开关改 flag_cached
（OnceLock<bool>）——这几处每条字节码都走，原先每次 std::env::var_os；另 PYAWA_FRAME_LOCALS_PROXY 同样缓存；
test_userstring 88-90s⇒86s、test_dict/test_iter 不退、test_collections 仍既有 runtime 中止（StackUnderflow 非本轮引入）；
分块十项全绿、fastcheck OK、check 16/16；未解：StackUnderflow 不是来自 Frame::pop（加了诊断没打印）=> 下轮逐点打回溯；
`docs/rounds/04-post-m6-01.md`｜10296｜
round 203（行为变更轮）：deque 下标接线（读/写+负下标：DequeObject::item/replace；越界 IndexError deque index/assignment
index out of range）+ 顺手修 dict.setdefault 的槽位/快照混用 panic（先前 dict_entries 快照按表槽位索引 => test_collections
里 index out of bounds len 54 index 54）=> test_contextlib unimplemented⇒runtime（FAILED failures=9 errors=5）、
test_contextlib_async ⇒runtime（35F/13E）、test_collections 不再 panic（回到既有 runtime 档 StackUnderflow）；
test_dict OK、test_userstring OK、test_iter 3F/1E；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜10263｜
round 202（行为变更轮）：Context 的映射面最小接线（__getitem__/__len__/__iter__）=> test_context 从 unimplemented
（下标协议报"这里是 'Context'"中止）回到 runtime（FAILED failures=12 errors=17 skipped=24）；试把 get/__contains__/copy
也接真实现会触发 malloc(): unaligned tcache chunk detected => 撤回那三格，只留三个最小面（如实登记）；顺带发现
test_contextlib 报"下标…'deque'"=deque 下标未接线；test_dict OK、test_userstring OK、test_iter 3F/1E 不退；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10226｜
round 201（工具轮）：整轮 survey 补跑完成 79 ok/0 语法错/69 未实现/244 运行期/5 超时（397 全跑）；逐格对比上一轮仅 2 处变化：
test_userstring timeout⇒ok（round199 性能修）、test_context runtime⇒unimplemented（新中止"指令44 下标…'Context'"，
round194 也复现，下轮定位）；工具加固 libtests_survey.py：_run_capture 改 Popen(start_new_session)+超时 killpg 杀整个
进程组（test_ntpath 在高负载下卡不可中断 IO 会把驱动挂到 600s 上限）、新增 --skip FILE（在 start/end 切片之后过滤，
序号口径不变）；small gate OK、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10192｜
round 200（行为变更轮）：iter(callable, sentinel) 的重入耗尽口径（gh-101892）——callable 自己把本迭代器耗尽时本次结果
不再交出、直接 StopIteration；修后 test_iter 4F/1E⇒3F/1E（test_iter_function_concealing_reentrant_exhaustion 转绿；
剩 3F=test_exception_locations 三档要 AST、1E=test_reduce_mutating_builtins_iter）；test_dict OK；分块十项全绿、
fastcheck OK、check 16/16；整轮 survey 未跑完（如实：跑完 209/397，47 ok/36 未实现/124 运行期/2 超时；200-240 片撞
平台 600s 上限，下轮拆细补跑）；`docs/rounds/04-post-m6-01.md`｜10150｜
round 199（行为变更轮·性能回归修）：找到 round190-198 诊断口引入的热路径 std::env::var_os（incref/release 每次都查
"PYAWA_WATCH_FRAME"；另 new_traceback/Owned::drop/attribute_clear/raise 等更热点）=> test_userstring 86s→108s（survey
翻 timeout）；新增 diag::flag_cached（OnceLock<bool>）改 7 处开关 => test_userstring 回到 88-90s；test_dict OK、
test_iter 4F/1E 不退；分块十项全绿、fastcheck OK、check 16/16；全量 survey 本轮未跑完（如实登记；按上轮 78/68/245/6
＋userstring 修回推 79/68/245/5，下一轮实测替换）；`docs/rounds/04-post-m6-01.md`｜10115｜
round 198（行为变更轮·泄漏硬化）：8 处「有就调用、没有就算了」探针分支里直接丢弃 Err(Raised) 的地方统一改
discard_error（dunder_text/length_of/call_dunder_method/call_object_method/__format__/index_value x2/mappingproxy 降级/
subscript 3 处）；量化对照：test_weakref 四连 skip live 1 481 512，关回溯挂载（NO_TRACEBACK_ATTACH=1）后 779 019（≈一半）；
tb_created/freed 差 108 ⇒ 主攻方向＝回溯挂载对帧链的保留；全档 weakref 175s 仍超时；test_iter 4F/1E 不退；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10075｜
round 197（行为变更轮）：**attach_traceback 同帧去重早退分支又一处漏还**——attribute_optional_of 交出的 previous_frame
（新引用）在该分支没还 => 帧 rc 多 1、整条帧链不释放；补上后 fai2.py 的 done 变 True、test_iter 5F/1E⇒4F/1E
（test_free_after_iterating 转绿）；定位靠新诊断口 PYAWA_TB_CLEAR_DEBUG（15 颗回溯释放都有 tb_frame，S.__getitem__ 帧
只被 2 条回溯释放而 watch 记 +3 => 探针那份没还）；test_dict/userstring/genexps/descrtut/userlist 全 OK；分块十项
全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜10030｜
round 196（诊断轮）：两个只读诊断口 PYAWA_OWNED_DROP_DEBUG（Owned<T> 落域打 ptr/类型）+ PYAWA_FRAME_DROP_DEBUG
（call_callable 建帧/执行完/返回前帧 rc）；把帧保留账算清：S.__getitem__ 帧 +3/-3 完全平衡、Owned<Frame> 落域确实
发生（owned-drop ty=frame），漏的是"某一条回溯释放时没还 tb_frame"（+3 里只还 2 条回溯引用，末态 rc=1）；
tb_created==tb_freed(15/15) 说明回溯对象都释放了 => 下一轮盯回溯释放时 attribute_clear/挂载字典的释放路径；
small gate OK、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9998｜
round 195（行为变更轮）：**两个真 bug 一起修**——① mappingproxy_live 裸指针登记且代理释放不注销 => 代理地址被新 dict
复用后 == 里活视图同步把老源条目拷进新 dict（{} != {'foo':'bar'} 陈旧内容）；修法：源持强引用、release_one 按类型名
调 forget_mappingproxy 注销、sync_mappingproxy 只对真 mappingproxy 且源仍活着、两个调用点按类型过滤；
② 根因修掉后 iter_value 的 __iter__ 探针可以安全改成"只查不抛 + discard_raised"（此前三种就地还被 bug ① 污染了
实验结果）；验证 test_dict 20/20、fai2 tb_created==tb_freed 15/15、test_iter 5F/1E 不退（剩余是另一处帧保留）；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9965｜
round 194（诊断轮）：PYAWA_WATCH_SITE_WATCH=1（盯住指定站点抛出的那一颗异常，之后每次 incref/decref 打现场+回溯）；
钉到：那颗泄漏 AttributeError 收支 +2/-2 平衡、末态 rc=1 => 只漏 Err 那份（iter_value Err 分支丢引用，round 171 起已知）；
摘回溯两条路都不行：①走属性协议 set_attribute_value => test_dict 10/10 必失败（异常正在 pending 槽里时属性协议再抛）
②低层 dict_remove_key => 7/10 偶发 => 一碰异常挂载字典就抖出 latent 堆问题；两条都撤回；方向改为"从头不造异常"但先清
堆问题；实验全撤、test_dict 5/5 稳、test_iter 5F/1E 不退；small gate OK、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9920｜
round 193（行为变更轮·小修）：attach_traceback 的"同帧去重"早退分支补 pending_exception 槽的正确交接（先前直接
set_pending(Some(saved)) 丢掉返回值 => 槽里那颗没释放）；「iter_value 改成不造异常」实验做了且测了：tb_created==
tb_freed（15/15）但 test_dict 2/5 偶发失败 => 撤回（保留"不造异常是正解方向"的结论）；dedupe 修复未改变 fai2 的
16/15（那颗泄漏另有来源，记入 NEXT）；test_dict 10/10 稳、test_iter 5F/1E 不回退；分块十项全绿、fastcheck OK、
check 16/16；`docs/rounds/04-post-m6-01.md`｜9882｜
round 192（诊断轮）：又两个只读诊断口（PYAWA_GC_REVERSE_EXTRA 打 traceback/异常族 rc/ext/父链；PYAWA_WATCH_SITE 指定站点
抛异常打 Rust 回溯）；钉到根链：泄漏回溯持有链 dict→AttributeError（根，非候选持有者），该 AttributeError 来自
iter_value 的 attribute_lookup_special("__iter__") 探针；三种"就地还"改法（摘回溯+只还 Err/只摘不还/round173 同款）
实测让 test_dict.test_constructor 偶发或必失败（基线 10/10 稳）=> 全部撤回，只留诊断口；结论：下一轮改成"不造异常"的
探测（先 attribute_optional/type_lookup_owner 判有没有 __iter__）；test_iter 5F/1E 不回退、test_dict 5/5 稳；
small gate OK、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9846｜
round 191（诊断轮）：加回溯对象计数（GC_LOG 打 tb_created/tb_freed）+ PYAWA_TB_DEBUG（每次 traceback 创建/释放打帧地址、
code 名、现场）；钉到具体一颗：fai2.py 最小复现 16 创建/15 释放 => 恰好一颗回溯没释放，它是 next() 抛 StopIteration 时
在 wrapper@40 挂上的（持有 wrapper 帧 => 帧成根 => A.__del__ 不跑）；结论=异常本体（或其 __traceback__ 引用）没被
完全释放，下一轮盯 StopIteration 释放账；test_iter 5F/1E 不回退；small gate OK、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9806｜
round 190（诊断轮）：给「帧被钉住」加只读诊断口——debug_holder_of 扩到全部实例指针字段（global_trace/
global_profile/modules/pending/tuple_pool）、新增 PYAWA_WATCH_FRAME/PYAWA_WATCH_FRAME_CODE（帧 incref 报现场+回溯）；
钉到：fai2 最小复现里 wrapper 帧在 next() 抛 StopIteration 路径被 new_traceback 连续加引用（rc 到 4），回溯对象
自身已释放但帧仍 rc=1/2 成根、持有者在候选集之外 => 病灶锁定「回溯释放时 tb_frame 那份引用漏还」；试给 traceback
类型正式登记（带 traverse/clear）无效果且早期做法引发递归爆栈 => 撤回；test_iter 5F/1E 不回退；small gate OK、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9768｜
round 189（行为变更轮）：**bytes.fromhex/bytearray.fromhex 接线**（ASCII 空白跳过、奇长/非十六进制参照 ValueError、
非 str TypeError）+ **linecache 注册源码实验撤回**（f.line 立刻有文本，但 inspect 也找得到源码 => ast.parse 未接线
=> 大片 unimplemented abort，与 round 178 同一个雷）；留下诊断事实：test_free_after_iterating 最小复现显示被 except
处理过的 StopIteration 仍钉回溯帧（A.__del__ 不跑），下一轮用 GC_REVERSE 定位；test_iter 5F/1E 不回退、
test_dict/userstring/genexps/list 不退化；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9728｜
round 188（行为变更轮）：**回溯 tb_lasti 口径对齐本层 co_positions()**（指令序号×2，按 inline_cache_entries 换算；
extract_tb 的 line/col 全对：for 表达式 17-45、raise 12-36）+ **同帧不重复入链**（协议层重抛不再给同一帧接两节）；
test_iter 4F/2E⇒5F/1E（exception_locations next_raises 转绿；剩 f.line 为空=占位 co_filename ⇒ linecache 拿不到源码行，
下一轮注册源码）；test_dict/userstring/genexps/list 不退化；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9689｜
round 187（行为变更轮）：**迭代器的永久耗尽闩**——callable_iterator 撞哨兵、sequence iterator 撞 IndexError/StopIteration
都永久耗尽（setstate 清闩、耗尽态 reduce 给 (iter,((),))）=> test_iter 8F/3E⇒4F/2E（sinkstate×2/mutating_seq_class_
exhausted_iter/mutating_seq_class_iter_pickle/stop_sequence 转绿）；剩余 2E（exception_locations next_raises/
reduce_mutating_builtins_iter）+4F（exception_locations init/iter_raises/free_after_iterating/
iter_function_concealing_reentrant_exhaustion）；test_dict/userstring/genexps/list 不退化；分块十项全绿、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9652｜
round 186（行为变更轮）：**迭代器类型的类属性面**——13 个内建迭代器类型字典补 __iter__/__next__（值只用于存在性），
collections.abc.Iterator 的 __subclasshook__ 终于认得出 => isinstance(iter(x), Iterator) True；test_iter 11F/3E⇒8F/3E
（iter_callable/seq_class_iter/mutating_seq_class_iter_pickle 等 3 格转绿；含踩坑：早期引导段 self.alloc 会 panic，
挪到 bootstrap_type_natives 才对）；test_list 1E 基线、test_dict/userstring/genexps 不退化；分块十项全绿、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9615｜
round 185（行为变更轮）：**operator.countOf/indexOf 接线** + **迭代器 pickle 三处 __reduce__**（range/longrange 按 Islice+count
算术推剩余值、callable_iterator 照参照无 state、sequence iterator 给 (iter,(target,),index)）=> test_iter 8F/13E⇒11F/3E
（问题总数 21⇒14；countOf/indexOf/iter_basic/iter_for_loop/iter_range/iter_big_range 转绿）；新增 11F 是原本被 pickle 异常
遮住的 isinstance(it, collections.abc.Iterator) 断言现在真的跑到（下一层缺口）；test_dict/userstring/genexps/list 不退化；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9575｜
round 184（行为变更轮）：**in 的迭代回退走富比较协议**（container element 在左、查找值在右：1==ALWAYS_EQ 反射 True、
NEVER_EQ==ALWAYS_EQ 左值 False 不再反射）+ **用户 __iter__/__getitem__ 异常原样上抛**（只有 TypeError 才落"不是容器"）
=> test_iter 8F/13E⇒7F/13E（test_in_and_not_in 转绿）；test_contains OK、test_dict/genexps OK、test_list 1E 基线；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9540｜
round 183（行为变更轮）：**sys.settrace/setprofile 改成真存**（Instance global_trace/global_profile 两份 RefCell，retain/release
存取；sys.gettrace() is f、None 清掉）+ **sys.unraisablehook/__unraisablehook__**（可存可换，catch_unraisable_exception
可用）；如实登记：事件驱动仍只有帧本地 f_trace 的 'line'；setprofile 只存不发；test_frame entry 用例从 AttributeError
变成真正执行（cm.unraisable is None）=> 揭示 __del__/收尾期 unraisable 事件还没有发布者；test_frame 10F/7E 不退；
分块十项全绿、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9506｜
round 182（行为变更轮+全量验收）：**本地追踪尊重 f_trace_lines=False**（下发 'line' 前读挂载字典，显式 False 不发；
事件路径只在 trace_active 时进入 ⇒ 零热路径开销）；探针验证写 f_trace_lines=False 后不再发事件；test_frame 10F/7E 不退；
全量 survey 79 ok/68 unimplemented/245 runtime/5 timeout（与 round 180 一致）；small gate OK、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9469｜
round 181（行为变更轮）：**帧本地追踪的 'line' 事件驱动**接上——Frame 加 trace_active/trace_last_line 两个 Cell；
maintain_frame_trace 在 attribute_write 与 set_attribute_with_protocol 两处入口维护开关（STORE_ATTR 走后者）；
execute 主循环行号变化时调 trace(frame,'line',None)、trace_dispatching 防递归、不可调用值不发事件；
探针 f_trace=trace 后 3 行 => 3 个 line 事件（先前 0）；test_frame 10F/7E 不退；分块十项全绿、fastcheck OK、
check 16/16；`docs/rounds/04-post-m6-01.md`｜9441｜
round 180（验收轮）：**全量 survey 复测 79 ok / 68 unimplemented / 245 runtime / 5 timeout**（round 178 的 69/244 微调、ok 不变）；
补 tools/libtests_named_runner.py 的父包属性（`from test import test_xxx` 自引用不再 ImportError，test_pdb named 跑法
改为撞 ast.parse 下一层缺口）；近绿 1 格套件逐一核对（c_locale_coercion/graphlib 子进程、largefile/threadsignals
卡 CM-8 信号、test_list 环境、test_pdb ast）；small gate OK、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9409｜
round 179（行为变更轮）：**帧 f_trace/f_trace_lines/f_trace_opcodes 可写且默认值照参照**（f_trace=None、f_trace_lines=True、
f_trace_opcodes=False）——frame 类型挂外部实例字典（mark_external_instance_dict，GC children_of 已算边）、
frame_getattr 先查挂载字典；test_frame 10F/8E⇒10F/7E（test_lineno_with_tracing 转绿）；如实登记：追踪事件驱动
（f_trace 可调用时按行回调）仍未接线，下一轮；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9371｜
round 178（行为变更轮·回归修复+全量验收）：**补 round 176 大回归**——嵌套 Constant::Code 也拿真文件名 => inspect/linecache
走 ast.parse（只接签名面）=> 大量套件 unimplemented abort（survey 79/136/180/2）；git worktree 对照二进制定位到
round 176，再二分出嵌套文件名这一项；修法：嵌套单元默认回占位名（PYAWA_NESTED_FILENAMES=1 才开）、frame.__repr__
在占位名时退回全局 __file__（test_repr 仍绿）；全量 survey 回到 **79 ok/69 unimplemented/244 runtime/5 timeout**、
test_frame 10F/8E 收益保住、test_abc 回基线；分块十项全绿、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9339｜
round 177（行为变更轮·原型）：**FrameLocalsProxy 原型进仓**（写穿视图+构造器+映射面，约 600 行；读 localsplus/写穿帧/
影子字典；删局部 ValueError、未知 KeyError）；**默认仍关闭**（PYAWA_FRAME_LOCALS_PROXY=1），默认口径 test_frame
保持 10F/8E 零回归；开关下 16F/16E 未达标 ⇒ 下一轮先补缺口再跑 survey 决定是否翻默认；踩到并修掉真 bug：
`__iter__` 把 dict_entries 借用键直接交 new_list ⇒ 无主指针 ⇒ `in` SIGSEGV（补 retain）；small gate OK、
fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9299｜
round 176（行为变更轮）：**co_filename 与 co_firstlineno 与参照一致**——CLI 脚本/-c/导入模块的 co_filename 不再写死
<pyawa-test>（真实路径／<string>；instantiate 拆 instantiate_named + 嵌套 Constant::Code 递归传名 + ABI
pa_set_program_filename + CLI 设置）；CompiledUnit 加 firstlineno（模块 1、函数/类体 def 行），pyac 格式升版
（INSTRUCTION_SET_VERSION 2⇒3）；frame_repr 文件名改单引号；test_frame 12F/7E⇒10F/8E（test_repr 转绿；
test_lineno_with_tracing FAIL⇒ERROR 是撞上未接线的 f_trace，非退化）；分块十项全绿、fastcheck OK、check 16/16、
boundary_guard 2/2；`docs/rounds/04-post-m6-01.md`｜9261｜
round 175（工具轮）：新增 tools/bench_hot.py 解释器热点微基准（属性读/写、内建调用、空循环四档，pyawa 脚本自身跑；
本机 baseline 6.409s / attribute_read 8.269s / attribute_write 4.807s / builtin_call 10.223s => 属性读约 baseline+0.93µs/次）；
顺手去掉属性读路径每次一个 String 分配（type_name(...)=="dict" => type_named+类型指针比较，效果在噪声内 0.2%，如实记）；
small gate OK、fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜9223｜
round 174（行为变更轮+全量验收）：**全量 survey 79 ok/69 unimplemented/244 runtime/5 timeout**（ok 78⇒79、timeout 仍 5）；
**call_value 内建可调用快路**（kwargs 空且 builtin_function_or_method 时直接调 NativeFn，省两次 Vec 分配＋逐实参
incref/decref）：2e6 次 len([1]) 循环 13.956 s⇒13.222 s（≈5%）；编译器 NFKC 改动后全量重测无回归；
test_dict/descrtut/userstring/genexps/compare/richcmp OK；test_builtin 直跑为基线（named-import 才 OK）；
fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜9194｜
round 173（行为变更轮）：**PEP 3131 跨字符组合面**——标识符 NFKC 增加两码点合成表（908 条生成数据，由 unicodedata
decomposition 取"无 tag 两码点、第二码点 ccc≠0、三码都合法"生成，按 (a,b) 排序供二分）；normalize_identifier 两步
（逐字符映射＋两码点贪心合成）⇒ `e`+组合记号 与 `é` 归一（`class T: cafe\\u0301=1` 后 getattr(T,'café')==1）；
如实登记不做规范重排；test_unicode_identifiers OK、test_keyword/genexps/dict OK、test_grammar 基线；
fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜9161｜
round 172（诊断轮）：钉住 round 171 撤回项为什么不能就地还——test_dict.test_constructor 失败是陈旧内容
（{} vs {'foo':'bar'}，来自前面 test_views_mapping 的堆复用）；实测该分支全部 rc=2（还一份后 rc=1 仍在槽里 ⇒
不是双放）⇒ 病灶在"槽里那份最终被释放时"（异常→回溯→帧链的所有权）；加只读诊断口 PYAWA_ITER_ERR_DEBUG；
下一轮评估"探针异常不挂回溯"可落性；test_dict 3/3 稳定、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9131｜
round 171（行为变更轮）：iterable_items 的协议探测不再丢异常（.is_ok() => 显式 discard_attribute_error；test_dict 5/5 稳定）；
并**如实撤回** iter_value 的同类修（三种修法都让 test_dict.test_constructor 间歇失败 1-2/5；撤回后 8/8 稳定 ⇒
症状是"该路径上任何干预都会改变 dict(mapping) 语义"，疑 pending 槽被清后 __getattr__ 回退走不同分支；
最小探针 f(*Plain()) x 2 万 ⇒ AttributeError/TypeError 根各 20000 仍在）；fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜9095｜
round 170（行为变更轮）：**`__getattr__` 回退路径真泄漏修**——属性查找失败但回退成功时原始 `AttributeError` 的
两份引用（Err+pending 槽）没还 ⇒ 每走一次 __getattr__ 漏一颗；最小复现 _WritelnDecorator.writeln x2 万：
AttributeError 根 **40076⇒0**；test_weakref 四连 skip 同类根 3957⇒216（还剩 TestLoader.loadTestsFromName@71/
_compile@178/_isnotsuite@8/EnumType.__new__@294/WeakValueDictionary.update@43 等同族路径，下一轮同法钉）；
live 仍 1.48 M、全档 171 s；回归 test_dict/test_descrtut/test_userstring/test_genexps/optparse OK、metaclass 基线；
fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜9053｜
round 169（行为变更轮·泄漏硬化）：迭代协议里吞异常的分支统一走 discard_raised（iter_advance 的 StopIteration/IndexError、
step_iterators 的 END_ASYNC_FOR/CLEANUP_THROW；连 pending_exception 槽那份一起还，一处真相）；**证否**它是
test_weakref 1.48 M 残留的根因（改后 live 仍 1 486 380/freed=0）；根因是 3 957 个 AttributeError（rc=1 ext=1、无候选父
⇒ 图外持有者，pending 槽已排除）⇒ 下一轮逐个排除 Instance 非 GC 字段/宿主侧；回归 test_genexps OK、test_iter/test_raise/
test_exceptions baseline；fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜9011｜
round 168（行为变更轮）：**test_unicode_identifiers 整档转绿**（1F/2E⇒OK）——① SyntaxError 位置面（filename/lineno/offset
＋参照文案 invalid character '€' (U+20AC) (badsyntax_3131.py, line 2)）② PEP 3131 标识符 NFKC 归一化（生成的 3641 条表：
µ⇒μ、𝔘𝔫𝔦𝔠𝔬𝔡𝔢⇒Unicode；逐字符近似，跨字符组合登记缺口）；Lib/test/tokenizedata 三个故意坏样本登记进 lib_compile KNOWN；
fastcheck OK、lib_compile 通过、分块十项全绿；预计 ok 78⇒79；`docs/rounds/04-post-m6-01.md`｜8971｜
round 167（验收轮）：**全量 survey 复测** 78 ok / 69 unimplemented / 245 runtime / **5 timeout**（ok 77⇒78、timeout 7⇒5；
超时只剩 decimal/ntpath/socket/statistics/weakref）；补 `Lib/test/tokenizedata`（上游测试数据包，生成数据）⇒
`from test import tokenizedata` 可导入、test_unicode_identifiers 的 2 个 ImportError 消掉；
下一步：SyntaxError 属性面（msg/filename/lineno/offset/text）、PEP 3131 NFKC 标识符归一化（需生成表）、
weakref 被吞 AttributeError +1 泄漏；small gate OK、fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜8933｜
round 166（行为变更轮）：**`dir(set)`／`dir(frozenset())` 与 CPython 逐名一致**（新增两张参照表面表 57/44 个名字，
逐名用本机 python3 的 dir 生成；set 方法都在 getattr 槽里、先前 dir(set()) 只有 union 26 个名字）；
`'clear' in dir(set())` ✓、`'add' in dir(frozenset())==False` ✓；test_rlcompleter OK、test_set 72F/166E、test_dict OK；
fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜8898｜
round 165（工具/诊断轮）：两个新诊断口 `PYAWA_GC_REVERSE`（反向边＋rc/ext/位点）与 `PYAWA_EXC_SITE`（异常分配位点）；
钉住 test_weakref 残留真形——**每颗被吞掉的 AttributeError 泄漏 +1 引用**（rc=1 ext=1 parents=<非候选持有者>），
其 traceback 链着测试帧 ⇒ 一颗拖一整片；对照（hasattr/getattr/try-except 各 2 万次）都不泄漏 ⇒ 泄漏在引擎内部
某条吞异常路径；fastcheck OK、check 16/16；`docs/rounds/04-post-m6-01.md`｜8868｜
round 164（行为变更轮）：**`set.clear()` 真 bug 修**（_weakrefset.py 的 clear 是 self.data.clear() => 本层 set 缺 clear =>
`weakref.WeakSet().clear()`/`threading._dangling.clear()` 报 `'set' object has no attribute 'clear'`）；test_set **171E⇒166E**；
另把 `[gc-marked]` 链增强（frame 打 code 名、覆盖 DummyKey）⇒ 钉住 weakref 残留持有链：根帧＝
check_threaded_weak_dict_copy，经 keys 列表与 **AttributeError 的 traceback 链**（WeakValueDictionary.update 帧）可达；
test_weakref 全档仍 169 s；test_dict OK、fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8830｜
round 163（工具/诊断轮）：`PYAWA_GC_ROOTS_DEBUG=2` 根集合按类型汇总模式（大图逐条打印会刷几十万行）；用它钉住
test_weakref 四连 skip 后 1.48 M live 的可达路径嫌疑——根只 5288 个（method=1901/dict=1398/generator=27…）、
DummyKey 本身不是根 ⇒ 被某根经闭包/方法链持有；直方图 DummyKey/DummyValue/KeyedRef 各 140k（两例载荷）；
下一轮核 threading 对象生命周期（_limbo/_active/Thread._target 没清）；fastcheck OK、check 16/16；
`docs/rounds/04-post-m6-01.md`｜8792｜
round 162（行为变更轮）：**推导式内嵌"引用推导变量"的生成器表达式真正接线**（隐藏形参法：生成器单元多收同名形参、
调用点在 .0 后同序压值 ⇒ 不需要内联推导式 cell 化）⇒ `[x for x in xs if any(str(x)==y for y in ['1','3'])]`
给 `[1,3]`（module/函数都对）；中途"analyze_cells 不提升推导目标"启发式过宽（functools.base 同名）已撤回；
test_genexps OK、test_dict/test_userstring/test_descrtut OK、test_iter baseline；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8755｜
round 161（行为变更轮）：推导式内嵌"引用推导变量"的生成器表达式——**进程级 panic ⇒ 明确编译错误**（不再崩）；
真因：module 作用域不跑 `analyze_cells`（PYAWA_CELLS_DEBUG 无 module 行）、函数作用域 cell 化后内联推导式
撞 `RefCell 借用冲突`（cell.rs:47）；判据与合法形态回归进台账；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8712｜
round 160（工具/诊断轮）：`tools/test_durations.py --test`（按 id 子串筛选用例，能在类内上下文里单测；实测 len_race
单跑 0.61 s vs 类内 20-22 s）；顺带钉出编译器真 bug——推导式里嵌生成器表达式引用推导变量 ⇒ 编译期 panic
（生成器表达式的自由变量必在本层 cell/free 表里）或解析错（实参表里出现 Some(RightBracket)），最小复现进台账；
`request_full_collect` 延迟到 pending 清空的实验无收益已撤回；fastcheck OK；`docs/rounds/04-post-m6-01.md`｜8673｜
round 159（行为变更轮）：**控制流异常 `SkipTest` 不挂 `__traceback__`**（如实偏差）——线程化用例跳过后 live
**645 783⇒85 810**、`test_weakref` 全档 **285 s⇒173 s**（−40%）；定位链：PYAWA_NO_TRACEBACK_ATTACH 下同序列 live
回 79 124 ⇒ 钉住的是 traceback→tb_frame→帧局部（70k）；根帧诊断见整条调用链（Thread.start/TestCase.run…）；
对照实验（普通/native/contextmanager 吞 SkipTest）都不涨 ⇒ 不是单点泄漏；另留 PYAWA_EXC_STATE_DEBUG/
PYAWA_GC_ROOT_FRAMES 静默诊断口；test_raise/test_traceback 与 baseline 一致；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8632｜
round 158（诊断轮）：`request_full_collect` 立刻生效（不等 live 级阈值）；钉住 test_weakref 剩余 slow 真因——
四万多个异常对象的 **traceback 帧链**留着被测函数局部（DummyKey/DummyValue/弱字典），全量回收仍当可达（freed=0）；
根类型分布显示 AttributeError=1102 等异常长期保活（怀疑 pending_exception/exception_state 栈）；test_weakref 全档
仍 285 s（不挂住）；test_dict/test_userlist/test_userstring/test_descrtut 全绿；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8591｜
round 157（行为变更轮）：**GC 分代（年轻段）**——`gc.collect(0)`/`collect(1)` 与自动回收只收"上次回收以来的新对象"、
每 16 次自动回收一次全量；边界必须在清扫后取（清扫前取会上悬垂 ⇒ 整链当年轻段 378k ✗）⇒ 同场景 GC 总耗时
25.0 s⇒6.7 s、慢回收 103⇒18 次、len_race 135 s⇒34 s；线程创建缺位改抛 `unittest.SkipTest`（8 个线程化用例记 s，
不再钉 70k traceback 帧）＋ `request_full_collect`；test_weakref 250 s 超时⇒282 s 跑完（仍 >120 s）；
test_dict/test_userlist/test_userstring/test_descrtut 全绿；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8550｜
round 156（行为变更轮）：**墓碑删除第三次落地成功**——`DictObject` 删除 O(n) 搬移 ⇒ **O(1) 墓碑 ＋ 索引增量摘除**；
`WeakKeyDictionary` 20k 次删除 **17.2 s ⇒ 0.5 s**（~35×）、回调删除 15.4 s ⇒ 0.27 s；前两轮"丢项"真凶是
`Instance::dict_set(replace_value)` 等 ~20 处拿过滤下标当原始位置（全仓改 `find_live_position`）＋ `dict.clear`
提前收手（改 `next_live_entry`／`last_live_entry`）；线程同步仿真死锁 ⇒ `_thread` 创建改回如实未实现；
test_dict/test_userlist/test_userstring/test_descrtut 全绿、conformance 5/5 回绿、分块十项全绿；
`test_weakref.MappingTestCase` 仍 >200 s（traceback 钉 70k ⇒ 99 次全量 GC × 2.6M live）；
`docs/rounds/04-post-m6-01.md`｜8510｜
round 155（诊断轮·再次撤回＋更窄阻塞点）：墓碑删除第二版成套口径（ensure_index 跳墓碑＋find_live_position 14 站＋
迭代器跳墓碑）后只剩一个 bug——类体最后一条语句是 `@x.setter` 时该项不进类命名空间（加任意后续语句即恢复）；
钉到最窄（命名空间条目里就没有该项 ⇒ 不是类型字典搬运）⇒ 下一轮核 `STORE_NAME` 覆盖分支的引用计数／
`property.setter` 返回口径；本轮代码树再次全撤回；`docs/rounds/04-post-m6-01.md`｜8461｜
round 154（诊断轮·撤回＋改变计划）：钉住 `test_weakref` 的 O(n²) 根因——`DictObject::remove` 的 `Vec::remove`
搬移（20k 次 pop 实测 0.84 ms/次，随规模线性 ⇒ 70k 次删除 O(n²)）；墓碑删除原型做完（含 `ensure_index` 跳过墓碑、
14 处位置口径 `find_live_position`）但时间盒不够验证完 ⇒ **全撤回**（树＝HEAD）；下一轮按清单一次改齐再落；
`docs/rounds/04-post-m6-01.md`｜8414｜
round 153（行为变更轮·回归修）：**类型对象不走序列协议**——round 152 的 `list.__getitem__` 未绑定描述符被
`iter()` 的序列回退误用 ⇒ `list(list)` 死循环（参照 TypeError: 'type' object is not iterable）⇒ `iter.rs` 加
`!is_type_object(iterable)` 守卫；`test_iter` **150 s timeout ⇒ 3 s**（9F/12E 基线）、test_descrtut 保持 OK；
test_io 仍 timeout（单格最慢 3.2 s，属用例多累积）；fastcheck OK、分块十项全绿；推算 77/69/245/6；
`docs/rounds/04-post-m6-01.md`｜8376｜
round 152（行为变更轮）：**test_descrtut 整档转绿**（tut3）——`LIST_TYPE_DIR_NAMES`（`dir(list)` 参照 48 名字）＋
注册 `list` 类型上的未绑定方法描述符（`list.__len__(a)`/`list.append(a,x)`，`descriptor:` 前缀路径）⇒ 76⇒77 ok；
threading 单线程仿真后时间结构变化：test_threading unimplemented⇒runtime、test_iter/test_io runtime⇒timeout
（同步仿真让线程体真跑）；全量 survey **77/69/244/7**；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8338｜
round 151（行为变更轮）：**`_thread` 单线程同步仿真**（`start_new_thread`/`start_joinable_thread` 直接跑目标）、
`_ThreadHandle` 的 `is_done`/`join`/`ident`、`_contextvars.Context.run` 接线 ⇒ `threading.Thread` 的
start/join/is_alive 真正可用；顺带提取 `list_method_handler` ＋ `call_callable` 的 `descriptor:` 未绑定描述符
（tut3 工具就绪，下一轮接面）；`test_weakref.MappingTestCase` 仍 >240 s（现在是真的在执行 70k 拷贝/pop）；
fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜8305｜
round 150（行为变更轮）：**`!=` 的 `object.__ne__` 默认委托**（自己无 `__ne__` 但有 `__eq__` ⇒ 委托自己的
`__eq__` 再取反）⇒ `test_compare` 整档转绿；**`__slots__` 私有名双形态兼容**（原文名 `'__num'` 与改写名
`'_Rat__num'` 都认）⇒ 修 round 147 隐藏回归、`test_binop` 回 OK；全量 survey **76/70/246/5**（ok 75⇒76）；
fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜8266｜
round 149（诊断/工具轮）：`tools/test_durations.py --stream`（逐条即时打印）＋在 `test_weakref` 上钉住 live 2.6 M 的来源——
**线程化 copy 用例报 NotImplementedError 后的 traceback 把 70k 条目的局部变量钉住**（另否证 WeakKeyDictionary 本身 O(n²)：
插入 2k→4k 线性）；下一轮把 `_thread.start_new_thread` 做同步仿真或改报错路径；fastcheck OK；
`docs/rounds/04-post-m6-01.md`｜8233｜
round 148（行为变更轮·GC 代语义）：**`gc.collect(generation)` 按代区分**——`collect(0)`/`collect(1)` 跳过孤儿类型回收
（`collect()` 默认全量仍做）；`test_weakref.test_weak_keyed_len_race` 从类内超时降到 **6 s**；但 MappingTestCase
整类仍 timeout（GC_LOG 钉住：类内 live 2.6 M、每次全量 GC 1.1-1.3 s）⇒ 下一轮动全量标记成本；`free_after` 家族
（test_dict/test_userlist）保持 OK；fastcheck OK、分块十项全绿；`docs/rounds/04-post-m6-01.md`｜8199｜
round 147（行为变更轮）：**`__slots__` 强制**（写非槽名报参照 `AttributeError`）⇒ `test_descrtut` tut2 转绿
（**2F⇒1F**）；实现级三修：引导期早退（空槽表直接放行，避免 stdlib 安装期段错误）、只对用户类实例生效
（避免 attach_traceback 无限递归）、MRO 口径（自身槽命中／用户基类无槽表授予字典／内建基类不授予）；
导入面 datetime/typing/email/weakref/calendar/dataclasses ok；fastcheck OK、分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8163｜
round 146（行为变更轮·迭代源热路径）：**被迭代源类型→布局种类缓存**（`iterable_length`/`iterable_item` 每元素
不再做 7 次子类型判定）——for-pass 300k **0.685⇒0.674 s**、`len()` 100k **0.416⇒0.364 s**（−12%）；首版 `thread_local!`
被 CX-3（禁全局可变状态）拦下 ⇒ 改实例字段缓存后 `check.py` 14/14 PASS 且性能保持；容器面回归与基线一致；
分块十项全绿；`docs/rounds/04-post-m6-01.md`｜8122｜
round 145（行为变更轮·迭代器热路径）：**专用推进分支链提取＋按代数缓存的专用类型集合＋`range` 迭代器快路径**
——`advance_iterator` 不再对普通迭代器逐个比 20+ 内建名；for-pass 300k **0.711⇒0.685 s**、while 300k
**1.187⇒1.058 s**、`test_userstring` **90⇒87-89 s**（累计 101⇒87，−14%）；迭代器面回归（test_iter 9F/12E、
test_list 0F/1E、test_dict OK、test_itertools named rc=0）与基线一致；分块十项全绿；
`docs/rounds/04-post-m6-01.md`｜8092｜
round 144（行为变更轮·再探热路径）：**类型名哈希索引**（再落地 round 142 补丁）——本轮用迭代器微基准（每迭代
`type_named` 查惰性/缺失名 ⇒ 跑满表）测到收益：for-pass 300k **0.775⇒0.711 s**、普通调用 **0.477⇒0.447 s**、
`test_userstring` **92⇒90-92 s**；机理：索引对「缺名/靠后名」收益明显、对「靠前内置名」因早停无收益（两轮不矛盾）；
分块十项全绿；`docs/rounds/04-post-m6-01.md`｜8052｜
round 143（行为变更轮·解释器热路径）：**指令表 O(1) 化**（`opname`/`inline_cache_entries` 建 256 项直接表；
`op>=256` 冷查询退回二分）＋ **`EXTENDED_ARG` 编号缓存**（解码器每条指令不再字符串二分）＋ **`CALL` 分派臂前置**
⇒ 普通调用 100k **0.532⇒0.477 s**、方法 **0.701⇒0.646 s**、`test_userstring` **101⇒92-94 s**（并行 survey 回 ok）；
GC 嫌疑同步排除（NO_GC 仅 −4%）；分块十项全绿；全量 survey **75/70/247/5**（ok 74⇒75）；
`docs/rounds/04-post-m6-01.md`｜8017｜
round 142（诊断轮·撤回+事实）：两版降调用开销试改**都无效**并撤回——① `type_named` 改 HashMap 索引：3000 类型
后 20 万次调用 2.117 s vs 线性 2.146 s（无收益）② 热指令编号直派（跳过 opname+字符串 match）：0.581 vs 0.531 s、
userstring 104 vs 99 s（更慢）。新事实：5 µs/次开销**不在**名字查找/逐指令分派；下一轮先验证「分配路径 GC 占比」
（NO_GC/大阈值对比）再决定修 GC 触发或转功能缺口；fastcheck OK（44 s）；`docs/rounds/04-post-m6-01.md`｜7979｜
round 141（诊断/工具轮）：**逐用例计时工具 `tools/test_durations.py`** ＋ `test_userstring` 热点分解——99.7 s 里
91 s 压在 5 个「双重大循环」用例（periodic 30.4 s、rindex/index 各 20 s、rfind/find/count 8-10 s）；根因是
解释器调用开销（普通调用 100k 次 0.534 s vs CPython 0.004 s ⇒ ≈5 µs/次）；LTO/codegen-units 试改仅 2-4% ⇒ 撤回；
fastcheck OK；`docs/rounds/04-post-m6-01.md`｜7942｜
round 140（验收/工具轮）：**全量 survey 分片跑通并合并** —— 新增 `tools/libtests_survey_merge.py`（分片 tsv 合并＋
--extra 补单文件＋四项统计）；本轮 **74/70/247/6**（并行档）：`test_richcmp` 新进 ok、`test_userstring` 在 jobs9
负载下越 120 s 线（单独跑 ok）、unimplemented 74⇒70 属分类前进；environ 键编码缓存试改（预估 3×）实测仅 ~10%
⇒ 撤回；`docs/rounds/04-post-m6-01.md`｜7906｜
round 139（行为变更轮）：**test_richcmp 整档转绿**（11 tests OK）——① 列表相等**先比长度**（参照
`list_richcompare`；修好递归容器的 RecursionError 逃逸）② 富比较改**三态**（Value/Fallback/None，不再重复派发
dunder；修 test_binop 回归）③ `!=` 富路径按参照次序只在自己 `__ne__` 会先命中时启用；回归面 test_binop/test_dict/
test_userlist/test_list/test_set 均基线；fastcheck OK、分块十项全绿；ok 75⇒76（推算 76/70/246/5）；
`docs/rounds/04-post-m6-01.md`｜7878｜
round 138（行为变更轮）：**富比较结果语义**——`<`/`<=`/`>`/`>=`/`==`/`!=` 对用户类返回 dunder 的结果对象
（`order_compare_result`/`equality_compare_result`，COMPARE_OP 与 operator.* 同口径）⇒ `test_richcmp` 错误 2⇒1
（`test_mixed` 转绿）；试改「所有类型都走富等值」曾让 test_dict 段错误 ⇒ 收紧到 `is_user_type`；
`test_dict` OK、`test_userlist` OK、`test_set` 基线；fastcheck OK、分块十项全绿；推算 75/70/247/5；
`docs/rounds/04-post-m6-01.md`｜7842｜
round 137（提速轮）：**`re.Match.__getitem__(int)` 快路径**（`m[1]` 20k 次 0.12 s；`Lib/ntpath.expandvars` 的
replacement 每个匹配都取 `m[lastindex]`）；热点分解：`expandvars` 真正大头是 `os.environ[name]` ≈31 µs/次
（encodekey≈16、`_data` 查找≈7）；`test_ntpath` 直跑仍 >200 s（timeout 口径）；fastcheck OK + small/conformance 绿
（round 136 已跑全十项）；`docs/rounds/04-post-m6-01.md`｜7806｜
round 136（行为变更轮）：**`str` 子类的下标走父类取码点**（`class my_unicode(str)` 取下标不再落「下标只接线了…」）
⇒ `test_re` 从首格 abort 变为整档跑起来（unimplemented⇒runtime，41F/31E）；survey 驱动子进程捕获改临时文件
（孙进程继承管道 ⇒ 超时杀父后 communicate 等 EOF 挂死驱动）；定位 `test_ntpath` 是**慢**非死循环（expandvars 100k
单次 ~13 s）；fastcheck OK、分块十项全绿；推算 75/70/247/5；`docs/rounds/04-post-m6-01.md`｜7772｜
round 135（行为变更轮）：**用户自定义迭代器「协议中 `__next__` 消失」改报参照 `TypeError`**（用户类才升级、
内建迭代器保持 Unsupported 以护 test_itertools）⇒ `test_iter` 从首格 abort 变为整档跑起来（unimplemented⇒runtime，
9F/12E in 2 s）；test_itertools named harness 仍 ok（无回归）；fastcheck OK、分块十项全绿；
survey 重跑撞上限未满，推算 75/71/246/5；`docs/rounds/04-post-m6-01.md`｜7740｜
round 134（行为变更轮）：**`math.fsum`／`math.sumprod` 改走迭代器协议**（`fsum(生成器)`／`sumprod(生成器,生成器)`
可用）＋ 修掉 `sumprod` 的**迭代器双放**（这就是 round 133 的 malloc 堆损坏根因）；`statistics.correlation`／
`covariance` 全通；`test_statistics` 不再崩溃/abort（>400 s ⇒ 将记 timeout）、`test_math` 13 s 跑完 35F/36E
（分类前进）；fastcheck OK；分块整轮十项全绿；推算 75/72/245/5；`docs/rounds/04-post-m6-01.md`｜7704｜
round 133（行为变更轮）：**`sum_native` 的 UAF 修**——混类型求和失败时先 `release(item)` 再用它拼 `TypeError`
消息 ⇒ `test_statistics` SIGSEGV；改为先读后放 ⇒ 崩溃消失、该套件 runtime⇒unimplemented（分类前进）；
`math.fsum`/`sumprod` 改迭代器协议的试改因 malloc 堆损坏撤回（下一轮定位）；fastcheck OK（55 s）；
分块整轮十项全绿；推算 survey 75/75/243/4；`docs/rounds/04-post-m6-01.md`｜7663｜
round 132（工具/流程轮）：**`gate_chunk.sh` 分块闸门 ＋ survey `--resume` 可续跑**——本会话单命令 ~600 s 上限下
恢复「全十项＋全量 survey」（四块全绿：small/stability/heap/conformance；survey 全量 75/74/244/4，ok 74⇒75；
unimplemented 80⇒74 来自 float(TypeError) 修复把 6 个 abort 变成正常失败）；`docs/rounds/04-post-m6-01.md`｜7629｜
round 131（行为变更轮）：**`float(bytes)` 接线 ＋ 未支持实参改报参照 `TypeError`**（`float(None)` 不再
「未接线」abort 套件）＋ **`re.Match.__getitem__`**（`m[g]` 与 `m.group(g)` 同口径）；fastcheck OK（57 s）；
整轮验收因本会话 ~600 s 单命令上限只能**分块**跑：十项中 9/10 绿（stability 3×997 一致、heap 全绿、
conformance 两档 5/5、夹具/语料/check/compile 全绿），survey 留下一轮（先做可续跑 survey）；
连带发现 test_statistics 新暴露 SIGSEGV；`docs/rounds/04-post-m6-01.md`｜7596｜
round 130（行为变更轮·健壮化）：**`retired_types` 登记**——回收后照旧**摘注册表**（名字表不留僵尸类）但
`assert_live` 认得本体 ⇒ 迟来的计数引用释放不悬垂；同名类重定义/内建名遮蔽场景实测正常；`test_userlist` 保持 OK、
`test_dict` OK／`test_list` 0F/1E／`test_rlcompleter` OK；套件 74/80/239/4；十项全绿；
`docs/rounds/04-post-m6-01.md`｜7548｜
round 129（行为变更轮）：**类型回收改为「保留注册表 + 只清字典」**（不摘 types/user_types，类型指针永远有效；
摘注册表会让仍在别处的计数引用将来释放时被当野指针 ⇒ round 124/128 的悬垂根因）＋在「只在显式回收时做」之上
放宽孤儿 `rc==1` 门槛 ⇒ **test_userlist 整档转绿**（`test_free_after_iterating` ✓），套件 **73⇒74**（74/80/239/4）；
test_dict OK／test_list 0F/1E／test_rlcompleter OK；风险：僵尸类名字冲突待查；十项全绿；
`docs/rounds/04-post-m6-01.md`｜7515｜
round 128（行为变更轮）：**孤儿回收只在显式回收时做**（自动回收不再跑类型扫描/清类字典；round 110 的节流
曾在撤回中丢失，本轮重新落地并进一步收窄）——收窄时机，导入期与重负载
不再被类型扫描拖慢、避免半成品期误判；`free_after` 判据都显式 `gc.collect()` ✓；第三次放宽 `rc==1` 在导入链
仍红（`EnumCheck`/`RegexFlag` 误收）⇒ `rc==1` 门槛恢复；`test_dict` OK／`test_list` 0F/1E／`test_rlcompleter` OK；
十项全绿；`docs/rounds/04-post-m6-01.md`｜7473｜
round 127（行为变更轮·基础件）：**模块的命名空间 dict 也直接标根**——模块用外置实例字典，
`AttributeObject::traverse` 只走内联字典 ⇒ 只标模块对象不够（`re.RegexFlag`／`signal.Signals` 等 enum 活类
仍被误收）；`module_roots()` 现在连 `instance_attributes(module)` 一起 push；放宽 `rc==1` 仍红再撤回；
`test_dict` OK／`test_list` 0F/1E／`test_rlcompleter` OK；十项全绿；`docs/rounds/04-post-m6-01.md`｜7438｜
round 126（行为变更轮·基础件）：**`sys.modules` 模块对象标根**（module_roots 并入两条可达性路径的根集合）——
定位到放宽 `rc==1` 时被误收的是**模块里的活类**（`_SubParsersAction` rc=5 等 60 个 argparse/unittest 类）；
模块根+栈根下 `test_dict` OK／`test_list` 0F/1E／`test_rlcompleter` OK，放宽试改再撤回（模块根不足以覆盖
全部活类来源，下一轮继续点名）；十项全绿；`docs/rounds/04-post-m6-01.md`｜7403｜
round 125（行为变更轮·基础件）：**解释器栈根**——`interpreter_stack_roots()`（current_frame 沿 f_back 链）
并入两条可达性路径的根集合（参照口径：栈上帧恒为根）；`PYAWA_ORPHAN_DEBUG` 加 `[orphan-reclaim]` 类型名；
第二次放宽 `rc==1`（栈根＋live 实例安全门、再加「以本类型为元类的类对象」）仍全红 ⇒ 两版再撤回；新事实：
被误收的不是「有实例/类实例」的类型 ⇒ 动态可达性在某些时刻把活类判不可达，下一轮用 reclaim 名字点名；
`test_dict` OK／`test_list` 0F/1E／`test_rlcompleter` OK；十项全绿；`docs/rounds/04-post-m6-01.md`｜7363｜
round 124（撤回＋事实）：放宽孤儿 `rc==1` 门槛——A 版（登记表安全门）让 **test_userlist 整档转绿** 但
test_rlcompleter 帧槽悬垂；B 版（live 全表枚举实例）更糟（连 test_dict 都悬垂）⇒ 两版全撤回；事实：`rc==1`
门槛是当前挡住「动态可达性漏根」的唯一保险 —— 下一轮先补根边（当前帧链/帧栈标根）再重试放宽；
`docs/rounds/04-post-m6-01.md`｜7323｜
round 123（行为变更轮）：**`build_class_native` 自定义元类路径漏还类体命名空间**（真 leak）——`M.__new__` 分支
`return Ok(result)` 不归还本函数那份、错误路径 `?` 也直接冒泡 ⇒ 每次自定义元类建类漏 1 份类体命名空间
（正是 round 121/122 的「root 类字典 rc=1 无持有者」）；修后最小骨架 `free_after` 由 False⇒True；
`test_dict` OK／`test_list` 0F/1E 不漂移、`test_super` 与基线一致；官方 helper 版仍差一环（见台账）；
十项全绿；`docs/rounds/04-post-m6-01.md`｜7284｜
round 122（诊断轮）：同一次回收内对比 `owner_type_dict`（被判可达实例的类型 dict）与 root：
`owner_type_dict=0x…3fc7a0`、`root=0x…4748e0` ⇒ **same=false** —— root 是**另一个同形状的类字典**
（`__module__/__qualname__/__firstlineno__/__del__/__static_attributes__`），rc=1 且 live 无持有者 ⇒
「上一代/旁路泄漏的类字典」+ 记账多留 1；下一轮用 `PYAWA_SETDICT_DEBUG`＋root 地址同跑追 set_dict/detach
路径（建类交账 vs 回收 detach 的配对/旁表持有）⇒ 再修；套件基线 73/80/240/4；
`docs/rounds/04-post-m6-01.md`｜7243｜
round 121（诊断轮）：钉「root 类字典」形状——`root_keys=[__module__,__qualname__,__firstlineno__,__del__,
__static_attributes__]`（典型类字典）、`external[A]=0 rc=1`，但根字典 `rc=1` 且 **gc 候选＋live 全表都没有
任何持有者** ⇒ 不是「候选边少扣」而是**类字典引用记账多 1**；`PYAWA_MARK_DEBUG` 扩根键/live 反查、
`Instance::live_addresses()`；下一轮审计 `set_dict`/建类交账与回收摘除配对 ⇒ 收 `test_userlist.free_after`；
`docs/rounds/04-post-m6-01.md`｜7207｜
round 120（行为变更轮）：**类命名空间不再挂 `__classdictcell__`**（照参照 `type_new` 的 pop 口径）——断掉
`类字典 ↔ cell` 自环（`compile.rs` 产物把 cell 挂进命名空间、cell 载荷就是命名空间本身）；
`C.__dict__` 里不再可见；`test_metaclass` OK、`test_super`/`test_descr` 与基线一致；`free_after` 仍差一环
（root 变成 `dict rc=1 referrers=[]`，下一轮钉扣减/候选集）；套件 73/80/240/4；十项全绿；
`docs/rounds/04-post-m6-01.md`｜7166｜
round 119（行为变更轮·基础件）：**生成器挂起断 `f_back`**（真 leak：`class A(UserList)` 的 A rc 2⇒1，
referrer 正是 `frame(wrapper)`）＋**用户类实例登记表**（`user_type_set`/`user_instances`，`adopt` 登记、
`release_one`/`free_garbage` 注销；孤儿回收按登记表枚举实例、无法证明不可达则保守不回收——规避 round 118 的
过度回收）＋引用者反查诊断（`attr_unreach`/`inst_ok`/`[orphan-referrers]`，默认零输出）；
`test_userlist.free_after` 仍未绿（A 已 rc=1，但实例经 `ty` 边把它标回可达，下一轮用登记表钉实例可达性）；
`test_dict` OK／`test_list` 0F/1E；十项全绿；`docs/rounds/04-post-m6-01.md`｜7123｜
round 118（撤回＋事实）：放松孤儿回收判据（去 `rc==1` 门槛/早退）会**过度回收活类**——实例不在 `gc_headers` 时
「全部活实例不可达」判据空过 ⇒ `test_rlcompleter` 帧槽悬垂（`PYAWA_DANGLING=1` 实测）⇒ 全量撤回；教训：放宽前
必须先有「活实例索引」（类型旁计数或全用户类实例入 `GC_TRACKED`）；下一轮先钉「类多余 1 份引用」来源再重试；
`docs/rounds/04-post-m6-01.md`｜7077｜
round 117（诊断轮）：定位 **`test_userlist.test_free_after_iterating` 环收不掉**的原因——`find_unreachable` 候选
集只有 `gc_headers`、**用户类型不在其中** ⇒ 类型字典的引用没被扣减、被当成外部根 ⇒ 穿过类型字典的环
（`__del__` 闭包 cell → `Sequence.__iter__` 生成器 → 实例）永远可达、终结器不跑；新增 `PYAWA_GC_ROOTS_DEBUG`
（根候选＋标记来源链＋不可达类型直方图，默认零开销）；下一轮把 user_types 并入释放路径可达性图（不 dealloc
类型本体）⇒ 收该用例；套件基线 73/80/239/5；`docs/rounds/04-post-m6-01.md`｜7035｜
round 116（行为变更轮）：**`callable()` 对元类非 `type` 的类对象误判 False**（pickle `PicklingError`）＋
**`__reduce_ex__` 协议 0/1 丢实例状态**（`(copyreg._reconstructor,(类,object,None),state)`）；
`test_userlist` 2F/1E ⇒ **1F/0E**（test_pickle 转绿）；剩 free_after（生成器-闭包-实例环的终结器时机）
归下一轮；套件 73/80/239/5；`docs/rounds/04-post-m6-01.md`｜6974｜
round 115（行为变更轮·小步）：**`list += 可迭代` 的反射口径补全**——右侧类型自带用户 `__radd__` 时走 `+`
的反射协议（CPython 3.14 实测：`list += UserList` ⇒ UserList、`list += R()`（R 有 `__radd__`）⇒ 反射结果、
无 `__radd__` 的可迭代 ⇒ 就地扩展）；`test_userlist` 2F⇒1F（`test_mixed_iadd` 全绿；剩 free_after/pickle）；
`test_list` 0F/1E 不漂移；套件 73/79/240/5；`docs/rounds/04-post-m6-01.md`｜6934｜
round 114（行为变更轮）：**用户覆写下标协议派发**——`__getitem__`／`__setitem__`／`__delitem__` 的落地类型非
内建容器时先调用户方法（并拆出 `*_inner` 给原生 `dict.__getitem__` 等绕过，避免递归）＋ `exec`／`eval` 往
globals 注入 `__builtins__` ＋ `list += 非 list/tuple` 回退反射协议（保留 `list.__iadd__` 直调语义）；
`test_descrtut` 3F⇒2F（tut1 全绿；剩 `__slots__`／`dir(list)` 面）；套件 73/79/240/5；
`docs/rounds/04-post-m6-01.md`｜6901｜
round 113（行为变更轮）：**`float('…')` 负号被吞真 bug 修**——解析前把 `-` 拆进 negative 却又对带 `-` 的
trimmed 解析、最后再取负 ⇒ `float('-1.0')`＝1.0（`-0.0`→0.0）；改为解析去符号 body；**`test_optparse`**
154 tests OK 整档转绿（nargs 负坐标用例）；另定位 test_descrtut 的三条独立缺口（dict 子类 `__getitem__` 派发、
exec 注入 `__builtins__`、`__slots__` 约束）留后续；套件 73/79/240/5；`docs/rounds/04-post-m6-01.md`｜6858｜
round 112（行为变更轮）：**`test_binop` 整档转绿**——`operator.eq`／`ne` 改走完整富比较协议（先前用
`values_equal_public` 快照、不派发 dunder）＋ `==`／大小比较的**子类优先级**（照 `do_richcompare`：右侧严格
子类先问，`__eq__ = None` 的阻断靠它触发）；`test_binop` 12/12 OK（修前 3F）、`test_compare` OK；
套件 72/79/241/5；`docs/rounds/04-post-m6-01.md`｜6819｜
round 111（行为变更轮）：**`test_keywordonlyarg` 整档转绿**——`__defaults__`／`__kwdefaults__` 做成 function
类型字典里的 getset 数据描述符（赋值/删除真正改函数字段 ⇒ 绑定实时生效；类型校验 tuple/dict/None）＋
def 形参表语法校验（裸 `*` 后必须有仅关键字、形参名不重复、`None/True/False` 不能当名）＋调用实参表
（位置实参不得跟在关键字之后）；套件 71/79/242/5（`test_keywordonlyarg` 新 ok）；`docs/rounds/04-post-m6-01.md`｜6785｜
round 110（行为变更轮·GC 提速）：孤儿用户类回收**节流**（新增 `gc_explicit`／`collect_explicit`：显式
`gc.collect()` 必跑、自动每 8 次；env `=1` 强制每次／`=0` 关闭）＋ 全堆标记优化（`is_type` 预建 HashSet、
`reclaim` 无 `rc==1` 候选早退、无终结器时省第二次 `find_unreachable`）；`test_dict`/`test_list` 绿保持，
`test_set` 152 s 无堆损坏（GC 非其主因）；十项全绿；`docs/rounds/04-post-m6-01.md`｜6744｜
round 109（行为变更轮）：**就地运算的用户 dunder 派发（PEP 203）**——`x op= y` 先调 `__iop__`（13 符号全覆盖，
`NotImplemented` 落回基运算）；**`test_augassign` 7/7 OK 转绿**；连带修 `dict.__ior__` 非 dict 右值走旧式
`__getitem__` 序列的死循环（统一走 `dict_update_native`）＋ `list.__iadd__` 收任意可迭代；`test_dict` OK、
`test_list` 0F/1E 保持；十项全绿（713 s）；套件 70/79/243/5（test_userstring 临界波动；两者皆绿时 71/78/245/4）；
`docs/rounds/04-post-m6-01.md`｜6702｜
round 108（行为变更轮）：
