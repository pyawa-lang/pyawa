# NEXT（会话 11 进行中 —— M6 后 P3 批次）

> **M6 已收口（2026-10-31）**：`MS-14` 407/407、已知差异 0、新差异 0；`MS-19` 补充随 `P3-38` surrogate epic 收口
> （`Lib/test` 语法错 0／可编译 397/397＝100%）；判据① 477/628＝76.0%；`Lib/` 逐字节同步 739 文件；
> `main` 已 `--no-ff` 收录 ＋ tag `m6`。**本战役（会话 11）只抬"官方套件能跑通的面"，不动 M6 的 gate ✓**。

- **HEAD**：`dev` 见 `git log -1`（round 121（诊断轮）：root 类字典 rc=1 且 gc 候选+live 全表都无持有者 ⇒ 类字典引用记账多 1（非候选边问题）；PYAWA_MARK_DEBUG 扩根键/live 反查；下一轮审计 set_dict/建类交账；round 120（行为变更轮）：类命名空间不再挂 __classdictcell__（照 type_new pop 口径，断类字典↔cell 自环）+ PYAWA_MARK_DEBUG 标记链诊断；test_metaclass OK、test_super/test_descr 基线一致；free_after 仍差一环（root=dict rc=1 referrers=[]）；十项全绿；round 119（行为变更轮·基础件）：生成器挂起断 f_back（类 rc 2⇒1，真 leak）+ 用户类实例登记表（孤儿回收可枚举活实例、保守不回收）+ 引用者反查诊断；test_userlist.free_after 仍未绿（实例经 ty 边标回可达）；十项全绿；round 118（撤回）：放松孤儿回收判据 => 过度回收活类（test_rlcompleter 帧槽悬垂）=> 全量撤回；教训：放宽前必须先有活实例索引；下一轮先钉类多余 1 份引用来源；round 117（诊断轮）：定位 test_userlist.free_after 环收不掉——find_unreachable 候选集不含用户类型 ⇒ 类型字典成外部根、穿过类型字典的环永可达；新增 PYAWA_GC_ROOTS_DEBUG（根/标记链/不可达直方图，默认零开销）；下一轮把 user_types 并入释放路径可达性图；套件 73/80/239/5；round 116（行为变更轮）：callable() 对 ABCMeta 类对象误判 False（pickle）＋ __reduce_ex__ 协议 0/1 丢状态 => test_userlist test_pickle 转绿（2F/1E⇒1F/0E）；套件 73/80/239/5；round 115（行为变更轮·小步）：list += 可迭代的反射口径补全（右侧带用户 __radd__ 时走 + 反射协议）⇒ test_userlist 2F⇒1F（mixed_iadd 绿）；套件 73/79/240/5；round 114（行为变更轮）：用户覆写下标协议派发（__getitem__/__setitem__/__delitem__，含 *_inner 绕过）+ exec/eval 注入 __builtins__ + list += 非 list/tuple 回退反射；test_descrtut 3F⇒2F；套件 73/79/240/5；round 113（行为变更轮）：修 float('…') 负号被吞真 bug（float('-1.0')=1.0）⇒ test_optparse 整档转绿；套件 73/79/240/5；round 112（行为变更轮）：test_binop 整档转绿——operator.eq/ne 走完整富比较协议 + 比较子类优先级（P3-42 派发）；套件 72/79/241/5；round 111（行为变更轮）：test_keywordonlyarg 整档转绿——__defaults__/__kwdefaults__ 可写描述符 + def/调用语法校验；套件 71/79/242/5；round 110（行为变更轮·GC 提速）：孤儿回收节流（显式 gc.collect 必跑、自动每 8 次）+ 全堆标记优化（HashSet/早退/省第二次 find）；test_dict/test_list 绿保持、test_set 152s 无堆损坏；十项全绿；round 109（行为变更轮）：就地运算用户 dunder 派发（PEP 203）⇒ test_augassign 7/7 OK 转绿；连带修 dict.__ior__ 非 dict 右值死循环 + list.__iadd__ 收任意可迭代；test_dict OK、test_list 0F/1E；套件 70/79/243/5（test_userstring 临界；两者绿时 71/78/245/4）；round 108（行为变更轮）：修 set.intersection() 无参拷贝漏 retain（集合元素无计数 ⇒ 非确定性堆损坏）⇒ test_set 三跑稳定完成、堆损坏消除；test_dict OK/test_list 0F/1E；套件空闲 70/78/245/4（test_set 仍 timeout=性能线）；round 107（诊断轮）：refcount 毒值 canary（PYAWA_REFCOUNT_CANARY=1）+ 确定性复现：test_set 的 set_clear 收已死 str；对照 round103 worktree 无悬垂、仅放 tuple 仍悬垂 ⇒ 触发 set 元素无计数潜伏 bug；下一轮从 set_insert 三路钉；套件 70/78/245/4；round 106（撤回）：test_set 堆损坏非确定性（同二进制两次结果不同；perturb 下正常）⇒ 二分只是相关性；下一轮先做确定性抓手（refcount 毒值 canary+DEAD_BT、perturb 矩阵、GC_LOG 直方图）再修；round 105（行为变更轮·小步）：修 type_mro_native 漏 retain（test_descr 的 type 双放根治）；新发现 test_set 在 round 104 融合存取旧值归还下堆损坏（旧值别名/重复所有者，下一轮主线）；套件空闲基线 70/78/245/4；round 104（行为变更轮）：用户类类型 GC 收尾——STORE_FAST_STORE_FAST 旧值归还+list.__setitem__ 借用值双放修+ORPHAN_RECLAIM 默认开 ⇒ test_dict 整档 OK、test_list 0F/1E（free_after ✓）；套件 70/78/245/4；新问题 test_set 变 timeout（回收扫描慢，下一轮节流）；scout 定位 type_mro_native 漏 retain；round 103（行为变更轮·小步）：修 STORE_FAST_LOAD_FAST 旧值泄漏 + 诊断 PYAWA_REF_TRACE_DEAD_BT；STORE_FAST_STORE_FAST 修复留 round 104（test_list 新悬垂=tuple 元素 alias）；计数口径纠偏：test_userstring 100 s 临界，稳定 68/78/247/4；roundcheck 改默认串行、survey --jobs 2；round 102（行为变更轮）：修 CALL_FUNCTION_EX 漏还 callable ⇒ 套件 68⇒69 ok、超时 4⇒3（test_userstring 转 ok）；scout 定位下一环 STORE_FAST_STORE_FAST 不归还旧值；round 101（提速基建，用户批准）：验收并行化——roundcheck 12m45s（-45%）、fastcheck 32s、slowcheck ③由⑤覆盖/⑨⑩并行/heap 扰动并行；判定口径不变；套件 68/78/247/4；round 100（续期后第 20 轮／目标轮次收尾）：诊断六——注册类型 -1 行带计数；类对象少一次 decref 只出现在 assertRaises 构造（模块级/函数内类对账都齐）；阶段总账 26⇒68 ok、未实现 246⇒78；套件 68/78/247/4；round 99（续期后第 19 轮）：诊断五——PYAWA_REF_TRACE_CLASS 按类名跟踪类型对象；assertRaises 构造下类对象 rc=2（少一次 decref，与 test_descr type 双放同源）；套件 68/78/247/4；round 98（续期后第 18 轮）：形参所有权翻默认（PYAWA_FRAME_OWN=0 可回退；每次调用漏 1 份实参旧账收尾）+ 孤儿用户类回收原型（PYAWA_ORPHAN_RECLAIM=1 默认关；简单夹具 done=True，assertRaises 构造下类对象多一份引用未收）；套件 68/78/247/4（test_descr unp→runtime）；round 97（续期后第 17 轮）：修 dict 相等比较清空操作数 UAF（键值各持一份+快照回退查找）；PYAWA_FRAME_OWN=1 全量套件 68 持平无 ok 回退（唯一差异 test_descr unp→runtime：type 双放）；套件 68/79/246/4；round 96（续期后第 16 轮）：修 list 定序比较活体双游标（真 UAF：元素的 __lt__ 清空操作数）；FRAME_OWN 实验路径 test_list 不再崩、默认套件 68 持平；test_weakref 超时另有 O(n²) 源；round 95（续期后第 15 轮）：形参所有权实验开关 PYAWA_FRAME_OWN=1（默认关、默认路径逐字节同 round 94）；实验路径 UAF 断言可点名借用实参站点（第一现场 ABCMeta.__new__ 类创建命名空间键）；套件 68 持平；round 94（续期后第 14 轮）：诊断四——ZOMBIE_TRACE 下对已释放对象的 incref 当场点名（附 Rust 回溯）；形参所有权第三处现场：build_class 搬运类命名空间时 incref(key) 的键早已被释放；套件 68 持平；round 93（续期后第 13 轮）：诊断三——PYAWA_REF_TRACE_BT（首次释放 Rust 回溯）；形参所有权两处现场点名（instance_attribute_delete_inner 的 name_object、比较协议 evil 二次释放）；套件 68 持平；round 92（续期后第 12 轮）：诊断二——live=false 现场安全化（REF_TRACKED 地址集合）；形参所有权第二次现场：delattr 删类命名空间键拿到悬垂键 ⇒ STORE_SUBSCR 的 _EnumDict.__setitem__ 协议分支＋形参所有权叠加的键计数缺口；套件 68 持平；round 91（续期后第 11 轮）：诊断增强（alloc/live=/通配符）——形参所有权第五次尝试的现场收敛：str 悬垂账平、问题是释放后裸借用（text_of 的 &'static str）⇒ 段错误；套件 68 持平；round 90（续期后第 10 轮）：PYAWA_REF_TRACE 诊断落地——A 族环外引用归因到"每次 Python 调用漏 1 份实参"；形参所有权第四次试后撤回（暴露类创建期 str 双放，EnumType.__new__@569）；套件 68 持平；round 89（续期后第 9 轮）：A 族孤儿用户类回收实现后撤回——顶层写法能收、函数内闭包写法因"环外引用未归因"（实例 rc=2、候选持有者只剩 dict_keyiterator）不收；套件 68 持平；round 88（续期后第 8 轮）：动态类类型 GC 实现后撤回（环外引用未归因/DANGLING invalid pointer 入账）+ dict(任意映射) 走 keys() 协议；套件 68 持平；round 87（续期后第 7 轮）：test_rlcompleter 转绿（ok 67⇒68）——builtins 去污染/dir 名字面/object 默认 dunder/用户 __getattribute__ 派发/文本签名扩展；survey 68/79/246/4；round 86（续期后第 6 轮）：inspect.signature 文本签名通路（compile PyCF_ONLY_AST 最小 AST + 内建 __text_signature__ 旁表 + object 特例）；survey 67/79/247/4；round 85（续期后第 5 轮）：iter(callable,sentinel)+惰性序列 iterator+itertools __next__ 属性+tokenizer `$`+compile _feature_version ⇒ test_iter 跑完；survey 67/81/245/4；round 84（续期后第 4 轮）：回溯逐帧展开 ⇒ test_frame 24⇒21；attach_traceback 保/恢复 pending 槽；三档试后撤回；survey 66/77/250/4（test_userstring 并发超时，基线同负载同值）；round 83（续期后第 3 轮）：f_generator 反向引用+StopIteration.value+localsplus 快照 ⇒ test_frame 25⇒24；形参所有权第三轮撤回（delattr 双放定位到类创建路径）；套件 67 持平；round 82（续期后第 2 轮）：终结局序真 bug 修（延迟队列临时引用误判复活 ⇒ __del__ 无限重复）＋Python 级 __del__ at-most-once；套件 67 持平；round 81（续期后第 1 轮，目标扩至 100 轮）：frame.clear+f_builtins ⇒ test_frame 29⇒25；形参所有权重试撤回（终结局序事实）；套件 67 持平；round 125（目标续期后第 20 轮／最后一轮）：视图 .mapping 活视图（sync_mappingproxy）⇒ test_dict 2F/0E⇒1F/0E；套件 67 持平；round 124（目标续期后第 19 轮）：A 族真 bug——dispatch_raise 未归还 Err(Raised) 引用 ⇒ 异常/帧/局部链泄漏；修后帧与局部按时回收；套件 67 持平；round 123（目标续期后第 18 轮）：视图键迭代原始迭代器 + dict(子类) 走 keys() 协议 ⇒ test_dict 3F/0E⇒2F/0E；套件 67 持平；round 122（目标续期后第 17 轮）：更新序列 __notes__ + dict.fromkeys 真类方法 ⇒ test_dict 4F/1E⇒3F/0E；套件 67 持平；round 121（目标续期后第 16 轮）：逐项缓存键哈希（DictObject.hashes）+ 字面量/setitem 哈希复用 ⇒ test_dict 6F/1E⇒4F/1E；套件 67 持平；round 120（目标续期后第 15 轮）：可失败查找面/整数哈希同值/dict.__eq__ 真实现/视图比较传播/update 末尾守卫/kwargs/is_tracked ⇒ test_dict 13F/4E⇒6F/1E；套件 67 持平；round 119（目标续期后第 14 轮）：dict 迭代变异检测（items/values 真迭代器+版本守卫、update 源变异）+ 视图迭代器 pickling + entry 直取性能 ⇒ test_dict 16F/4E⇒13F/4E；套件 67 持平；round 118（目标续期后第 13 轮）：dict 类级方法面/__ior__/clear-copy-popitem 参数口径/text_of 接受 str 子类 ⇒ test_dict 18F/9E⇒16F/4E；套件 67 持平；round 117（目标续期后第 12 轮）：两层闭包＋类体 nonlocal＋escape codec＋gethostname ⇒ test_dict 47F/17E⇒18F/9E、test_builtin 恢复 ok；套件 67/77/250/3；round 116（目标续期后第 11 轮）：test_dict 47F/17E ⇒ 34F/14E（dict 合并/视图比较/reversed/__missing__/参数口径/用户键 == 查找）；套件 67 持平（收敛轮）；round 115（目标续期后第 10 轮）：dict 视图真类型 ⇒ test_dict 错误 119⇒17（失败 47 持平）；套件 67 持平（收敛轮）；round 114（目标续期后第 9 轮）：defaultdict 方法面/deque maxlen/mappingproxy 来源类型/dataclass 换行/排序相等语义/类型级 repr 槽位 ⇒ test_pprint 转绿；套件 66⇒67；round 113（目标续期后第 8 轮）：types.SimpleNamespace 真类型＋sys.implementation 真实例＋实例 __dict__＋子类 __repr__ 与基类同对象 ⇒ test_pprint 15 格⇒9 格；套件 66 持平（收敛轮）；round 112（目标续期后第 7 轮）：mappingproxy 真类型（修 pprint 无限递归）＋大栈执行线程＋扩展切片赋值次序 ⇒ test_builtin 转绿、test_pprint 26 格⇒15 格、test_list 3 格⇒2 格；套件 65⇒66；round 111（目标续期后第 6 轮）：test_list 4F/3E⇒2F/1E（迭代器属性面/pickle 真接线、真 list_reverseiterator、子类 __iter__ 覆写、list.__eq__ 改表、-c 命令行、内置布局子类终结器）⇒ test_heapq 转绿；套件 64⇒65；round 110（目标续期后第 5 轮）：test_list 17F/6E⇒4F/3E（序列富比较协议/__init__/sort 相等与改表/迭代器耗尽哨兵/tuple*1/__imul__ 就地/list.__repr__ 改表安全/list() 关键字/staticmethod）；套件 64/77/253/3 持平；round 109（目标续期后第 4 轮）：ABI 追加 lutime/utime_ns（纳秒级 utime）＋copy.copy 的 __reduce_ex__ state＋bytes.partition＋types.MethodType＋stat(follow_symlinks=False)＋with 体内 return 的异常表切分＋尾斜杠 open 语义＋绑定报文 qualname ⇒ test_filecmp、test_shutil 转绿；套件 62⇒64；round 108（目标续期后第 3 轮）：OSError 三元组属性面＋str 格式（errno/strerror/filename/filename2）⇒ `test_unicode_file_functions` 转绿、`test_shutil` 87E⇒21E、`test_tempfile` 48E⇒35E；套件 61⇒62；round 107（目标续期后第 2 轮）：`unicodedata.normalize` 真实现＋`os.utime` 全链路＋WTF-8 串安全面 ⇒ `test_unicode_file` 转绿；round 106（目标续期后第 1 轮）：下标 `__index__`＋`list` dunder/copy 面＋序列重复收子类＋cell_clear 借用修＋推导式元组融合修 ⇒ `test_listcomps` 转绿、`test_list` 跑完 114 测试；套件 59⇒60；round 105（无翻页，目标末轮）：`__iter__` 返回值校验＋序列 `*=` 就地重复 ⇒ `test_list` 跑完 114 测试；套件 59／81／255／2（未实现 85⇒81、超时 3⇒2）；round 104：`int(bytes)`＋序列拼接收子类＋`list` 族 `__reduce_ex__` 可重建形状 ⇒ `test_xml_dom_minicompat` 转绿，套件 58⇒59；round 103：类型级 `__repr__` 每类型包装器＋身份哈希（pprint 分派真 bug）＋包优先＋模块 `__dir__`/`__doc__`＋`import *` 的 `__all__` 子模块 ⇒ `test_pkg`、`test_difflib` 转绿，套件 56⇒58；round 102（无翻页）：importlib fresh 子模块导入面真接线（finder 父包 __path__ 定位重接＋exec_module 三处缺陷＋locate_source 带点名字）＋子类切片 ⇒ test_xml_etree 族跑完 228 测试；套件 56/85/254/2（超时 3⇒2）；round 101：冻结模块测试面（`_imp._override_frozen_modules_for_tests` 真接线）⇒ `test_frozen` 转绿＋`from pkg import 子模块` 装载回退，套件 55⇒56；round 100：比较协议三处真语义（str/bytes 子类序、bytearray 等值、`!=` 优先级＋`object.__ne__`）⇒ `test_compare` 转绿，套件 54⇒55（新增 `test_decimal` 超时 137 s ✗）；round 99：`io.BufferedReader`/`BufferedWriter` 真实现＋`pow` 浮点/负指数面 ⇒ `test_urllib_response`、`test_pow` 转绿，套件 52⇒54；round 98：`import a.b.c as x` 链式 IMPORT_FROM 真 bug＋模块 `__spec__`／`__package__`＋4 元组/变长 charmap codec ⇒ `test_charmapcodec`／`test_sundry`／`test_stat` 转绿，套件 49⇒52；round 97（无翻页）：`io.open` 的 `opener`／内建 `format`／`os.chmod`／`os.stat(fd)`／`function.__call__`／`FileIO.writelines`／`os.symlink(target_is_directory)` ⇒ `test_tempfile` 2F/52E⇒5F/48E；套件 49／86／260／2（`test_collections` 并行调查下超时、单跑 66 s）；round 96：元组 freelist 复用＋`map` 泄漏修＋`__next__.__doc__` ⇒ `test_genexps` 转绿，套件 48⇒49；round 95：`del obj.attr` 的 `__delattr__` 分派（`unittest.mock` 收尾）⇒ `test__colorize` 转绿，套件 47⇒48；顺带 `sys.__stdout__`／标准流 `fileno`／`os.DirEntry` 导出／`stat_result` 必有名／`_io` 缓冲协议／`bytes` normpath；round 94：`bytes(str,enc)` 全编码（`fnmatch` bytes 模式）＋`os.lstat` 真实现（断链／`lexists`）⇒ `test_glob`、`test_fnmatch` 转绿，套件 45⇒47；round 93：编译器真 bug（`for: try: with:` 的正常出口跳回循环续点）＋`os.scandir`/`DirEntry`/`PathLike` ⇒ `test_mimetypes` 转绿、套件 44⇒45、`os.walk`/`shutil.rmtree` 转通；round 92：补 `gc.get_debug/set_debug/get_count`、`os.putenv/unsetenv/isatty`、`set.intersection` 一族 ⇒ 面向 `test_gc`/`test_mimetypes`/`test__colorize` 的首因，套件 44／85／267／1 ⇒ 44／86／266／1（无翻页）；round 91：`test_bufio` 转绿——`os.fstat`／`os.chdir`／`int.__index__`／`os.write` 缓冲协议＋`bytearray` 缓冲快路（`_pyio` 100 KB 0.27 s⇒毫秒）⇒ 套件 43⇒44；round 90：**撤回轮**——「帧拥有形参」实验：实参泄漏 +100⇒0、conformance 双模式绿，但 `__del__` 帧释放晚于 `python_level_finalize` 返回 ⇒ 复活误判 ⇒ `__del__` 无限重复 ⇒ 撤回；留下新事实（A 族新顺序）＋ `PYAWA_DEL_TRACE` 诊断；round 89：修 `itertools.groupby` 键双持有真双放 ⇒ **悬垂全量 memory 1⇒0（硬门①首次达成）**，套件 43／85／268／1 持平；round 88：内存安全专项启动（`docs/MEMORY_SAFETY.md`＋`memdangle_survey`）＋修 `filter` 迭代器 UAF ⇒ 悬垂全量 memory 5⇒1＋`getrandbits` 大 `k` O(n) ⇒ 超时回 1，套件 43／85／268／1；round 87：`float.conjugate`＋`divmod`＋`LOAD_GLOBAL.__missing__` ⇒ 41⇒43；round 86：set 索引失效＋`in` 迭代回退＋deque ⇒ 39⇒41）；`main` ＝ M6 合并提交（tag `m6`）。
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
  **round 71 完成**：千参编译链路去 O(n²)＋字典哈希/协议 ⇒ 64k 形参 lambda 4.25 s⇒0.20 s、`dict(可迭代)`
  2.7 s⇒0.03 s；套件 37／82／276／2 持平。
  **round 72 完成**：多字节 `EXTENDED_ARG`＋`type_lookup` 哈希 ⇒ `test_collections` 整份 59.5 s 跑完
  （超时解除）；但 `test_userstring` 101 s 临界被判 timeout ⇒ 套件 36／82／277／2（ok −1，如实记）。
  **round 73 完成**：`isinstance` ABC 快速通道 ⇒ `test_userstring` 101 s⇒80 s（ok 恢复 ✓）；套件
  **37／82／277／1**（超时 2⇒1，只剩 `test_weakref`）。
  **round 121 完成（诊断轮）**：① 钉「root 类字典」形状：`PYAWA_MARK_DEBUG` 根信息加 `root_keys`，实测
  `root_keys=[__module__,__qualname__,__firstlineno__,__del__,__static_attributes__]`（典型类字典）、
  `external[A]=0 rc=1`；② 根字典 `rc=1`、**gc 候选里无引用者**，把反查扩到 **live 全表**（新增
  `Instance::live_addresses()`，含未跟踪对象）仍 `[]` ⇒ **没有任何存活对象持有它但 rc=1** ⇒ 结论：
  不是「候选集/扣减」问题，而是**类字典引用记账多 1**（某处 incref 未配对的 transfer）或回收摘除
  `set_dict(None)+release(dict)` 的配对问题。③ 落地：`PYAWA_MARK_DEBUG` 扩根键/`root_type_dict_of`/
  live-then-gc 根引用者反查；`Instance::live_addresses()`；默认零输出/零开销。fastcheck OK（48 s）。
  **下一轮（round 122，行为变更轮）**：① 审计 `TypeObject::set_dict` 与 `build_class` 交命名空间的引用账
  （若确认多计 1 就修；注意 test_super/test_metaclass/test_descr 回归）⇒ 收 test_userlist.free_after
  ② 若账目正确：对「rc==1 且 live 反查为空」的类字典按孤儿处理（成本仅在显式/节流回收时）
  ③ test_descrtut 的 __slots__/dir(list) 与 test_userstring 提速。

## 内存安全专项（口径与路线）

- **口径／清单／路线／验收硬门**：见 [`docs/MEMORY_SAFETY.md`](docs/MEMORY_SAFETY.md)（**一处真相**）。
- 可执行基线：`python3 tools/memdangle_survey.py`（`PYAWA_DANGLING=1` 全量套件）⇒
  `target/memdangle_survey.tsv`；**memory 类计数收敛到 0** 是本专项第一硬门（目前非 0，数字见台账）。
- 本机限制（如实）：**无 valgrind**、rust 只有 stable（无 `-Zsanitizer`）⇒ ASan/valgrind 门先记"未测"，
  需先换/装工具链。

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
