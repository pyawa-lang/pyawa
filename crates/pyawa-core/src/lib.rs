//! Pyawa VM 核心：实例、对象模型、帧、字节码解释器、编译管线。
//!
//! 归属规格与硬约束见本 crate 的 `README.md`。模块树按规格编号组织，
//! 代码里的 `OM-n`／`BC-n` 指回那条硬约束。
//!
//! **已落地**
//!
//! - 对象模型 §4 实例级内存（**OM-1**…**OM-4**）
//! - 对象模型 §5 对象头（**OM-5**…**OM-8**）
//! - 对象模型 §6 类型对象的最小骨架（**OM-9**…**OM-15**：字段与槽位就位、注册表按实例）
//! - 对象模型 §7 引用计数协议（**OM-16**…**OM-22**；**OM-21** 的显式待处理栈已用于深链释放）
//! - 对象模型 §8 单例表（**OM-23**：`None`／`True`／`False`／小整数按实例创建；
//!   `IMMORTAL` 位按 **OM-24** 只预留并保持 0）
//! - 对象模型 §9 的**回收算法**：标记-清除（**OM-25**）、触发点与可配阈值（**OM-26**）、
//!   回收顺序的 ③④（**OM-27**：终结器 → 释放，含复活后的重判定）、遍历计数（**OM-29**）
//! - 对象模型 §12 的值表示（**OM-38**／**OM-39**：带标签枚举，只有单例覆盖到的值才内联）
//!   与类型擦除守卫 `PyRef`（**OM-40** 的访问器 `Instance::own`）
//! - 字节码 §8 的**指令表与元数据**（**BC-30**…**BC-42**：基线 154 名与编号、专有指令取空闲编号、
//!   19 个 cache 宽度、七类分类、栈效应规则、`nb_ops` 与 `cmp_op` 顺序、指令集版本常量）——
//!   数值由 `tools/gen_opcode_tables.py` 从运行时探测生成，`pyawa-stdlib` 的
//!   `_opcode`／`_opcode_metadata` 是它的 Python 层包装（**BC-38** 的依赖边裁决）
//! - 字节码 §8 的**逐指令 oparg 解码**（**BC-57**／**BC-58**／**BC-59**）：`src/argdecode.rs`
//!   按 `dis` 的口径产出 `argval`／`argrepr`，由 oracle 夹具逐条对拍（`T-BC-19`…`T-BC-21`）
//! - 字节码 §8 的**码元解码**（**BC-32**…**BC-36**：2 字节码元、`EXTENDED_ARG` 大端折叠、
//!   cache 槽跳过，以及发射方体检 `validate`）
//! - 字节码 §9 的**帧布局**（**BC-42**…**BC-48**）：值栈上界、局部槽、独立的 cell 槽、
//!   指令指针与异常表游标、可挂起状态（挂起／恢复）；`BC-54` 的异常表**只保存字节串**
//! - 字节码 §9 的 **BC-45**：cell 是独立对象且 `GC_TRACKED`（经 `traverse`／`clear` 入链）
//! - 字节码 §9 的 **BC-54**：`co_exceptiontable` 的解析（4 个 varint／条，字节偏移与 `dis` 一致）
//! - 字节码 §10 起步指令集的前两批（**BC-49**）：常量与局部、整数运算与比较、`is`、
//!   `RETURN_VALUE`，以及**控制流**（跳转目标按 **BC-55** 算，`T-BC-17` 用参照实现产出的
//!   码元逐条对拍）——这一条是**早期快照**：调用、容器、属性与下标、异常、`with` 后来都已接线，
//!   当前覆盖面与缺口看本文档后面的分组清单（生成器只接了骨架，方法族与协程仍未接线）
//! - 对象模型 §6 的 **OM-13**：MRO 用 **C3 线性化**（钻石继承与不一致基类都有用例）
//! - 类型系统 **TS-41**／**TS-42**／**TS-43**：内建类型表**由探测导出**（`tools/gen_builtin_types.py`
//!   → `src/builtin_types.rs`，130 个类型的 `__bases__`／`__mro__`）；**第一阶梯**已注册——
//!   `object`／`type`／`NoneType`／`bool`／`int`／`float`／`str`，载荷布局按 `TS-43` 由实现自选
//!   （见 `src/builtin_objects.rs`）
//! - 类型系统 **TS-40**：`bool ⊂ int`（`True + 1` 算 2）＋ `Instance::is_subtype`（走 MRO）
//! - 字节码 §10 的**容器与解包**族（**BC-49**）：`tuple`／`list`／`dict`／`set` 四个类型
//!   （层次查探测表、载荷按 `TS-43` 自选，见 `src/builtin_objects.rs`）＋ `BUILD_TUPLE`／
//!   `_LIST`／`_MAP`／`_SET`／`_STRING`、`UNPACK_SEQUENCE`／`UNPACK_EX`、`LIST_APPEND`／
//!   `SET_ADD`／`MAP_ADD`／`LIST_EXTEND`／`SET_UPDATE`
//! - 字节码 §10 的**调用与返回**族（**BC-49**／**BC-56**）：`function` 类型（M2）、
//!   `PUSH_NULL`／`MAKE_FUNCTION`／`SET_FUNCTION_ATTRIBUTE`／`CALL`／`CALL_KW`，
//!   以及**参数绑定**（仅位置 → 位置或关键字 → `*args` → 仅关键字 → `**kwargs`；
//!   四类错误各成一个变体）。`T-BC-18` 要求消息与参照实现一致——消息已实测记录在
//!   `tests/calls.rs` 的文档里，等异常对象接线后照抄
//! - `OM-11` 的 **`getattr`／`setattr` 槽位**（形状按 `SPEC-bytecode.md` §10 的注"由实现自选"：
//!   返回新引用／`None`）＋ `BC-4` 的第一批 `co_*` **计算型属性**
//!   （`co_name`／`co_qualname`／`co_filename`／`co_firstlineno`／`co_argcount` 一族／
//!   `co_varnames`／`co_names`／`co_consts`），
//!   走槽位而不是给内建类型旁路
//! - 字节码 §10 的**模式匹配族**（**已接线且有用例**）：对拍夹具 `tools/gen_match_fixture.py`
//!   ⇒ `tests/fixture-match-3.14.json`（10 个"被测值 × 两类模式"的判定结果，参照导出；
//!   `tests/patterns.rs` 手搭同一形状的骨架逐条比对）。口径细节见下：
//!   `MATCH_SEQUENCE`／`MATCH_MAPPING`（净 +1）、
//!   `MATCH_KEYS`（净 +1：**保留**被测对象与键 tuple，压"值的 tuple"或 `None`）、
//!   `MATCH_CLASS`（净 −2：**连被测对象一起吃掉**、只压结果）、`STORE_FAST_STORE_FAST`
//!   （净 −2，打包槽位：高 4 位收 TOS）、`NOT_TAKEN`（§10 三分类②：**必须容受**，无操作）。
//!   实测口径：`str`／`dict` 都**不算**序列；缺键 ⇒ 该 case 不匹配
//! - 字节码 §10 的**格式化族**：`FORMAT_SIMPLE`（净 0，`str()`）、`CONVERT_VALUE`（净 0，
//!   `!s`／`!r`／`!a`，实测 oparg 1／2／3）、`BUILD_STRING`（净 −(n−1)，早先已落地）
//! - 字节码 §10 的**生成器族**：`CO_GENERATOR`（实测 32）的 `CALL` **不跑函数体**而是把挂起的帧
//!   包成生成器；`RETURN_GENERATOR`（恢复时是空操作）、`YIELD_VALUE`（挂起：值栈进恢复点、
//!   ip 指向下一条）、`ExecOutcome` 把"返回"与"让出"分开；`GET_ITER` 认"生成器是它自己的
//!   迭代器"、`FOR_ITER` 的取下一个就是**恢复生成器的帧**（跑完走耗尽路径）；`execute` 会从
//!   帧的**恢复点**接上（`BC-47`）；`yield from` 那一套也通了：`GET_YIELD_FROM_ITER`（净 0）、
//!   `SEND`（让出就压让出的值往下走、耗尽就压返回值并跳转）、`END_SEND`（**去掉 TOS1 的接收者**、
//!   把结果留下）、`JUMP_BACKWARD_NO_INTERRUPT`
//! - 字节码 §10 的**异常族前半（能抛）**：`BaseException` 层次（**69 个类**，名字与基类都来自
//!   `TS-41` 的表，多继承那几支走 C3）＋ `ExceptionObject` 载荷（`args`／`__cause__`／
//!   `__context__`／`__suppress_context__`）＋ `RAISE_VARARGS`（0 重抛／1 `raise X`／2 `raise X from Y`）。
//!   `BC-60` ②：当前异常状态与最近抛出的异常都**按实例存**（无进程级全局）
//! - 字节码 §10 **异常族的后半（处理块派发）**（**BC-60** ①）：异常表区间查询 → 值栈**回退到
//!   `depth`** → `lasti` 置位时压最后一条指令偏移 → 压异常实例 → 跳到处理块入口；
//!   `PUSH_EXC_INFO`／`CHECK_EXC_MATCH`／`POP_EXCEPT`／`RERAISE`（栈形状按参照实现的发射骨架**实测**
//!   导出，见 `tests/handlers.rs` 的文档）
//! - `BC-56`／`T-BC-18`：参数绑定错误现在抛**真** `TypeError`，消息按参照实现**实测**的公式拼
//!   （`demo() takes 2 positional arguments but 3 were given` 一类；两参用 `'a' and 'b'`、
//!   三参及以上用 `'a', 'b', and 'c'`——都是实测出来的差别）
//! - **类体编译**（`class C[(B)]: …`）**已落地两档**：① 体里**不含 `def`** 的那一支与参照
//!   逐字节一致（`tests/compile.rs` 语料）＋ 端到端（`tests/compiled_class.rs`）；
//!   ② 体里**含 `def`** 的那一支：编译器按实测发射 `__classdict__` cell 那一套
//!   （`MAKE_CELL` 在 `RESUME` 之前、`LOAD_LOCALS; STORE_DEREF 0`、收尾前
//!   `LOAD_FAST_BORROW 0; STORE_NAME __classdictcell__`）、嵌套 `def` 放开，qualname 规则为
//!   模块 `f`／类体 `C.m`／函数 `f.<locals>.g`，语料同样**逐字节**一致。
//!   **运行期也已打通**：`LOAD_FAST_BORROW` 在局部槽**越界**时回落到**同号 cell 槽**——
//!   参照把 cell 也放在"快速局部槽"里（类体收尾用 `LOAD_FAST_BORROW 0` 读 `__classdict__`），
//!   而 `BC-45` 在本层是**独立 cell 槽** ⇒ 补这条兼容后类体跑得起来（端到端用例
//!   `tests/compiled_class.rs::a_class_body_with_a_def_runs_end_to_end`）。
//!   另外：参照给合成指令 `MAKE_CELL` 的 `co_positions()` 是 `(None, None, None, None)`，
//!   本层位点表每项都是四个整数 ⇒ **表达不了「缺失」**（语料里如实标注，不假装对齐）
//! - **属性访问（`self.x`）编译器那半已落地**：词法加 `.`、后缀解析（可连缀 `a.b.c`）、
//!   AST 的 `Expression::Attribute`、发射 `LOAD_ATTR <名字下标 << 1>`（低位是"取方法"标志，
//!   纯取值是 0）。语料用**两个**不同属性名把位移暴露出来 ⇒ 与参照**逐字节**一致；
//!   端到端：建类 → 实例化 → 调方法 → 经 `self.x` 读回类属性（`tests/compiled_class.rs`）
//! - **属性写（`self.x = 1`）已落地**：语句变体 `AssignAttr`（对象.名字 ＝ 表达式，可多级
//!   `a.b.c = …`），发射是**先值后对象**再 `STORE_ATTR <名字下标>`（实测）；语料用"先写 `self.y`
//!   再读 `self.z`"两个名字把写那条的下标暴露出来 ⇒ 与参照**逐字节**一致；端到端：建类 →
//!   实例化 → `m()` 里写 `self.x = 5` → 从实例上读回 5（`tests/compiled_class.rs`）
//! - **`__init__` 传参链：编译器那半已通，运行期还差一处**。语料
//!   `class P: def __init__(self, v): self.v = v …` 与参照**逐字节**一致；但 `P(9)` 在运行期报
//!   `FellOffEnd`（`P()` 不带参时没事，因为那些类没有 `__init__`）⇒ 缺陷在运行期的
//!   **已排除一种可能（本轮探针）**：`type_lookup(class, "__init__")` 拿到的对象类型名是
//!   **`function`**（就是我们自己那个自定义函数，不是 `object.__init__` 的槽包装）⇒ 所以故障在
//!   **第二轮探针（本轮）又排除一种可能**：把类里的 `__init__` 与模块里的普通函数**逐字段对比**
//!   ——类型名都是 `function`、`argcount`／`nlocals` 都对（2／2 与 1／1）、`flags` 都是 `0x3`、
//!   都没有 free／cell 变量、`globals` 都已绑 ⇒ **函数对象本身没问题**。
//!   **二分办法（下一轮第一步）**：直接从 Rust 用 `[实例, 3]` 调那个 `__init__`——
//!   成功 ⇒ 故障在 `type_call` 的**转交**（`created` 的那份引用／实参表）；
//!   同样报 `FellOffEnd` ⇒ 故障在**帧执行**（此时再打印帧的 ip 与 code 长度）。
//! - 仍缺：**下标赋值**（`a[0] = 1`，形状待测）
//! - 字节码 §10 的**迭代族**：`GET_ITER`／`FOR_ITER`／`END_FOR`／`POP_ITER`／`GET_LEN`，
//!   迭代器类型（`tuple_iterator`／`list_iterator`／`str_ascii_iterator`／`dict_keyiterator`／
//!   `set_iterator`——名字照探测表取）与 `SWAP`／`COPY`（§10 表外的增量）
//! - 字节码 §10 属性与下标族的**属性**部分：`LOAD_ATTR`（含**取方法**低位）、`STORE_ATTR`、
//!   `DELETE_ATTR`，加上类型字典（沿 MRO 查）与**实例属性字典**（`new_attribute_type`；
//!   参照实现里 `object()` 自己**没有** `__dict__`，故字典挂在 `AttributeObject` 载荷上）
//! - 字节码 §10 属性与下标族的**下标**部分：`BINARY_OP` ＋ `NB_SUBSCR`（3.14 无 `BINARY_SUBSCR`）、
//!   `STORE_SUBSCR`、`DELETE_SUBSCR`——`tuple`／`list`／`dict`／`str`，负下标与值相等的键都在内
//!
//! **尚未接线**（占位，不要当成已就位）：
//!
//! > **读法**：本段是**按历轮落地顺序追加**而成的，所以里面混有"后来补齐"的条目——以**条目自己的
//! > 措辞**为准（写着"已接线／已落地／已就位"的就是已经能跑的；写着"要等…／尚未…／随后接"的才是
//! > 待做）。历轮追加时没有回头重排段落，这里如实留痕，而不是把已落地的条目搬来搬去（搬运容易
//! > 出错，也容易丢内容）。
//!
//! - `TS-42` 的 **M2 阶梯**其余部分：函数对象／迭代器对象／`BaseException` 层次（表里已有 80 项）
//! - 字节码 §10 容器族的其余指令：`BUILD_SLICE`（要 M3+ 的 `slice` 类型）、`DICT_UPDATE`／
//!   `DICT_MERGE`（要字典源与重复键的 `TypeError`，异常对象未接线）
//! - **`OM-11` 的 `getattr`／`setattr` 槽位**：现在的查找顺序（实例字典 → 类型 MRO → 报错）
//!   写死在执行器里；类型可覆写的槽位与数据描述符随类型系统接线
//! - 取绑定方法：`method` 类型、绑定对象与 `repr` **都已就位**（`obj.method` 不调用的取值
//!   走 `attribute_read` 的绑定分支）
//! - `LOAD_SUPER_ATTR`：要 `super()` 的 `__class__` cell
//! - **协程**（`§10` 的生成器与协程族）：`CO_COROUTINE`（实测 `0x80`）的 `CALL` 交出协程对象
//!   （载荷与生成器同形、类型不同）、`GET_AWAITABLE`（净 0：协程与 `CO_ITERABLE_COROUTINE`
//!   生成器**原样**就是 awaitable，其余走 `__await__`，都没有 ⇒ 实测
//!   `'int' object can't be awaited`）、`SEND`／`send`／`throw`／`close` 与生成器共用一条路；
//!   协程**不是迭代器**（没有 `__next__`／`__iter__`）。协程里逃出来的 `StopIteration`
//!   按**被驱动对象的种类**变成 `RuntimeError`（实测两句话：`coroutine raised StopIteration`／
//!   `generator raised StopIteration`），转换做在知道种类的那一层。
//! - **异步生成器**（`CO_ASYNC_GENERATOR`，实测 `0x200`）：`CALL` 交出 `async_generator` 对象
//!   （载荷同形，`repr` 的词是 `async_generator`），类型按探测表注册；`GET_AITER`（净 0；
//!   非异步可迭代的实测消息 `'async for' requires an object with __aiter__ method, got int`）、
//!   `END_ASYNC_FOR`（净 −2；`dis` 的 argrepr 用 **"from"** 不是 "to"，对拍抓出来的）、
//!   `CLEANUP_THROW`（净 −1）都已接线；`SEND` 把异步生成器跑完 ⇒ `StopAsyncIteration`。
//!   **未接线**：`async_generator.__anext__()` 交出的 awaitable（参照实现叫
//!   `async_generator_asend`）——所以 `GET_ANEXT` 碰上异步生成器**如实报未接线**，
//!   `async for` 还跑不完整。**实测纠错**：异步生成器**不能**直接 `await`
//!   （`TypeError: 'async_generator' object can't be awaited`）——第一版图省事写成
//!   "await 一次推进一格"，被这条实测打回
//! - **`async with` 已接线且有用例**：3.14 **没有** `BEFORE_ASYNC_WITH` 这条指令（实测
//!   `opcode.opmap` 里没有），它由 `LOAD_SPECIAL __aenter__`／`__aexit__`（特殊方法表下标 2／3）
//!   ＋ `GET_AWAITABLE`／`SEND` 构成，这两条都已接线 ⇒ 用例覆盖"进／出各 await 一次、
//!   `as` 拿到 `__aenter__` 交回的值"。协程的 `__await__`／`cr_*` 与异步生成器的
//!   `ag_*`／`aclose`／`athrow` 仍未接线
//! - **生成器的 `throw`／`close` 已接线**（"恢复时先抛"：帧上有一个"待抛异常"格，
//!   `execute` 一恢复就按**本帧的**异常表派发它 ⇒ 生成器体里的 `try/except` 接得住）。
//!   实测口径：`close()` 在已结束/从未启动时给 `None` 且**不跑函数体**、被关闭时又让出 ⇒
//!   `RuntimeError: generator ignored GeneratorExit`、正常收尾或捕获后 `return` ⇒ 交回**返回值**、
//!   抛出别的异常原样往外；`throw` 在已结束/从未启动时抛在**调用处**，类自动实例化，
//!   实例再带值 ⇒ `TypeError: instance exception may not have a separate value`，
//!   超过 3 个实参 ⇒ `TypeError: throw expected at most 3 arguments, got N`
//! - **编译器 `P1-10`**（`BC-14`…`BC-18`、§11）：**最小可测切片已接线**——词法 → 语法 → 发射，
//!   产物过 `validate`，并与参照实现**逐条指令（含字节偏移）**对拍
//!   （`tools/gen_compile_fixture.py` ⇒ `tests/fixture-compile-3.14.json`，9 段源码全绿）。
//!   覆盖：模块级 `NAME = <表达式>`（`;`／换行分隔）、`def NAME(形参…):` ＋ 缩进体、
//!   函数体里的赋值与 `return`、十进制整数字面量（`0..=255` 走 `LOAD_SMALL_INT`）、
//!   单引号字符串、名字、`+`（非全字面量时发射 `BINARY_OP`；两侧都是局部借入时打超指令）。
//!   夹具 21 段源码，其中的"已覆盖"子集与参照**逐条指令（含字节偏移与 `BC-18` 的位置表）**一致。
//!   **两处实测怪规则**（原因不明，照实实现并让夹具盯着）：小整数**只在常量表为空时**登记
//!   （`x = 1` ⇒ `[1, None]`，但先有 code 常量后再 `x = 1` ⇒ `[<code>, None]`）；
//!   函数常量表**只有它自己没有别的常量时**才登记 `None`。
//!   **常量折叠已接线**（三条实测规则：只有**最左叶子**进常量表；结果是小整数走
//!   `LOAD_SMALL_INT` 不进表；否则该常量**收尾之后**才登记——`x = 200 + 100` ⇒
//!   `[200, None, 300]`；字符串也折叠）。
//!   **检查档位**（`TS-31`）是**显式编译输入**（`CheckTier`：浅层默认／深层），与模式、优化级
//!   同层；`.pyac` 头部按 `IM-19` 带上它（排在优化级之后），陈旧判定比它（`IM-20`），
//!   产物五要素含它（`IM-21`）。**深层档位的执行器那半已落地**（标签带内层 ⇒ 对 `list`／`tuple`
//!   元素递归比，`TS-31`）；编译器按标注**发**检查指令（`BC-25`）仍待标注支持。
//!   编译器还接上了**调用**（`<可调用>; PUSH_NULL; <实参…>; CALL <个数>`，位置逐形态实测）
//!   与**比较**、**`if`／`else`**、**`while`**（回边按实测公式回填；条件是比较时不补 `TO_BOOL`）、**`for`**
//!   （`GET_ITER`／`FOR_ITER`／`JUMP_BACKWARD`／`END_FOR`／`POP_ITER`）、**两者的 `else`**、
//!   **关键字实参**
//!   （`CALL_KW` ＋ 名元组常量）、**`*`／`**` 实参**（`CALL_FUNCTION_EX`：`BUILD_LIST`／
//!   `LIST_EXTEND`／`INTRINSIC_LIST_TO_TUPLE`／`BUILD_MAP`／`DICT_MERGE`）（跳转按 `BC-55` 回填，含缓存宽度；末尾 `if`
//!   的分支隐式 return 与"两分支都 return 则不补收尾"两条都是实测）。
//!   **位置表（`BC-18`）已接线**（逐形态实测：见 `compile.rs` 的模块文档表）；注意它目前只在
//!   编译产物里（`CompiledUnit::positions`），**还没**接到 Python 可见的
//!   `co_positions()`／`co_lines()`（那是 `CodeObject` 存与取的事，下一片）。
//!   **未接线**（照实报 `Unsupported`／`Syntax`，不猜）：制表符缩进、嵌套函数定义、
//!   负数与转义、任意精度整数（`i64` 溢出）、链式比较、多个 `*` 实参、`break`／`continue`、
//!   **嵌套调用的位置**（该段语料如实标为未覆盖）、`EXTENDED_ARG`、
//!   **`if` 形态下的模块收尾位置**（4 段 `if` 语料如实标注，指令流照常对拍）、
//!   扩展模式的边界检查指令
//! - **`.pyac` 容器格式**（`P3-12` 起步，`IM-18`…`IM-21`）：产物路径规则、头部编解码、
//!   **两步陈旧判定**（① 按名字精确查找——别的版本／模式留下的产物连读都不读；
//!   ② 读头部比指纹，长度相同而哈希不同也算陈旧）、以及**纯函数性**（`encode` 只吃
//!   「源码＋模式＋优化级＋指令集版本」，路径与时间**结构上**进不来）。
//!   规格只固定**字段顺序**（`IM-19`），具体编码（magic、小端宽度、FNV-1a 64 位指纹）由本层
//!   选定并记在 `crates/pyawa-runtime/src/pyac.rs` 的模块文档里；代码段在这一层是不透明字节，
//!   谁来填（Pyawa 自己的编译器，`IM-28`）不归它管
//! - **`OM-14` 的实例字典挂载**：机制（内联／另行挂载两种）早已就位；这一轮补上它的
//!   **可观察面** `__dict__`——实例上返回**那个字典本身**（实测：同一个对象、透过它加属性
//!   立刻可见、`obj.__dict__ = {…}` **整体替换**且旧键随之不可见；还没写过时**惰性建**一个空
//!   字典）；赋值不是字典 ⇒ 实测 `TypeError: __dict__ must be set to a dictionary, not a 'int'`；
//!   类型不带实例字典 ⇒ 实测那条 `AttributeError: 'X' object has no attribute '__dict__'`。
//!   **规格未点名** `__dict__`（口径全部取自参照实现），如实记在这里。
//!   **未接线**：类对象上的 `C.__dict__`（参照实现给 `mappingproxy` 只读视图，本层还没有那个类型）
//! - **`co_*` 属性面**（`BC-4`）：参照实现有 22 个，本层已接线 **19** 个（含 `co_cellvars`／
//!   `co_freevars`——名字**单独存**，不能从 `co_varnames` 推，实测那里只有局部名）；
//!   `co_code`／`co_exceptiontable`／`co_linetable`／`co_lnotab` 要 `bytes` 类型（M3+）；
//!   **`co_positions()`／`co_lines()` 已接线**（`P1-10` 的编译器产出位置表 ⇒ `CodeObject`
//!   带着它 ⇒ 过属性通道交出**迭代器**；与参照逐条对拍，见 `tests/compile.rs`）；
//!   `co_branches()` 未接线（要 3.14 的异常分支表）；
//!   另有两个本层多出来的便利属性 `co_ncellvars`／`co_nfreevars`（参照实现没有）
//! - **名类**：`LOAD_NAME`（局部 → 全局 → 内建）、`LOAD_GLOBAL`（全局 → 内建；`BC-57` 的
//!   `>> 1` 移位与低位的"先压 `NULL`"）、`STORE_GLOBAL`／`DELETE_GLOBAL` 都已接线；
//!   函数的 `__globals__` 由 `MAKE_FUNCTION` 捕获（帧的全局层由函数带进调用），
//!   类体帧取类体函数那一层。**内建层是可选槽**（`Instance::set_builtins`）——
//!   核心引导期只装 `__build_class__` 一个对象、还不是映射，等 stdlib 装进来
//! - **`in` 的字节码形态 `CONTAINS_OP` 已接线**（`str`／`list`／`tuple`／`dict`／`set`；
//!   两条实测错误消息：非容器 ⇒ `argument of type 'X' is not a container or iterable`、
//!   `str` 容器而左操作数不是 `str` ⇒ `'in <string>' requires string as left operand, not X`）。
//!   元素比较走本层的 [`values_equal`]（整数／浮点／字符串按值、其余按身份）⇒ 容器之间的
//!   **值相等已接线**（`list`／`tuple` 递归、`dict` 按键匹配、`set` 双向包含）；仍等
//!   `OM-11` 的 `richcompare` 槽位的是容器的**序**比较（`<` 一族）
//! - **`with` 协议**：`LOAD_SPECIAL`（`__enter__`／`__exit__`）与 `WITH_EXCEPT_START` 已接线
//!   （骨架照参照实测；正常出口 `__exit__(None, None, None)`、异常出口按返回值抑制或重抛）。
//!   仍缺 `async with` 的 `BEFORE_ASYNC_WITH`／`GET_AWAITABLE` 一族（与协程同批）
//! - **`CompiledUnit` 已带 `cellvars`／`freevars`**（`BC-45`）：`instantiate` 传给 `CodeObject`，
//!   `.pyac` 的代码段同步编解码这两个文本表
//! - **cell 族已落地**（`BC-45`）：`MAKE_CELL`（在 cell 槽建 cell，初值取同号局部槽）／
//!   `LOAD_LOCALS`（压本帧命名空间；函数帧没有 ⇒ 如实报未接线）／`STORE_DEREF`／`LOAD_DEREF`；
//!   `cell` 类型**显式注册**（它在 `TS-42` 的探测表里挂 `Ladder::Later` ⇒ 引导期不会自动建）；
//!   验收 `tests/cell_opcodes.rs`。**仍未接线**：`COPY_FREE_VARS` 与"函数里嵌套 `def`"
//! - 调用族的其余部分：闭包（`COPY_FREE_VARS`…，**函数里读全局名
//!   已落地**：`LOAD_GLOBAL`）、
//!   `CALL_FUNCTION_EX`（`*args`／`**kwargs` 展开）、生成器与协程（生成器的 `send`／`__next__`
//!   已接线，见下）。`SET_FUNCTION_ATTRIBUTE` 的 `16`（`annotate`）**已落地**
//!   （`FunctionObject::annotate` ＋ `f.__annotate__`／`__annotations__`／`__doc__`）
//! - 异常对象：`args`／`__traceback__` 一族见上；参数绑定错误**已按参照实测拼消息**
//!   （`T-BC-18`）
//!   返回新引用／`None`）＋ `BC-4` 的第一批 `co_*` **计算型属性**
//!   （`co_name`／`co_qualname`／`co_filename`／`co_firstlineno`／`co_argcount` 一族／
//!   `co_varnames`／`co_names`／`co_consts`），
//!   走槽位而不是给内建类型旁路
//!   ⇒ `tests/fixture-match-3.14.json`（10 个"被测值 × 两类模式"的判定结果，参照导出；
//!   `tests/patterns.rs` 手搭同一形状的骨架逐条比对）。口径细节见下：
//!   `MATCH_SEQUENCE`／`MATCH_MAPPING`（净 +1）、
//!   `MATCH_KEYS`（净 +1：**保留**被测对象与键 tuple，压"值的 tuple"或 `None`）、
//!   `MATCH_CLASS`（净 −2：**连被测对象一起吃掉**、只压结果）、`STORE_FAST_STORE_FAST`
//!   （净 −2，打包槽位：高 4 位收 TOS）、`NOT_TAKEN`（§10 三分类②：**必须容受**，无操作）。
//!   实测口径：`str`／`dict` 都**不算**序列；缺键 ⇒ 该 case 不匹配
//!   `!s`／`!r`／`!a`，实测 oparg 1／2／3）、`BUILD_STRING`（净 −(n−1)，早先已落地）
//!   包成生成器；`RETURN_GENERATOR`（恢复时是空操作）、`YIELD_VALUE`（挂起：值栈进恢复点、
//!   ip 指向下一条）、`ExecOutcome` 把"返回"与"让出"分开；`GET_ITER` 认"生成器是它自己的
//!   迭代器"、`FOR_ITER` 的取下一个就是**恢复生成器的帧**（跑完走耗尽路径）；`execute` 会从
//!   帧的**恢复点**接上（`BC-47`）；`yield from` 那一套也通了：`GET_YIELD_FROM_ITER`（净 0）、
//!   `SEND`（让出就压让出的值往下走、耗尽就压返回值并跳转）、`END_SEND`（**去掉 TOS1 的接收者**、
//!   把结果留下）、`JUMP_BACKWARD_NO_INTERRUPT`
//!   `TS-41` 的表，多继承那几支走 C3）＋ `ExceptionObject` 载荷（`args`／`__cause__`／
//!   `__context__`／`__suppress_context__`）＋ `RAISE_VARARGS`（0 重抛／1 `raise X`／2 `raise X from Y`）。
//!   `BC-60` ②：当前异常状态与最近抛出的异常都**按实例存**（无进程级全局）
//!   `depth`** → `lasti` 置位时压最后一条指令偏移 → 压异常实例 → 跳到处理块入口；
//!   `PUSH_EXC_INFO`／`CHECK_EXC_MATCH`／`POP_EXCEPT`／`RERAISE`（栈形状按参照实现的发射骨架**实测**
//!   导出，见 `tests/handlers.rs` 的文档）
//!   （`demo() takes 2 positional arguments but 3 were given` 一类；两参用 `'a' and 'b'`、
//!   三参及以上用 `'a', 'b', and 'c'`——都是实测出来的差别）
//!   迭代器类型（`tuple_iterator`／`list_iterator`／`str_ascii_iterator`／`dict_keyiterator`／
//!   `set_iterator`——名字照探测表取）与 `SWAP`／`COPY`（§10 表外的增量）
//! - 字节码 §10 属性与下标族的**属性**部分：`LOAD_ATTR`／`STORE_ATTR`／`DELETE_ATTR`／
//!   `LOAD_SUPER_ATTR`——要动 `OM-11` 的 `getattr`／`setattr` 槽位（槽位形状见下）
//! - 切片（`a[1:2]`）：要 `TS-42` 里排在 M3+ 的 `slice` 类型
//! - 字节码 §10 **异常族的后半（处理块派发）**：`PUSH_EXC_INFO`／`CHECK_EXC_MATCH`／`POP_EXCEPT`／
//!   `RERAISE`，以及 `BC-60` ① 的 `depth`／`lasti` 落点（判据 `T-BC-22`）
//! - **原生可调用对象**（`builtin_function_or_method`，探测表的 `later` 阶梯）：Rust 函数 ＋ 名字，
//!   实参以**借用视图**递进去、返回值是**新引用**，绑定形态多带一个 `self`；`repr` 实测
//!   `<built-in function len>`。它是 `AB-24`／`AB-25` 的宿主函数与 `__build_class__` 一类
//!   内建函数的落点
//! - `FORMAT_WITH_SPEC`（`§10` 格式化族最后一条）：按 **`TS-44`** 的裁定——**语义只走属性通道**，
//!   内建类型在**类型字典**里放**原生可调用对象**（`object` 给默认：空规格 ⇒ `str(x)`、
//!   非空 ⇒ TypeError），**没有** `format` 槽（`OM-11` 的清单不扩充）。迷你语言在 `src/format.rs`，是**受测子集**（对齐／填充／`0`／符号／
//!   `#`／宽度／分组／精度／`d`/`b`/`o`/`x`/`X`/`c`/`f`/`e`/`g`/`%`/`s`），
//!   形状与错误消息**逐条实测**：`format(42, '05') = '00042'`、`format('ab', 'd')` ⇒
//!   `ValueError: Unknown format code 'd' for object of type 'str'`、`format(None, 'd')` ⇒
//!   `TypeError: unsupported format string passed to NoneType.__format__` 等
//! - **`OM-11` 的 `repr`／`str` 槽**（语义按 `SPEC-type-system.md` §8 的表：`str` 省略时**回退到
//!   `repr`**，两者都省略时由类型对象给默认形式 `<X object at 0x…>`）：`int`／`bool`／`None`／
//!   `float`／`str`／`list`／`tuple`／`dict`／`set`／类型／生成器／函数／code object／绑定方法／
//!   异常，形状**逐条实测**；`repr` 带**递归守卫**（按实例存，`CX-3`）⇒ 自引用给 `[[...]]`／
//!   `{'k': {...}}`。`FORMAT_SIMPLE`／`CONVERT_VALUE` 因此改走真槽位（临时垫片已删）
//! - 内建指令 `CALL_INTRINSIC_1`（净 0；**按名字**分派，编号到名字来自探测产物）：
//!   `INTRINSIC_UNARY_POSITIVE`（整数／布尔）、`INTRINSIC_LIST_TO_TUPLE`、
//!   `INTRINSIC_STOPITERATION_ERROR`（生成器里漏出的 `StopIteration` ⇒ `RuntimeError`，
//!   实测原话 `generator raised StopIteration`）；其余 intrinsic 如实报未接线（带名字）
//! - 普通迭代器的 `SEND`（`yield from [1, 2]` 那条）：走"取下一个"，耗尽时压 `None`；
//!   `FOR_ITER` 与它共用同一个推进助手
//! - **`__new__` 分派**（`OM-14` 的"子类分派槽位"里 Python 侧那一半）：类型被调用时先找
//!   类字典里的 `__new__` 并传 `(cls, *args, **kwargs)`；它交回的**不是本类实例**时
//!   `__init__` **不**被调用（实测），是实例时照常调 `__init__`（实参仍给全）。
//!   另按实测补上"无 `__init__` 且走通用分配时带实参创建 ⇒ `X() takes no arguments`"，
//!   判据用类型标志 [`GENERIC_ALLOCATION`]（**不**比较函数指针）
//! - `OM-11` 的 **`call` 槽**（宿主函数与"有 call 槽即可调用"的判定都走它）＋ 公开的
//!   `call_value`（按值调用）与 `Instance::raise_builtin_error`（槽位实现要在 core 之外抛异常），
//!   以及 `py_object!` 的对外可用（`OM-14`：宿主类型要能定义自己的载荷）
//! - **槽位回调执行器**（`TS-44` 的另一半）：槽位可以回到执行器调用 Python 级覆写——
//!   `__del__`（终结器，`OM-20` ①）与**容器元素的 `repr`**（`repr([x])` 尊重 `x.__repr__`）
//! - **`class` 语句的落点**：`LOAD_BUILD_CLASS` ＋ `__build_class__`（原生，按实例存）＋
//!   帧的**命名空间形态**（类体／模块级的局部变量是**映射**，`LOAD_NAME`／`STORE_NAME`／
//!   `DELETE_NAME`）；类体跑完把命名空间搬进**类型字典**，基类走 C3，`__init_subclass__`
//!   钩子经属性通道调用（`OM-14` 的类创建面）
//! - 字节码 §10 的**星号调用与打包局部变量**：`CALL_FUNCTION_EX`（净 −3；栈是
//!   `[可调用, self|NULL, 实参 tuple, 关键字 dict|NULL]`）、`DICT_MERGE`／`DICT_UPDATE`
//!   （净 −1，前者覆盖、后者遇同名键要带 qualname 的消息 ⇒ 如实报未接线）、
//!   `LOAD_FAST_LOAD_FAST`／`LOAD_FAST_BORROW_LOAD_FAST_BORROW`（净 +2，**高 4 位先压**）
//! - **实例生命周期（`P1-7` 的对内一半）**：中断**按实例**请求（`CX-3`），执行器每条指令查一次
//!   ⇒ 中断后执行类函数"随即返回"（本层是 `ExecError::Interrupted`，ABI 面映射到
//!   `PA_ERR_INTERRUPT`）；实例隔离与"环由 GC 收"都有用例（`T-OM-4`）
//! - **`OM-11` 的 `new` 槽与类型对象的实例化**：`list()`／`dict()`／`int()`／`ValueError("x")`
//!   一类走类型自己的 `new` 槽，随后按 `OM-14` 从类型字典沿 MRO 找 `__init__` 并调用
//!   （"实例在先、实参在后"）——用户类的实例化与异常类的带参构造都落在这条路上
//! - **绑定方法**（`method` 类型）：`obj.method`（**不调用**）产出"函数 ＋ 绑定的 `self`"；
//!   调用它时绑定的实例自动当第一个位置实参。`obj.method()` 仍走编译器的取方法位
//! - **`OM-14` 的实例字典另行挂载**：布局固定的实例（宿主类型／`list` 一类的子类）把属性字典
//!   挂在 `Header` 的 `dict` 那一格上（头部 32 → 40 字节；取舍记录在 `header.rs`），
//!   与"内联在 `AttributeObject` 载荷里"的用户类并存，两者都走 `OM-11` 的 `getattr`／`setattr`
//!   通道；释放（`OM-20` ②）与遍历（`OM-36`）两处都已挂钩子
//! - 3.14 的 `LOAD_SMALL_INT`（实测净 +1：直接把 `oparg` 当小整数压栈，不走常量表）
//! - `BC-4` 其余 `co_*`：`co_code`／`co_exceptiontable`（要 `bytes` 类型）、
//!   `co_positions()`／`co_lines()`（要方法调用、tuple 迭代与行号表）
//! - 类型调用的带参构造只接了异常类与用户类；`list(x)`／`str(x)` 一类（要迭代／转换协议）
//!   现在会落进"`cannot create … instances`"，**消息与参照实现不同**，属于已知粗糙边
//! - **`pyawa-abi` 的对外出口**（`pa_create`／`pa_destroy`／`pa_interrupt` 与版本三件套）：
//!   见 `crates/pyawa-abi`；`pa_create` 的返回形状在 `docs/SPEC-c-abi.md` §15 只列了
//!   "`pa_create(const pa_host *)`、栈契约 `—`"，而 `AB-49` 要求返回值一律走状态码、
//!   新实例又没有栈（`AB-13`）——**这一处口径待裁**（见提交说明与报告）
//! - `class` 其余：`metaclass=`、`__prepare__`、`__set_name__`（要描述符）、`__mro_entries__`
//! - `OM-14` 其余：宿主对象的 `new` 槽位（**子类分派槽位**已随 `a30d4cc` 与 `P1-9` 落地）
//! - `repr`／`str` 其余：顶层 `repr(x)`／`str(x)` 与**容器元素**都已先走属性通道（`TS-44`）。
//!   **已知偏差**：覆写里抛出异常时本层**吞掉**并退回槽位路径（`object_repr` 的签名没有异常
//!   通道，见上一组），参照实现是向上传播；`float` 的边界写法。
//!   （绑定方法 `repr` 的 **qualname** 已落地：编译器产出 `co_qualname`、类创建钩子补写 `C.m`）
//! - `CALL_INTRINSIC_1` 其余：`ASYNC_GEN_WRAP`、`PRINT`、`IMPORT_STAR`，以及 PEP 695 那一组
//!   （`TYPEVAR`／`PARAMSPEC`／`TYPEALIAS`／`SUBSCRIPT_GENERIC`／`PREP_RERAISE_STAR`…）
//!   ——**实测卡在依赖上**：`type X = int` 产出的是 `typing.TypeAliasType`、泛型参数是
//!   `typing.TypeVar`，两者都在 `Lib/typing.py` 里、**不是内建类型**（`TS-41` 的探测表里没有，
//!   故不能另造一个类型顶替）。⇒ PEP 695 要等 `P3-14` 的 `Lib/typing` 先落地
//! - 星号调用其余：`DICT_MERGE` 的同名键错误（要函数的 qualname）、
//!   `CALL_FUNCTION_EX` 的映射协议（现在只认 `dict`）
//! - **编译器标注面**：注解的**解析与边界检查发射**已落地（`BC-25`②＋`TS-31`：扩展模式＋深层
//!   档位＋带注解 ⇒ 序言/返回前发 `CHECK_BOUNDARY_IN/OUT`，`list[int]` ⇒ 复合标签；端到端有验收）。
//!   **仍未落地**：`BC-25`①的"只在标注／未标注交界处发射"（要跨模块静态信息 ⇒ 现按带标注保守发射）、
//!   **PEP 649** 的注解对象一族（`__annotate__` ＋ `SET_FUNCTION_ATTRIBUTE`，参照实现 3.14 的形态）、
//!   `TS-32`…`TS-39` 的编译期检查器与覆盖率报告
//! - `sys` 的**其余**面（`stdout` 一族归 `_io`＝`fs` 域、路径一族要能力层、importlib 一族；
//!   见 `SPEC-c-modules.md` §5.2.3）——`getrefcount` 已随 `sys` 落地（`OM-22`）
//! - `_imp` 的**其余**面（导入锁、`.pyc` 源码哈希、扩展模块、冻结表；见 §5.2.4 的"未落地"）——
//!   `pyc_magic_number_token` 与 `is_builtin` 已落地
//! - `_opcode`／`_opcode_metadata` 的 Python 层包装**已落地**（转发本 crate 的探测表）；
//!   仍未实测的错误路径见 §5.2.5
//! - `itertools` **落地了 `count`／`repeat`／`islice`／`chain`**（§5.2.6）：`CountIteratorObject`
//!   只含整数、不持引用；`repeat`／`islice`／`chain` 走 `ItStateObject`（**持引用** ⇒ 挂
//!   `traverse`／`clear`）。两条边界（越过 `i64`、浮点实参）如实报 `Unsupported`；
//!   `chain.from_iterable` 与其余函数随后补
//! - 模式匹配族其余：`MATCH_CLASS` 的**位置形参**（本层只接了关键字形参）与"属性是方法"
//!   那一支（要绑定方法对象）、`MATCH_KEYS` 的 `__getitem__` 协议（现在只认 `dict`）
//! - 格式化族其余：迷你语言里**没实现**的写法（`n` 的本地化、数值的自定义填充细节、
//!   `.N` ＋ `g` 的组合等）在 `src/format.rs` 的文件头逐条列着，命中时如实报未实现
//! - 生成器族的其余面：`SEND` 只接线了**生成器**（普通迭代器那条随后补）、
//!   `GET_AWAITABLE`／`coroutine`／`async_generator`（`await` 那一半）、`CLEANUP_THROW`、
//!   生成器对象的方法（`send`／`throw`／`close`——要方法绑定与属性通道）、
//!   `CALL_INTRINSIC_1` 的 `STOPITERATION_ERROR` 与 `GeneratorExit`
//! - `except*`（intrinsic 族：`CHECK_EG_MATCH`／`INTRINSIC_PREP_RERAISE_STAR`）与
//!   `sys.exc_info()` 的 Python 可见形态（`with` 的 `BEFORE_WITH`／`WITH_EXCEPT_START` 已接线）
//! - `__traceback__` 的追加与 `lasti` 的还原（`BC-60` 点名的最后一条，要 traceback 对象）
//! - 异常对象的**其余** Python 可见面：`__notes__`／`__context__` 的显式赋值、
//!   traceback 一族（`str(e)` 与 `KeyError` 的单实参口径已落地：`str(KeyError('k'))` ⇒ `"'k'"`）
//! - `T-BC-22` 的**夹具对拍**面：`try`／`except`／`else`／`finally`／`with`／`except*` 的发射序列
//!   与可观察行为——表已经对拍（`tests/fixture-code-3.14.json`），派发用镜像骨架的手写用例锁住；
//!   逐程序的完整对拍要等 `MS-` 的 conformance harness
//! - 字节码 §10 的其余族：**只剩 PEP 695**（`§2.4` 的 `co_*`、生成器与协程、格式化、
//!   模式匹配都已接线——各自的"其余面"散见下面几条）
//! - **迭代协议**（`OM-11` 的 `iter` 槽位）：**已接线**——`GET_ITER` 对非内建可迭代对象走
//!   `__iter__`（属性通道），迭代推进走 `__next__`（`StopIteration` ⇒ 耗尽）；没有协议的给
//!   实测消息 `TypeError: 'X' object is not iterable`。**已知边界**：内建容器的**子类**没有
//!   可调用的 `list.__iter__` 一类内建方法 ⇒ 目前按"有 `__iter__` 才走协议"处理；`in` 的
//!   `richcompare` 那一摊仍见本条上方
//!
//! **类型槽位的形状**（`OM-11`）：`§10` 的注允许实现自选 Rust 签名；`AB-37` 要求
//! `pa_newtype` 带**子类分派槽**，而 `§13-2` 的改动矩阵说"事后追加槽位 ＝ 主版本 +1"——
//! 所以这套形状定下来就是长期契约，动手前值得过一眼。
//! - `TS-40` 数值塔的其余部分（`int`／`float`／`complex` 的互操作与提升）
//! - 字节码 §10 起步指令集的其余部分（控制流、调用、容器、属性与下标、异常、生成器、
//!   格式化、模式匹配、PEP 695）与 §11 的下降规则
//! - 字节码 §2.4 的 `co_*`：**只剩要 `bytes` 的那四个**（`co_code`／`co_exceptiontable`／
//!   `co_linetable`／`co_lnotab`）与 `co_branches()`；`co_name`／`co_qualname`／`co_positions()`／
//!   `co_lines()` 等都已接线并逐项对拍
//! - 对象模型 §10 弱引用：**OM-27** 的 ② "先清弱引用"目前只是顺序上的占位点
//! - **OM-28** `gc` 模块的可见行为（需要模块系统，不属本层）
//! - 对象模型 §11 宿主对象（**OM-34**…**OM-37**）、**OM-13** C3、**OM-14** 宿主类型注册
//!   **均已落地**（`AB-58`／`AB-59` 与 `P1-9`：载荷同尺寸、type 经栈传、实例字典另挂）
//! - **OM-11** 槽位表里**还缺三个**：`hash`／`richcompare`／`iter`（`getattr`／`setattr`／
//!   `call`／`repr`／`str` 都已接线；签名已定的那几个见 `Slots` 的公开面）
//! - **已知偏离 `OM-12`**：类型对象自身也会成环（`bases`／`mro`／`dict`），但**没有**标
//!   `GC_TRACKED`、也**没有** `traverse`／`clear`——`OM-12` 要求可成环的类型两者齐备。
//!   `alloc` 现在按"是否提供 `traverse`"判定入链，所以类型对象暂不入回收链表；
//!   补 `traverse`／`clear` 时**必须**同时补标记。
//!
//! **边界**
//!
//! - **OM-6**：本 crate 的内部表示（头部、句柄）**禁止**出现在 C ABI 签名里——那是 `pyawa-abi`。
//! - `DESIGN.md` §7 原则 5：本 crate 不依赖 `std::fs`／`std::net`／libc，不出现
//!   `#[cfg(target_os)]`（约束见 `CX-4`，检查实现见 `tests/ci/check.py`）。
//! - **OM-18**：**禁止**用 `Rc`／`Arc` 作对象引用。
//!
//! **排查中的一处缺陷（`FellOffEnd`）——四轮探针的净结果**（探针都已撤，不留调试代码）：
//!
//! - 现象：`class C: def __init__(self, v): self.v = v` ⇒ `C(9)` 报 `FellOffEnd`（跑到码外）
//! - 已排除：① 初始化器不是 `object.__init__` 的槽包装（探针：类型名是 `function`）；
//!   ② 函数对象形状无差异（`argcount`／`nlocals`／`flags`／free／cell／`globals` 全同）；
//!   ③ 不是"实例由 `type_call` 新建"造成的（用别的类的实例去调，一样失败）
//! - **判别完成**：分界是「**未绑定**调用 ＋ 体里写属性」这一组合 ——
//!   未绑定但体里**不写** `self`（`return v`）⇒ 通过；**绑定方法**里写属性（`self.x = 5`）⇒ 通过；
//!   未绑定且体里写属性 ⇒ `FellOffEnd`（`C(9)` 与 `C.__init__(that, 9)` 都是）
//! - **下一步**：对失败那次调用打印**帧的 ip 与 code 长度**，并与通过的绑定方法路径逐条对照
//!   （`bound` 那份引用、实参表、帧的 locals 布局）
//!
//! **（下一轮的精确假设）** 又排掉一层：`call_callable` 的 Python 函数路径读过——
//! `bound_self` 前插、`bind_arguments` 绑定、逐槽 `set_local`，而"两个形参的绑定"本身没问题
//! （`def __init__(self, v): return v` 经 `type_call` **通过**）。于是嫌疑缩到**一个**地方：
//! `self.v = v` 里的值来自**第二个形参**（`LOAD_FAST_BORROW 1`），而通过的用例
//! （`self.x = 5`、`return v`）一个用常量、一个不写属性 ⇒ 都没同时踩到"读第 2 槽 ＋ 写属性"。
//!
//! **下一轮第一步（很便宜）**：把体改成写**常量**——`def __init__(self, v): self.v = 5`
//!   - 通过 ⇒ 问题在**读第 2 个局部槽**（`LOAD_FAST_BORROW 1` 一族）
//!   - 仍失败 ⇒ 问题在"写属性 ＋ 且作用域里有第 2 个形参"这个组合（再看 `STORE_ATTR` 的栈序）
//!
//! **（第 96 轮：又缩一层，并把 `FellOffEnd` 的语义搞准了）**
//!
//! - `FellOffEnd` **不是**"跳到越界偏移"，而是执行循环的**兜底返回**（`executor.rs` 末行）：
//!   ip 一直走到**码尾**却没命中 `RETURN_VALUE` ⇒ 所以"写常量 vs 读形参"那一刀的结果是：
//!   `self.v = 5`（写常量）与 `self.v = v`（写形参）**都** `FellOffEnd`
//!   ⇒ 与"读不读第 2 个槽"**无关**，分界是「**两槽作用域** ＋ 写属性」这一组合
//!   （1 个形参 ＋ 写属性通过、2 个形参 ＋ 不写通过、2 个形参 ＋ 写失败）
//!
//! **下一轮第一步（对准"没命中 RETURN_VALUE"）**：把失败帧的**指令轨迹**打出来
//!   （每条执行到的指令名 ＋ ip），看它是**跳过了收尾两条**，还是**从中间某条直接落到尾**。
//!   同时打印该 code 的 `code().len()` 与最后几条指令，确认收尾两条**确实在**（语料已证明编译产物有）。
//!
//! **（第 99 轮：拿到执行轨迹，问题性质变了）**
//!
//! 在执行循环里加了临时 `PYAWA_TRACE` 打点（跑完即撤，`grep` 校验为 0），失败帧的轨迹是：
//!
//! ```text
//! TRACE op=128 off=0 size=1     （RESUME）
//! TRACE op=94  off=1 size=1
//! TRACE op=86  off=2 size=1
//! TRACE op=110 off=3 size=5     （STORE_ATTR，带 4 格内联缓存）
//! RESULT Err(FellOffEnd)        ← 到这里就"没有下一条"了
//! ```
//!
//! 而**同一份源码**加进语料后，我们的产物与参照**逐字节一致**（含收尾的 `LOAD_CONST None;
//! RETURN_VALUE`）⇒ 所以不是"编译少了尾两条"，而是**执行时那个帧拿到的 code 与编译产物不同**。
//! 头号嫌疑：**类创建钩子**为修 `co_qualname` 而"换一份 code ＋ 新函数"（`DIV-3` 那条路，
//! `instance.rs` 里第二个 `CodeObject::new` 调用点）——它拷的是 `code.code().to_vec()`，
//! 但**带内联缓存的指令**在这里会不会被截短，要下一轮印 `code().len()` 与最后两条指令对照。
//!
//! **下一轮第一步**：同一份源码分别印「模块里那个函数」与「类字典里的 `__init__`」的
//! `code().len()` ＋ 最后两条指令字节——两者不一致就落在钩子的拷贝上。
//!
//! **（第 100 轮：根因抓到了，而且不在运行期）**
//!
//! 直接印两边的字节：
//!
//! ```text
//! 单元里的类体: name=C code_len=38
//!   单元里的方法: name=__init__ code_len=16 tail=[0,0,0,0,0,0,0,0]   ← 编译器产物就少了尾两条
//! 模块里的 plain: code_len_bytes=6  tail=[128,0,86,0,35,0]           ← 正常
//! 类字典里的 __init__: code_len_bytes=16 tail=[0,0,0,0,0,0]         ← 与单元一致 ⇒ 运行期没动手脚
//! ```
//!
//! ⇒ **根因**：对 `class C: def __init__(self, v): self.v = 5` 这个源码，发射器**没发**函数收尾的
//! `LOAD_CONST None; RETURN_VALUE`（16 字节＝RESUME／LOAD_SMALL_INT／LOAD_FAST_BORROW／
//! STORE_ATTR ＋4 格缓存，正好缺尾巴）。而"1 个形参 ＋ 写属性"（`m(self)` 内 `self.x = 5`）是好的
//! ⇒ 待查的是发射器的收尾条件在**哪种语句／形参组合**下不成立。
//!
//! ⚠ **同时暴露一个夹具问题**：这条语料已经进了 `tests/compile.rs` 的对比，却**没红** ✗
//! ⇒ 要么夹具的 `nested` 递归没覆盖到这一层，要么这条用例没真的被比。**下一轮先查夹具**，
//! 再修发射器（否则修完也没人守）。
//!
//! **（第 101 轮：修好了）**
//!
//! 根因是 `compile_scope` 的**函数收尾分支**：它只在常量表空着时**登记**一个 `None`，
//! **从不发** `LOAD_CONST None; RETURN_VALUE` ⇒ 任何"能落到末尾"的函数（＝没有显式 `return`
//! 的函数）执行时都必然 `FellOffEnd`。之前通过的用例**全都**有显式 `return`，所以一直没露头。
//!
//! 修法与守卫（两处都按实测）：
//! - 末尾不是 `return` 时补 `LOAD_CONST None; RETURN_VALUE`（位置取最后一条真指令的跨度）；
//!   `if/else` 两分支都 return 的情形仍由 `epilogue_needed` 覆盖
//! - **保留**"函数没有常量时仍登记一个 `None`"这条旧规则（`def f(**kw): return kw` ⇒ `['None']`），
//!   它与补不补尾两条**无关**
//! - 端到端：`P(9)` ⇒ `__init__(self, v)` ⇒ `self.v == 9` ⇒ `get() == 9`（`compiled_class.rs`）
//!
//! ⚠ **夹具盲区已补**：`tests/compile.rs` 原先用 `zip` 逐条比指令 ⇒ **少了指令也不会红**。
//! 现在先断言**指令条数相等**再比（这条断言当场抓出了上面"末尾是 return 仍补尾"的错误一轮）。
//!
//! **（第 102 轮：又发现一个真特性，已记档待做）** 给"隐式返回"上锁时，我把
//! `class C: def m(self): self.x = 1` 加进语料，夹具立刻报：类体的 `__static_attributes__`
//! 参照给的是 `('x',)`，而我们**恒发 `()`** ✗ ⇒ 3.14 会**静态收集**"方法里写过 `self.X` 的
//! 那些名字"作为类体的 `__static_attributes__`（不是运行时收集）。
//! ⇒ 这是一条**新特性**（不是缺陷）：要做"类体里扫描各方法的 `self.X = …` 赋值，按参照的
//! 去重／排序规则生成那个元组"。那条语料因此**暂时没入库**（等特性做完再进，免得红着），
//! 其余四条"能落到末尾的函数"已入库并由新的条数断言守住。
//!
//! **（第 102 轮续）语料给隐式返回上锁**：新增四条"能落到末尾"的函数（`x = 1` 体／带形参／
//! 带文档串／`if x: return 1`）——有了夹具的条数断言，这类漏发以后再出现**必然变红**。
//! 其中 `if x: return 1` 的**尾条位点**与参照不同（参照记在**那条 `if` 语句**的跨度上，本层用
//! 最后一条真指令的位点）⇒ 按既有机制标"位置表未对齐"并写明理由（不假装对齐）。
//!
//! **（第 103 轮：做完了）`__static_attributes__` 的静态收集已落地**（实测规则）：
//! 只收**赋值**形态 `self.名字 = …`（只读不算）、**字母序去重**、类体层的普通赋值不算、
//! **方法里嵌套函数中的赋值也算**。实现在 `compile_class_scope` 的收尾（`Constant::Tuple`），
//! 语料两条（单方法／两方法＋乱序）＋ 端到端（`compiled_class.rs` 读类字典里的那个元组）。
//! **尚未接线**：`if`／`while`／`for` 体里写 `self.X` 的收集（未实测，照实留着）。
//!
//! **（第 104 轮续）** 上一条"尚未接线"已补掉：`if`／`while`／`for` 的体（含各自的 `else` 体）
//! 现在也走进收集（实测 `if x: self.a = 1` ⇒ `('a',)`、`for i in xs: self.b = i` ⇒ `('b',)`），
//! 语料两条（`if` 体／`for` 体）＋ 上一轮的端到端一起守着。
//!
//! **（第 105 轮：撞上规格缺口，已上报）`chain.from_iterable` 卡在类型调用的错误通道**
//!
//! - 参照的 `chain` 是**类型对象**（`type(chain(...)).__name__ == 'chain'`），类方法
//!   `from_iterable` 挂在它上面；而"把一个不可迭代的东西传进来"必须报
//!   `TypeError: 'int' object is not iterable`
//! - 本层 `Slots` 的 `NewFn` 签名是 `unsafe fn(…) -> Option<NonNull<Header>>`：**没有异常通道**
//!   ⇒ 只能返回 `None`，而那会被 `type_call` 解释成"不能创建该类型的实例"（消息与参照不同）
//! - **两条路**：①给 `NewFn` 加结果通道（`Result<_, ExecError>` 或"错误格"）⇒ 改槽位签名 ＋
//!   约六处 `with_new` 调用点 ＋ 一处 `type_call` 的错误传播；②让 `chain` 继续当**函数**、
//!   额外挂 `from_iterable` 属性 ⇒ 不动槽位签名，但 `type(chain(...)).__name__` 与参照不同，
//!   要记一条 `DIV-`
//! - **倾向**：①（把语义对齐，不新增差异）；代价中等且集中在槽位签名一处。**等裁定**。
//!
//! **（第 107 轮）`__set_name__` 的参照实测（oracle 数据，实现待做）**
//!
//! 前置已齐（类体、属性读写、方法调用都通了），实测行为如下：
//!
//! - 对**本类自己命名空间**里的每一项（**按插入序**），若它有 `__set_name__` 就调用一次，
//!   参数是 `(类对象, 属性名)`：`class C: d = D(); e = D()` ⇒ `[('C', 'd'), ('C', 'e')]`
//! - **继承来的项不再调**（`class Sub(Base)` 不会对 `Base.b` 再调一次）
//! - 没有 `__set_name__` 的项**跳过**（类体里定义的函数就不会被调）
//! - `__set_name__` 抛错**原样传播**出类创建（实测 `ValueError: boom` 直接冒出来，不包装）
//! - 内建 `property` **也带** `__set_name__`（将来接 `property` 时靠它拿名字）
//!
//! **实现落点**：类创建钩子（`classes.rs`）把命名空间搬进类型字典之后、返回类对象之前，
//! 按插入序遍历并调用；本轮剩余预算不足以安全摸清 dict 的遍历 API ＋ 调用链，故先落数据。
//!
//! **（第 108 轮）实现前摸到的 API 与还差的一格**：
//!
//! - 落点已选定：`classes.rs` 的类创建钩子里，`entries = mapping.entries()`（**按插入序** ✓）
//!   那个循环里就能顺手做 `__set_name__`（`for (key, value) in entries` 之后、`insert_raw` 前后）
//! - 取属性：`executor::attribute_optional(instance, object, name)`（可选取 ✓）
//! - 调用：`executor::call_value(instance, callable, args, kwargs)` ✓
//! - 报错：`Instance::raise_builtin_error(name, message) -> ExecError` ✓，读侧 `current_exception()` ✓
//! - **还差的**：类创建这一路（`build_class_native -> NonNull<Header>`）怎样把**用户代码抛出的
//!   异常**挂成"当前异常"（读侧有、写侧的入口没找到；`raise_builtin_error` 只造内建错误）。
//!   找到它就能一次做完：按插入序取 `__set_name__`（没有就跳）⇒ 调 `(类, 名字)` ⇒ 原样传播。
//!   本轮预算不足以把这一格摸准，**不硬塞**（宁可留精确的下一步，也不留半成品）。
//!
//! **（第 109 轮：`__set_name__` 已落地）** 上一轮以为缺的"异常写侧"其实不缺——
//! `build_class_native` 本身就返回 `Result<NonNull<Header>, ExecError>` ⇒ 用户异常可以
//! **原样 `Err` 传播**。实现放在类命名空间逐项 `insert_raw` **之后**（保持插入序、字典已就位），
//! 用 `attribute_optional` 探测 `__set_name__`（没有就跳过），用 `call_value(它, [类, 键])` 调用。
//! 端到端：`class C: d = D(); e = D()` ⇒ 两处 `self.name` 分别是 `d`／`e`（`compiled_class.rs`）。
//! **尚未加用例（实测已确认、待补测试）**：继承来的项**不重调**（结构上保证：只遍历本类命名空间）、
//! 抛错**传播**（结构上保证：`Err` 直接返回）。
//!
//! **（第 110 轮）两条承诺的用例已补**：
//! - **继承项不重调**：`Base.b = D()` ＋ `class Sub(Base)` ⇒ `Base.b` 那个描述符的 `owner`
//!   仍是 `Base`（若被重调会被覆写成 `Sub`）
//! - **抛错传播**：`__set_name__` 体里读未定义的全局名 ⇒ 类创建以 `Raised { NameError }` 结束
//!   （当时用读未定义名字而不是 `raise`，因为那时 **`raise` 还没接线**；现在 `raise` 已全链落地
//!   （含 `from`）⇒ 这条测试随时可以改用 `raise`）
//!
//! **（第 111 轮）`raise` 的勘察结果（本轮未落地，代码已整体回退）**：
//!
//! - **执行器那一半现成**：`RAISE_VARARGS` 已接线（`executor.rs` 里那条 oparg 只能 0／1／2）
//! - **参照形状**：`raise ValueError(1)` ⇒ `LOAD_NAME ValueError; PUSH_NULL; LOAD_SMALL_INT 1;
//!   CALL 1; RAISE_VARARGS 1` ⇒ oparg 1＝带值、2＝带因、0＝裸重抛
//! - **编译器那半我已写出来并跑过**（词法 `raise`、AST、解析含 `from`、发射、语料两条），
//!   指令与常量**对上了**，卡在两处**未验证**的细节上，故整笔回退、不留半成品：
//!   ① 模块收尾两条的位点：参照记在**当条 `raise` 语句**的跨度上，本层取的是模块整段；
//!   ② 端到端测试要先把**内建装进实例**（`ValueError` 这类名字才解析得到），否则先报 `NameError`
//! - **下一轮要做**：补 ①（收尾位点那格）＋ ②（测试装内建），再把上面那套实现重新落盘，
//!   语料两条 ＋ 端到端一条一起进，全闸门后提交
//!
//! **（第 112 轮）再试一次 `raise`，又抓到两个精确 blocker（代码再次整体回退）**：
//!
//! 本轮按上一轮的勘察**先 grep 原文再改**（锚点全部命中 ✓），实现后夹具给出两条**新的**差异：
//!
//! 1. **函数里对全局名发调用**，参照用 `LOAD_GLOBAL` 的**低位（NULL 位）**：
//!    `def f(): raise ValueError(1)` ⇒ 参照 `LOAD_GLOBAL 1; LOAD_SMALL_INT 1; CALL 1`（**没有**
//!    单独的 `PUSH_NULL`），而本层发 `LOAD_GLOBAL 0; PUSH_NULL; …` ✗ ⇒ 说明"压 NULL"要么由
//!    `LOAD_GLOBAL` 的低位承担、要么由 `PUSH_NULL` 承担，**取决于被调用者的形态**——
//!    这块要改 `Call` 的发射路径（知道被调用者是不是全局名）
//! 2. **模块收尾两条的位点**：参照记在**当条语句**的跨度上（`raise` 那条），本层取模块整段 ✗
//!    （我在 `Statement::Raise` 里设了 `epilogue_span` 仍不生效 ⇒ 模块收尾用的**不是**这个字段，
//!    下一轮先找到它真正读的是哪个）
//!
//! ⇒ 这两条都不是 `raise` 本身的问题，而是**两条通用规则**（调用位的 NULL 承担者、模块收尾位点），
//! 做任何新语句都会撞上 ⇒ 值得单独一轮专门啃。代码已回退，工作区绿。
//!
//! **（第 113 轮）"模块收尾位点"这条规则的矛盾点已定位到行号**：
//!
//! - `compile.rs:619` 就是模块收尾那两条的发射点：`let tail = emitter.epilogue_span;`
//!   ⇒ **它读的确实是我改的那个字段**
//! - 但 `raise` 那一笔里我在 `Statement::Raise` 臂中先把 `epilogue_span` 设成 `raise` 语句的
//!   跨度，夹具仍报模块整段 ⇒ **赋值在 619 之前被覆盖了**（`epilogue_span` 在文件里有 8 处赋值：
//!   `382`／`518`／`856`（赋值语句）／`890`（返回）／`984`（`for` 目标）／`1032`（`if` 条件）／
//!   `1103`／`1139`／`1271`）
//! - **下一轮一行探针就能定案**：在 619 之前 `eprintln!` 打印 `emitter.epilogue_span` 与
//!   `emitter.last_span`，跑那条 `raise ValueError(1)` 看究竟是哪个写入最后落地
//!   ⇒ 之后要么改那个写入者、要么把模块收尾的取法换成 `last_span`（视证据定，不猜）
//!
//! **本阶段纪律记一笔**：113 轮里有 3 轮（111／112 与本轮）是零净增的诊断轮。我选择**不**在
//! 预算不足时开第 4 个探针，而是把"矛盾点＋一行探针"写死在这里 —— 下一轮可一步定案。
//!
//! **（第 119 轮）`operator` 第一刀实现前的 API 结论（都已问清，实现留一笔）**：
//!
//! - 造 native：`BuiltinFunctionObject::new(ty, Box::leak(name…), Cell::new(handler))`，
//!   类型名 `builtin_function_or_method`（照 `itertools_module.rs` 的 `make_native`）
//! - **相等**：`executor::values_equal_public(instance, left, right) -> bool`（公开入口 ✓）
//! - **布尔返回值**：`Instance::new_bool(bool) -> NonNull<Header>` ✓
//! - 报错：`Instance::raise_builtin_error(name, message) -> ExecError` ✓
//! - ⚠ **`truthiness` 目前是私有的**（`executor.rs:1145`，`fn` 不是 `pub fn`）⇒ 所以
//!   `truth`／`not_` **不能**在 stdlib 里做（复制一份真值规则＝两处真相，`AGENTS.md` 禁止）
//!   ⇒ 要么先给核心开一个公开入口（一小笔），要么第一刀就只做 `eq`／`ne`／`is_`／`is_not`
//! - 本轮我把模块文件写出来过（`eq`／`ne`／`is_`／`is_not` ＋ 实测消息），但**没注册、测试还是
//!   占位** ⇒ 按"不留半成品"**删掉了**；生成脚本、夹具与 §5.2.7 合约**都已在库**，
//!   下一笔照上面四条 API 直接写完即可
//!
//! **（继续后的第 1 轮）收口一处"指令未验证"的洞**：第 78 轮给三条语料（`return g`／`return g(1)`／
//! `return a + g`）标的是 `covered=False`＋"位置表未对齐"——我当时的本意只是**豁免位点**，
//! 但 `covered` 这个标志会把**指令比对整个跳过** ⇒ 那三条其实一直"只验证了能编过" ✗。
//! 本轮把它们翻回 `covered=True`（理由保留"位置表未对齐" ⇒ 只豁免位点）：
//! **夹具仍然全绿** ⇒ 说明这三条的**指令流（含 `LOAD_GLOBAL`／`PUSH_NULL` 的形态）本来就对** ✓，
//! 也顺带修正了我第 112 轮的推断——那个 `LOAD_GLOBAL` 低位差异**不是**通用规则，
//! 只出现在 `raise` 那个形状里（待 `raise` 落地时再一起看）。
//!
//! **（第 121 轮）把"标志用错"的洞扫全了**：第 78 轮起给 8 条语料用的 `covered=False`＋
//! "位置表未对齐"，本意只是豁免位点，实际把**指令比对整个跳过** ✗。本轮把 8 条全翻回
//! `covered=True`（理由保留 ⇒ 仍由 `positions_covered` 豁免位点）：三条 `return g…` ＋
//! 五条带注解的（理由写的是"**注解单元**的位置表未对齐"）——**夹具全部保持绿** ✓
//! ⇒ 结论：这 8 条的**指令流本来就对**，此前只是"没在比" ✗。
//! **现在的口径**：语料里**每一条**都进指令比对；位点豁免只由 `positions_covered`（＋理由）承担。
//! ⇒ 顺带修正了第 112 轮的推断：`LOAD_GLOBAL` 低位那个差异**不是**通用规则，只在 `raise` 形状里。
//!
//! **（第 122 轮）`operator` 第一刀已落地**（`pyawa-stdlib` 的 `operator_module`）：
//! `eq`／`ne`／`is_`／`is_not`／`truth`／`not_` 六个 —— 相等走 `values_equal_public`、
//! 真值走**新开的** `truthiness_public`（原先 `truthiness` 是私有的 ⇒ stdlib 用不了；
//! 复制一份规则会成两处真相，故给它开了公开入口）、身份是**指针相等**。
//! 消息照实测原文（`eq expected 2 arguments, got 1`／`_operator.truth() takes exactly one
//! argument (2 given)`），验收是**模块自带的单元测试**（不依赖测试脚手架）。
//! **写测试时顺手钉住两件事**：① 小整数是单例（`OM-23`）⇒ `is_(1, 1)` 为真；
//! ② 当时容器的"值相等"还没接线；**现在已接线**（`list`／`tuple` 递归、`dict` 按键匹配、
//!   `set` 双向包含），仍等 `OM-11` 的是容器的**序**比较
//! ⇒ 所以值相等那条用**字符串**验，别拿 `[]` 去断言。
//!
//! **（第 123 轮）`operator` 第二刀（`lt`／`le`／`ge`／`gt`）撞上"通用比较还没落地"**：
//!
//! - 核心的 `COMPARE_OP`（`executor.rs:5940`）现在**只对整数**比较：它先把两边 `as_int(...)`
//!   ⇒ `str`／浮点／容器的比较根本没有入口 ✗
//! - 所以第二刀**不是**"照 `truthiness_public` 那样开个公开入口"就行：要先把**通用比较**
//!   落地（一处实现，`COMPARE_OP` 与 `operator` 共用 —— 两条路各写一份就是两处真相 ✗）
//! - **参照口径已实测**（供实现时用）：`lt(1,2)=True`／`lt(2,1)=False`／`le(1,1)=True`／
//!   `ge(1,1)=True`／`gt(1,2)=False`；不可比时
//!   `TypeError: '<' not supported between instances of 'int' and 'str'`；
//!   参数个数不对：`lt expected 2 arguments, got 1`
//! - **下一笔的建议切法**：先做"通用比较"本身（`int`／`bool`／`str`／浮点按值，其余按身份或
//!   报实测消息），把它同时接到 `COMPARE_OP` 与 `operator` 上；语料侧补字符串/浮点比较的用例，
//!   逐字节验证（这正是把"比较"从"整数专用"变成"通用"的一跳，`TS-40` 里排得上）
//!
//! **（第 124 轮）"通用比较"已落地**（`TS-40` 的一跳）：新增公开入口
//! `executor::compare_public(instance, left, right, symbol, opcode) -> Result<bool, ExecError>`——
//! `==`／`!=` 走与从前同一套（`values_equal_public`），大小比较对 `int`／`bool`／`str` **按值**，
//! 其余类型报**参照实测**的 `TypeError`（`'<' not supported between instances of 'int' and 'str'`）。
//! **`COMPARE_OP` 也改走它**（一处实现两处用；整数路径行为不变 ⇒ 已有语料不受影响），
//! 从此非整数比较不再被当成"未接线" ✗。
//! **`operator` 第二刀随之落地**：`lt`／`le`／`ge`／`gt`（与 `COMPARE_OP` 共用同一份规则），
//! 生成脚本扩了五条实测结果与两条消息，测试改成**夹具驱动**（`RESULTS` 逐行核 ＋ 名字表 57 个）。
//! **顺手修掉生成脚本一个潜伏 bug**：它把 Python 的 `True`／`False` 直接写进 Rust ✗（夹具此前
//! 没被 `mod` 进来过所以没暴露）⇒ 现在输出 `true`／`false`。
//! **仍未接线**：浮点（本层浮点还没落地）与容器的**序**比较（`OM-11` 的 `richcompare` 槽位）。
//!
//! **（第 125 轮）容器的"值相等"已接线**（实测口径）：`values_equal` 现在对
//! `list` 与 `list`、`tuple` 与 `tuple` **递归逐项**比；**不同种类**一律不等
//! （`[1] == (1,)` ⇒ `False`）；长度不同直接 `False`；嵌套（`[[1]] == [[1]]`）也对。
//! ⇒ `==`／`!=`／`in`／`operator.eq` 一起受益（都是同一份 `values_equal`）。
//! **仍未接线**：`dict`／`set` 的比（等 `OM-11` 的 `richcompare` 槽位 —— 文档里那条老缺口），
//! 以及浮点（本层浮点还没落地）。
//! 写测试时又碰到 stdlib 的 `#![forbid(unsafe_code)]`（连测试里也禁）⇒ 改用**安全包装**
//! `Instance::retain(...)` 来给容器一份引用（`new_list`／`new_tuple` 是**接手**语义）。
//!
//! **（第 126 轮）`dict`／`set` 的值相等也接线了**（实测口径）：`dict` 要求"长度相等 ＋ 每个键
//! 在右边**按键值相等**找得到、且对应值递归相等"（`{1:2} == {1:3}` ⇒ 假、`{1:2} == {2:1}` ⇒ 假）；
//! `set`／`frozenset` 要求"长度相等 ＋ 左的每一项在右里找得到"（双向包含由长度＋单向推出），
//! 跨 `set`／`frozenset` 也照 Python 允许（`{1} == frozenset({1})` 为真——这条是照语言语义写的，
//! 未逐条实测）。**不同种类**一律不等（`[1] == {1}` ⇒ 假）。
//! ⇒ 至此 `int`／`bool`／`str`／`list`／`tuple`／`dict`／`set` 的值相等都通了，
//! `==`／`!=`／`in`／`operator.eq` 共用这一份。**仍未接线**：浮点（本层浮点还没落地）。
//!
//! **（第 127 轮）"模块收尾位点"查清了——不是通用规则问题，是我自己的编辑静默失效**：
//!
//! - 模块收尾（`compile.rs:619`）读的确实是 `emitter.epilogue_span`，而这条字段有一个**约定**：
//!   **每条语句臂在发射完自己那串指令后要把它设成自己的跨度**（`Assign`／`Class`／`For`／`If`／
//!   `Def` 各自那几处 `epilogue_span = …` 就是这个约定）；没设的话就落在**模块默认值**上
//!   （初始化在 `compile_scope` 的 `epilogue_span: …`）。
//! - 第 112 轮我在 `Statement::Raise` 臂里**写了**这一行，但它**没生效** ✗ —— 因为我当时的自校验
//!   写的是 `assert 'self.epilogue_span = *span;' in text`，而这句字符串在文件里**本来就有**
//!   （别的手臂里）⇒ **断言形同虚设** ✗ ⇒ 编辑静默 no-op，收尾两条于是落在模块默认跨度上，
//!   与观察到的 `(1, 1, 0, 19)`（整段）**完全吻合** ✓。
//! - ⇒ 结论：**没有待查的通用规则**；`raise` 落地时只要真的把那一行设上（并用**唯一锚点**改、
//!   改完 `grep` 出那一行回读）即可。这条"自校验要用**唯一**证据"的教训值得记牢。
//!
//! **（第 128 轮）`raise` 的编译器那半已落地**（执行器的 `RAISE_VARARGS` 早就现成）：
//! 词法 `raise`、AST `Statement::Raise`、解析（含 `from`）、发射（oparg 0／1／2）＋
//! `epilogue_span`（这次用**唯一锚点**改，并**回读该臂自身**确认那行真的在 ✓）。
//!
//! **两处差异按实情标注（不是假装对齐）**：
//! - 模块级 `raise ValueError(1)`：指令／常量／名字都逐字节一致 ✓，但 `RAISE_VARARGS` **那条
//!   自身的位点**与参照不同（参照 `(1,1,6,19)`，本层取到模块整段）⇒ 语料标"位置表未对齐"
//! - 函数级（`def f(): raise ValueError(1)`）：**指令流**就不同 ✗，是那条**已知规则**——
//!   函数里对全局名发调用时，参照把「压 NULL」放在 `LOAD_GLOBAL` 的**低位**（`LOAD_GLOBAL 1`，
//!   无 `PUSH_NULL`），本层发 `LOAD_GLOBAL 0; PUSH_NULL` ⇒ 语料标 `covered=False` ＋ 写明规则，
//!   等专门一轮修 `Call` 的发射路径
//! - `raise X from Y`：本层在这一支把 `None` 当**名字**了（`names` 多出 `None` ✗）⇒ 该语料
//!   **暂未入库**，与"两条通用规则"同批处理
//! - **端到端测试还没加**：测试实例没有装内建（`ValueError` 解析不到，会先报 `NameError`）
//!   ⇒ **已办**（第 130 轮）：`tests/compiled_class.rs` 的端到端用例先 `set_builtins` 装一个最小的
//!   `ValueError`，`raise ValueError(1)` 的端到端因此跑通 ✓
//!
//! **（第 129 轮）那条"已知规则"修掉了**：`Call` 的发射在**函数作用域里、被调用者是全局名**时，
//! 改发 `LOAD_GLOBAL <下标 << 1 | 1>`（"压 NULL"由**低位**承担）且**不发** `PUSH_NULL`；
//! 其余形态（模块级 `LOAD_NAME`／本地名／属性调用）照旧 —— 实测依据是
//! `def f(): raise ValueError(1)` 的参照字节码（`LOAD_GLOBAL 1; LOAD_SMALL_INT 1; CALL 1`）。
//! ⇒ 那条语料的**指令流现在逐字节一致** ✓；只剩**位点**不同（参照把 `raise <调用>` 那几条记在
//! **被调用者**的跨度上）⇒ 按实情标"位置表未对齐"。
//! 另外：`raise X from Y` 那一支本层把 `None` 当名字（`names` 多出 `None`）⇒ 语料暂未入库；
//! **（第 193 轮更新）端到端测试已加** ✓ —— `tests/compiled_class.rs` 的 `raise_propagates_the_user_
//! exception`：先 `set_builtins` 装一个只放 `ValueError` 的 dict ⇒ `raise ValueError(1)` 执行以
//! `Raised { ValueError }` 结束 ✓（此后 `tests/constructors.rs` 也照同一套路装 `int`／`bool`）
//!
//! **（第 130 轮）`raise` 端到端通了**：`raise ValueError(1)` ⇒ 执行以 `Raised { ValueError }` 结束
//! （`tests/compiled_class.rs`）。测试里**先给实例装内建**（`set_builtins` ＋ 一个只放 `ValueError`
//! 的 dict） —— 否则 `ValueError` 解析不到、会先报 `NameError`（第 111 轮踩过这一点）。
//! **`raise X from Y` 仍未入库**：本层在该支把 `None` 当**名字**（`names` 多出 `None` ✗），
//! 而表达式路径眼下没有 `None`／`True`／`False` 的**常量**形态 ⇒ 先把这三个字面量做成常量
//! （`LOAD_CONST`）才能收这一支；这是"字面量补全"的一个小切片。
//!
//! **（第 131 轮）`None` 成了**常量**（`Expression::Constant`）**：实测 `x = None` ⇒ 常量表
//! `['None']`、`LOAD_CONST 0` ⇒ 解析器在名字分支里把 `None` 直接产出为常量表达式；
//! 发射就是 `LOAD_CONST <下标>`。语料三条（`x = None`／`y = None; z = None`／
//! `raise ValueError(1) from None`）**逐字节一致** ✓ ⇒ `raise X from Y` 那一支的 `names`
//! 不再多出 `None` ✓。
//! **仍未做**：`True`／`False` —— 它们也要走 `LOAD_CONST`（实测常量表 `['True', 'None']`），
//! 但需要给 `Constant` 加一个 `Bool` 变体（现在只有 None／Int／Str／Code／Names／Type／Tuple）；
//! 顺带这也能修掉 `x = None` 之前那种"当名字读"的运行时 `NameError` 隐患。
//!
//! **（第 132 轮）`True`／`False` 也成了字面量常量**：给 `Constant` 加 `Bool(bool)` 变体 ⇒
//! `instantiate_constant` 映射到**单例**（`retain(singletons().boolean(…))`）、`.pyac` 的代码段
//! 加一个标签（`7`，`u8` 载荷）、测试渲染器按生成器的拼写输出 `bool:True`／`bool:False`、
//! 解析器把 `True`／`False` 产出为常量。语料两条（`x = True`／`x = False`）**逐字节一致** ✓。
//! **运行期**用例：`x = None; y = True; z = False` ⇒ 三者绑到的都是**单例** ✓
//! （顺带关掉"字面量被当名字读 ⇒ 运行时 `NameError`"的隐患）。
//!
//! **（第 133 轮）`operator` 的算术族落地**（`add`／`sub`／`mul`）：核心新开
//! `executor::arithmetic_public(instance, left, right, symbol, opcode)` —— 整数用 `checked_*`
//! （`i64`，越界如实报未接线）、非整数报**参照实测**的
//! `TypeError: unsupported operand type(s) for +: 'int' and 'str'`。
//! 生成脚本扩了三条结果与两条消息；夹具的 `RESULTS` 第四列改成 `i64`（布尔记 1／0，
//! 因为算术行的期望是整数）⇒ 测试按**名字分派**：比较族看 `bool_value`、算术族看 `int_value`。
//! **本层算术仍是"整数专用"**：浮点没落地；字符串的 `+`（拼接）与列表的 `+` 也还没接线
//! （参照的 `add` 对它们都有定义）⇒ 这些走"非整数"分支报消息。
//!
//! **（第 134 轮）`operator` 第四刀**：`floordiv`／`mod`／`pow` —— 仍走核心那一份
//! `arithmetic_public`（本轮给它加了 `//`／`%`／`**`）：整数 `checked_div_euclid`／
//! `checked_rem_euclid`／`checked_pow`（指数先转 `u32`，负指数如实报未接线）；
//! **除零**照参照实测报 `ZeroDivisionError: division by zero`（`//` 与 `%` 一样）。
//! 生成脚本加三条结果与一条消息；夹具 `RESULTS` 仍按名字分派。
//! **仍限整数**：浮点（`truediv` 与负指数都要它）没落地 ⇒ 这两个还没做。
//!
//! **（第 135 轮）`operator` 第五刀**：一元四个（`neg`／`pos`／`abs`／`invert`，走核心新开的
//! `executor::unary_public`）＋ 位运算五个（`and_`／`or_`／`xor`／`lshift`／`rshift`，扩进
//! `arithmetic_public`）。**边界照实测**：负移位报 `ValueError: negative shift count`；
//! 一元对非整数报 `TypeError: bad operand type for unary -: 'str'`；移位用 `checked_shl/shr`
//! （越界如实报未接线）。
//! ⇒ `operator` 现在落地 **33** 个函数（比较 10 ＋ 算术 6 ＋ 一元 4 ＋ 位运算 5），
//! 全部与核心**共用同一份实现**（没有一处重复规则）。
//! **仍未落地**：`truediv`（要浮点）、`matmul`／`getitem` 一族（要协议槽位）、
//! `itemgetter`／`attrgetter`／`methodcaller`（要类体与闭包，类体已通）。
//!
//! **（第 136 轮）`operator` 补上 3.14 新增的 `is_none`／`is_not_none`**（身份判定，
//! 与 `is_`／`is_not` 同一口径：只跟 `None` 单例比）⇒ 本模块累计 **27** 个函数。
//!
//! **（第 137 轮）`operator` 再补两个**：`inv`（`invert` 的**别名**，同一个实现）与
//! `index`（`int`／`bool` 的整数载荷原样给回，其余照实测报
//! `TypeError: 'str' object cannot be interpreted as an integer`）⇒ 累计 **29** 个函数。
//!
//! **（第 138 轮）`operator.contains` 落地**：核心把私有的 `contains` 开了公开入口
//! `executor::contains_public(instance, container, item, opcode)`（与字节码 `CONTAINS_OP`
//! **共用同一份实现**）；**参数顺序照参照**：`contains(容器, 项)`（我第一版写反了 ✗）。
//! 不可迭代时照实测报 `TypeError: argument of type 'int' is not a container or iterable`。
//! ⇒ `operator` 累计 **33** 个函数。
//!
//! ---
//!
//! **本阶段小结（第 121–139 轮落地的东西，按编号列清，便于接手）**
//!
//! **语言面**
//! - `raise` 全链：词法／AST／解析（含 `from`）／发射（`RAISE_VARARGS` 0／1／2）＋ 端到端
//!   （测试侧要先 `set_builtins`）
//! - 字面量常量化：`None`／`True`／`False` 走 `LOAD_CONST`（新增 `Expression::Constant` 与
//!   `Constant::Bool`，含 `.pyac` 标签 7、渲染器 `bool:True`／`bool:False`）
//! - `Call` 的**全局名那一格**：函数作用域里对全局名发调用时用 `LOAD_GLOBAL` 低位承担"压 NULL"
//! - 类体两档（含 `def` 的 `__classdict__` cell）＋ `__set_name__` ＋ `__static_attributes__`
//!   的静态收集（含 `if`／`while`／`for` 体）
//! - **函数的隐式返回**（`LOAD_CONST None; RETURN_VALUE`，只在语句体能落到末尾时发）
//!
//! **运行期面**
//! - **通用比较** `executor::compare_public`（`int`／`bool`／`str` 按值；不可比照实测消息）
//!   ⇒ `COMPARE_OP` 与 `operator` 比较族共用
//! - **容器值相等**：`list`／`tuple` 递归、`dict` 按键匹配、`set`／`frozenset` 双向包含；
//!   不同种类一律不等
//! - **算术／一元／位运算** `arithmetic_public`／`unary_public`（整数 `checked_*`，越界如实报；
//!   除零、负移位、一元非整数都照实测消息）
//! - **`in` 的公开入口** `contains_public`（与 `CONTAINS_OP` 共用）
//!
//! **模块面**
//! - `operator`（`SPEC-c-modules.md` §5.2.7）累计 **33** 个函数：比较 12 ＋ 算术 6 ＋ 一元 4 ＋
//!   位运算 5 ＋ `inv`／`index`／`contains` 3；生成脚本、夹具、单元测试随实现同笔入库
//!
//! **验证面**
//! - 语料**每一条都进指令比对**（此前的 `covered=False` 只该用于"位点豁免"，那是
//!   `positions_covered` 的职责）＋ 指令**条数**断言（防 `zip` 静默截断）
//! - 18 个生成脚本全部可复现（重跑不改工作区）
//!
//! **（第 140 轮）`operator.concat` 落地，并顺带修好 `add` 的序列行为**：核心新增
//! `executor::concat_public` —— `str`／`list`／`tuple` 拼接，其余落到 `arithmetic_public` 的 `+`
//! （整数相加、报实测消息）。**实测**：参照里 `concat(['a'], ['b'])` 与 `add(['a'], ['b'])`
//! **都是拼接** ⇒ 两者共用同一条路 ✓（此前我们的 `add` 只认整数，对序列直接报消息 ✗ ——
//! 那是一条**未记录的差异**，这一轮一并修掉 ✓）。⇒ `operator` 累计 **33** 个函数。
//!
//! **（第 141 轮）对账结果**（数字都是本轮实测，不是回忆）：
//! - `README` 的规格计数：**共 12 份、已写 12 份、待写 0 份**（由 `check.py` 机械校验）
//! - `itertools` **18** 个函数、`operator` **33** 个函数（与 §5.2.6／§5.2.7 的记载一致）
//! - 编译语料 **101** 条；**18** 个生成脚本全部可复现（重跑一遍，工作区零改动）
//!
//! ---
//!
//! **剩余未落地清单（第 142 轮整理，供接手排期）**
//!
//! **A. 已定位、只差落地的**
//! 1. `raise` 那一支的**位点**：参照把 `raise <调用>` 的几条记在**被调用者**跨度上
//!    （语料里已标"位置表未对齐"并写明理由）
//! 2. `operator` 还剩 **24** 个公开名（已落地 **33**）：`truediv`（要浮点）、下标/属性族
//!    （`getitem`／`setitem`／`delitem`／`attrgetter`，要协议槽位与 `slice`）、`countOf`／`indexOf`
//!    （要核心开"序列按值查找"入口，与 `contains_public` 同套路）、`matmul`／`i*` 原地族、
//!    `itemgetter`／`methodcaller`（要一个新的可调用对象类型）
//!    —— `length_hint`（第 149 轮）与 `call`（第 147 轮）**已落地**，从本条移出
//! 3. `BC-25`①：检查指令"只在标注／未标注交界处发射"要**跨模块静态信息**（现按带标注保守发射）
//! 4. 合成注解单元的位置表（要给注解记 span）
//! 5. `f.__annotations__` 的**可写**
//!
//! **B. 缺前置的**
//! 6. **浮点**（`float` 类型面）——它挡着 `truediv`、负指数、浮点比较、`DIV-2` 的归一路径
//! 7. **协议槽位**（`OM-11` 的 `richcompare`／`nb_*` 完整面）——挡着容器的序比较、下标族、
//!    `matmul`、用户类型重载
//! 8. `P3-12` 两半：finder（要 `importlib` 在 VM 里跑起来）／能力层 I/O（要 `pyawa-capabilities`）
//! 9. 类创建钩子的另外三格：`__prepare__`／`metaclass=`／`__init_subclass__`
//! 10. `co_code` 一族（要 `bytes` 类型）
//!
//! **C. 原"等你裁定"的四件 —— 第 190 轮对账：四件都已有结论** ✓
//! ① **`NewFn` 槽加异常通道** ⇒ 已裁（`OM-11` 扩："每个槽位签名必须能表达失败"）＋**已落地**
//!    （第 176 轮：`Result` ＋ 11 个实现 ＋ 调用点透传 ✓；剩余 `CallFn`／`ReprFn`／`StrFn` 等仍待办）
//! ② **`marshal` 义务边界** ⇒ 已裁为 `CM-27`（存在且自洽、**不追**字节兼容 ✓）；**卡在 `bytes` 类型面** ✗
//! ③ **`int` 宽度／溢出** ⇒ 已裁为 `TS-45`：**任意精度，现在就做**（`P1-11`）；落地前越界**如实报未实现**
//! ④ **持引用字段的覆盖不变量** ⇒ 已立为 `CX-21` ＋ `T-CX-12`（Rust 侧承担）＋**已落地**（第 147／189 轮）
//!
//! **现在真正卡住的（都不是"等你裁定"，而是缺前置或等裁定之外的规格）**：
//! - `marshal`（`CM-27`）⇐ **`bytes` 类型面**（`dumps` 返回 `bytes` ✓；本层只有类型壳 ✗）
//! - `pa_exec_string` ⇐ 规格没写 **`mode` 的取值域** ✗（`AB-7` 只说"显式必填"）
//!   —— **（第 195 轮已解 ✓）**：`AB-60` 裁了取值域，函数已落地；见文末第 195 轮
//! - `MS-21` 示例 ＋ `T-AB-1` ⇐ **连带卡在 `pa_exec_string`**（M1 判据要"执行一段脚本"）
//!   —— **（第 195 轮已解 ✓）**：示例与验收脚本入库，真编译真运行；见文末第 195 轮
//!
//! **（第 147 轮）`operator.call` 落地**（3.11 新增）：把实参转给可调用对象（`args.split_first`
//! ＋ `call_value`）。**实测消息**是 `call expected at least 1 argument, got 0`（我第一次硬编码了
//! 另一句 ✗，被实测纠正 ⇒ 现在按实测拼）；`call(1)` ⇒ `'int' object is not callable`（那由
//! `call_value` 的老路径报，已实测过）。⇒ `operator` 累计 **33** 个函数。
//!
//! **（第 149 轮）`operator.length_hint` 落地**：有长度给长度、没有给 `default`（默认 0）——
//! 走核心的**安全**入口 `Instance::length_of`（stdlib 禁 `unsafe`，这条路正好合适）。
//! 实测四条：`([1,2])`⇒2、`("abc")`⇒3、`(5)`⇒0、`(5, 9)`⇒9。⇒ `operator` 累计 **33** 个函数。
//!
//! **（第 194 轮刷新；原第 150 轮定格）**（都用命令实测，不是估的）：
//!
//! - `cargo test --workspace` ⇒ **404 passed / 0 failed**
//! - `cargo check --workspace --all-targets` ⇒ **0** 警告/错误
//! - `check.py` ⇒ **11/11**；`selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致
//! - 编译语料 **101** 条；**18** 个生成脚本（复跑零改动 ✓）；`operator` **33** 个函数；`itertools` **18** 个函数
//! - 本地 `dev` 领先 `origin/dev` **34** 笔（未推送）
//! **（第 155 轮复核）交接定格数字**（都用命令实测，不是估的）：
//!
//! - `cargo test --workspace` ⇒ **397 passed / 0 failed**
//! - `cargo check --workspace --all-targets` ⇒ **0** 警告/错误
//! - `check.py` ⇒ **11/11**；`selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致
//! - 编译语料 **101** 条；`operator` **33** 个函数；`itertools` **18** 个函数
//! - 本地 `dev` 领先 `origin/dev` **116** 笔（未推送）
//!
//! **（第 156 轮）本阶段小结续（第 140–155 轮）**：`operator` 又落六刀（算术 → `floordiv`／`mod`／
//! `pow` → 一元＋位运算 → `is_none` 一族 → `inv`／`index`／`contains`／`concat`／`call` →
//! `length_hint`，累计 **33** 个函数）；`add` 对**序列**的行为随 `concat_public` 一并修好；
//! 复核／清扫共清 **11 处**"已落地但文档还说没做"的说法。
//!
//! **（第 161 轮）`CM-27`（`marshal`）的参照实测**（写给实现用；数值都是真跑出来的）：
//!
//! - **API 面**：`dump`／`dumps`／`load`／`loads`／`version` 五个（`dir()` 实测）
//! - **`version` ＝ 5**（参照当前值；裁决要求"自有格式带自己的版本号" ⇒ 模块属性仍报 5，
//!   但**字节格式由我们自己定** ✓）
//! - **类型标签**（首字节实测）：`None` 0x4e、`True` 0x54、小整数 0xe9＋4 字节小端、
//!   大整数 0xec、浮点 0xe7、`bytes` 0xf3、`str` 0xda、`list` 0xdb、`tuple` 0xa9、
//!   `dict` 0xfb、`set` 0xbc、`frozenset` 0xbe、`code` 0xe3 —— **这些我们都不追** ✗
//!   （字节差异按 `MS-19` 登记 ✓）
//! - **往返语义**（这条要追 ✓）：`loads(dumps(x))` 对上述每种类型都给出**等价**对象；
//!   `code` 也往返（`repr` 不同只是因为地址，属测量假象 ✗）
//! - **错误消息**（实测）：`loads(b"")` ⇒ `EOFError: EOF read where object expected`；
//!   `loads(b"zzzz")` 与被截断的流都 ⇒ `EOFError: marshal data too short`
//! - **`.pyac` 不依赖它** ✓（裁决原文）：产物走自己的容器（`IM-18`）
//!
//! **（第 163 轮）`CM-27` 的硬前置：`bytes` 类型面**（我自己第 162 轮的建议在这里被纠正 ✗）：
//!
//! - 实测：参照的 `marshal.dumps(x)` **返回 `bytes`**（`isinstance(b, bytes) is True`）
//! - 本层现状：`bytes` **只注册了类型壳**（`builtin_types.rs` 里 `Ladder::Later`），
//!   **没有载荷类型、没有创建入口、没有取值入口**（`grep BytesObject`／`new_bytes` 全空）
//! - ⇒ **不是只有 `dump`／`load` 卡住**：`dumps` 的**返回类型**就是 `bytes` ⇒ 整个 `marshal`
//!   都卡在 `bytes` 类型面上 ✗。硬造一个"像 bytes 但不是"的返回物 ✗ 等于伪造类型面，
//!   而替尚未写出的规格（`bytes` 那一族）做决定是 `AGENTS.md` 禁止的 ⇒ 记 B 档、等裁定
//! - **B 档新增**：`bytes` 类型面（载荷 ＋ 创建／取值入口 ＋ 与 `str` 的编解码）——`CM-27` 的**前置**
//! - 转做：`TS-45`／`P1-11` 的**第一小步**（`PLAN` §9.4 新第 4 条点名的 M1 三处小改动：
//!   `crate-type`／`pa_exec_string`／`MS-21` 示例），它与 `bytes` 无关、可立即开工 ✓
//!
//! **（第 165 轮）M1 第二处 `pa_exec_string` 的切口（规格已裁、实现待补）**：
//!
//! - **规格**（`SPEC-c-abi.md` §15）：`pa_exec_string(st, src, len, chunkname, mode)` ——
//!   执行字符串，**`mode` 显式必填、无默认**（`AB-7`）；同族还有 `pa_exec_file`（I/O 走能力层
//!   `IM-15`）与 `pa_exec_bytecode`（`.pyac`；指令集版本不符返 `PA_ERR_INVALID`，`BC-29`）
//! - **现状**：`pa.h` 里**没有 `pa_exec*` 的任何声明** ✗，`pyawa-abi`／`pyawa-runtime` 里
//!   也没有实现 ✗ ⇒ 三处里这一处是**纯补**（不是改）
//! - **下一轮的切口**：① `pa.h` 加三条声明（注释写清 `mode` 必填与 §15 的返回码约定）；
//!   ② 实现 `pa_exec_string`（走现成的 `compile` ＋ `instantiate` ＋ `execute`；
//!   `mode` 映射到编译期模式，未知 mode ⇒ `PA_ERR_INVALID`）；③ 出参用现成的"状态码 ＋ 出参"
//!   形态；④ 测试：一条能跑的字符串 ＋ 一条语法错 ＋ 一条未知 mode（错误码逐条断言）
//! - **必须先读**：`pa.h` 的行文样式（§"状态码 ＋ 出参"那段）与一个现成入口（如 `pa_call`）的实现，
//!   **不凭记忆写** ✗
//!
//! **（第 166 轮）`pa_exec_string` 卡在一个规格缺口：`mode` 的取值域没写** ✗
//!
//! - `SPEC-c-abi.md` §15 只说 `mode` **显式必填、无默认**（`AB-7`），**没有**给它的取值域，
//!   也**没有**写"未知 `mode` 返什么"
//! - 代码里能对上的只有 `Mode` 的两个变体：`PurePython`／`Extension`（`IM-1`）
//! - ⇒ 我**不能**凭直觉定 `"pure"`／`"extension"` 这样的字符串（那是替规格做决定 ✗，
//!   也正是我把 `pa_exec_string` 从上一轮拖到现在的**正确理由**：切口本身有个洞）
//! - 已按 ①②③ 问用户（`mode` 取值域＋未知值的返回码）；裁定前**不动**这一处，
//!   改为先做 M1 第三处（`MS-21` 的 ≤50 行 C 示例 ＋ 它的 `T-AB-1` 验收）——
//!   那一处的接口在 `pa.h` 里**已经存在**，不存在这种"要发明"的地方 ✓
//!
//! **（第 167 轮）M1 的依赖链：判据 → `pa_exec_string` → `mode` 取值域**（一条裁定卡住三处）
//!
//! - **M1 判据原文**（`PLAN` §6）："一个 ≤50 行的 C 程序能：**创建实例 → 执行一段脚本 →
//!   注入一个宿主函数 → 取回一个值**"
//! - "**执行一段脚本**"的唯一入口就是 `pa_exec_string` ✗（`pa.h` 里没有任何 `pa_exec*` 声明；
//!   `pa_exec_bytecode` 要 `.pyac` 容器、`pa_exec_file` 要能力层 I/O，都不适合当 M1 的最小示例）
//! - ⇒ **`MS-21` 示例` 也卡在 `mode` 那个缺口上** ✓（不是独立可做的）⇒ 本轮不写那个 C 程序
//! - **实测环境**：本机 `cc` ＝ gcc 15.2 ✓ ⇒ `T-AB-1` 可以真编译真运行（不做"缺编译器就跳过"这种
//!   弱化处理 ✗；缺就**红**）
//! - **转做 ②**：`OM-11` 扩（槽位签名必须能表达失败）——裁决原文明确、无规格缺口 ✓，
//!   先动 `NewFn`（约 6 处 `with_new` ＋ `type_call` 传播），随后 `chain.from_iterable(5)`
//!   如实报 `TypeError` ✓
//!
//! **（第 168 轮）②`OM-11` 扩的精确坐标（都读出来的，不是估的）**
//!
//! - **槽位现值签名**（`type_object.rs`）：
//!   `pub type NewFn = unsafe fn(NonNull<TypeObject>, &[NonNull<Header>], &Instance)
//!   -> Option<NonNull<Header>>;`
//!   —— 注释已写明"返回 `None` ＝ 这个类型不能这样实例化（调用方报 `TypeError`，消息照参照）"
//!   ⇒ **现状是 `Option`**：能表达"失败"，但**表达不了失败的原因** ✗ ⇒ 裁决要求的正是补上原因
//!   （"成功值／失败"二元 ＋ 由 VM 转成 Python 异常、落实例级异常状态 `BC-60`）
//! - **调用点数量**：`with_new(` 共 **14 处**（`instance.rs` 12 ＋ `classes.rs` 1 ＋ 其余 1）
//!   ⇒ 裁决里写的"**约 6 处**"是**低估**，机械化改动比预估大一倍多 ✓（先纠正数字再动手 ✓）
//! - **槽位被调用处只有一处**（`executor.rs` ≈3466）：现在 `None` ⇒ 现场拼出
//!   `TypeError: cannot create '<类名>' instances` ⇒ 改成 `Result` 后，这条消息由**槽位**给，
//!   调用点直接透传 `Err`（不再由调用点猜测原因 ✓ 这正是裁决要的"一处真相"）
//! - **下一轮的执行顺序**：① `NewFn` 签名改 `Result<NonNull<Header>, ExecError>`；
//!   ② 14 处 `with_new(...)` 的实现逐个补 `Ok(...)`／按实测消息返 `Err`；③ 调用点透传；
//!   ④ 验收：`chain.from_iterable(5)` 如实报 `TypeError: 'int' object is not iterable`（参照实测）
//! - **同类槽位**（`ReprFn` 是 `Option<String>`、`StrFn` 同、`CallFn` 的失败路径…）随后按同一口径扫一遍 ✓
//!
//! **（第 169 轮）②`OM-11` 扩的性质：槽位今天**不会失败**（`None` 收尾确实存在，见第 173 轮）**
//!
//! - 事实（**读原文**，第 173 轮定的规矩）：`*_new` 里**确实有** `if !args.is_empty() { return None; }`
//!   （`plain_new` ≈2119、`int_new` ≈2150 等）⇒ 失败通道今天**存在**，但
//!   **表达不了原因** ✗（`Option` 只有"能不能"，没有"为什么"）——这正是裁决要补的
//! - `int_new` 的实际行为：**只接线了零参形态** ⇒ `int("a")` 今天报
//!   `TypeError: cannot create 'int' instances`（类型与消息都不对 ✗，但**不是**静默给 0）
//!   —— 参照**规定了** `int("a")` 抛 `ValueError` ⇒ 按 `MS-19`（可观察语义缺口**必须修**）这是**必须修**的
//! - ⇒ ② 的真实形状：**(a) 通道**（签名改 `Result`）与 **(b) 失败语义**（逐类型按**实测消息**抛错）
//!   **不可分离** ✗（我第 169 轮提的"通道先行"是错的）：要改签名就得给每个 `None` 一个**正确**的
//!   失败值，而正确与否取决于该处是"未实现"还是"该抛 Python 异常"
//! - 材料已备：`tools/gen_constructors_fixture.py`（12 条实测，默认只打印、实现落地那一笔再 `--emit`）
//!
//! **（第 179 轮）②(b) 第二／三批 ＋ B 档新增一格**
//!
//! - **`bool_new`**：零参 ⇒ `False`；一个实参 ⇒ 走核心**同一份**真值判定（`truthiness_public` ✓，
//!   不另写一套）；多参 ⇒ 照实测报 `TypeError: bool expected at most 1 argument, got 2` ✓
//!   （顺带抽了 `singleton_bool` 小助手：单例 ＋ 给调用方一份引用 ✓）
//! - **行为验收**：`tests/constructors.rs` 加了 `bool` 那组（`bool()`／`bool(1)`／`bool(0)` ⇒ 真值，
//!   `bool(1, 2)` ⇒ `TypeError` ✓）
//! - **（第 190 轮更新）容器字面量：列表与字典已落地** ✓ —— `[]`／`[1, 2]`（第 183 轮）与
//!   `{}`／`{1: 2}`（第 187 轮）都与参照**逐字节一致**；第 178 轮撤掉的 `int([])` 源级用例也已收回 ✓。
//!   **仍缺**：**元组字面量** `(1, 2)`／`()` —— 实测它们走 `LOAD_CONST` **常量折叠**（不是 `BUILD_TUPLE`）
//!   ⇒ 需要给常量表加元组常量（形状已在第 180 轮测清 ✓）；集合字面量 `{1, 2}` 也还没做
//! - **仍未做**：其余 7 个 `*_new` 的失败语义（`float`／`str`／`list`／`tuple`／`dict`／`set`／
//!   `exception` ✓，材料都在 `tests/fixtures/constructors.rs`）
//!
//! **（第 180 轮）容器字面量的参照形状（原始实测，未转述）**
//!
//! - `x = []` — ` — ` — ` — ` —  consts=['None']` — RESUME(0) BUILD_LIST(0) STORE_NAME(0) LOAD_CONST(0) RETURN_VALUE`
//! - `x = [1, 2]` — ` — ` —  consts=['1', 'None']` — RESUME(0) LOAD_SMALL_INT(1) LOAD_SMALL_INT(2) BUILD_LIST(2) STORE_NAME(0) LOAD_CONST(1) RETURN_VALUE`
//! - `x = {}` — ` — ` — ` — ` —  consts=['None']` — RESUME(0) BUILD_MAP(0) STORE_NAME(0) LOAD_CONST(0) RETURN_VALUE`
//! - `x = (1, 2)` — ` — ` —  consts=['1', 'None', '(1, 2)']` — RESUME(0) LOAD_CONST(2) STORE_NAME(0) LOAD_CONST(1) RETURN_VALUE`
//! - `x = ()` — ` — ` — ` — ` —  consts=['None', '()']` — RESUME(0) LOAD_CONST(1) STORE_NAME(0) LOAD_CONST(0) RETURN_VALUE`
//!
//! - ⇒ 下一轮实现时按这些形状发射（`BUILD_LIST`／`BUILD_MAP`／`BUILD_TUPLE` ＋ 常量表 ✓），
//!   并与语料逐字节对拍 ✓；注意空 `()` 与空 `[]` 的常量表差异（见上表 ✓）
//!
//! **（第 181 轮）容器字面量这一刀的精确坐标**（读出来的，不是估的）
//!
//! - **词素**：`2016:    LeftBracket,`
//! - **AST `Constant` 变体**：`1778:    Constant(crate::compile::Constant, Span),`
//! - **解析主匹配（名字分支）**：`2943:        Some(Lexeme::Name(name)) if name == "None" => {`
//! - **`span()` 里并 `Constant`**：`1833:            | Expression::Constant(_, span)`
//! - **`BUILD_LIST` in crates/pyawa-core/src/opcode.rs**：`（没找到）`
//! - **`BUILD_LIST` in crates/pyawa-stdlib/src/opcode.rs**：`（没找到）`
//!
//! - 词法**已有** `[`／`]`／`{`／`}`（见上）⇒ `x = []` 报的是**解析**错、不是词法错 ✓
//! - **第一刀只做列表**（`[]` ＋ `[1, 2]`）：解析＝"逗号分隔到 `]`"、发射＝`BUILD_LIST(n)` ✓；
//!   要同步的 `match` 有四处（`span()`／`fold_constant`／`leftmost_literal`／发射 ✓）
//! - 发射形状照第 180 轮实测：空表 ⇒ `BUILD_LIST(0)`；`[1, 2]` ⇒ 两条 `LOAD_SMALL_INT` 后
//!   `BUILD_LIST(2)` ✓（**元组走 `LOAD_CONST` 折叠、不是 `BUILD_TUPLE`** ✓ —— 我原先猜错了 ✗）
//! - ⚠ **`BUILD_LIST` 的指令表位置本轮没查准**（`opcode.rs` 里没有；上面列了另外两个候选文件的结果 ✓）
//!   ⇒ 下一轮**先确认指令名与编号**（照 `BC-30` 那一族的生成表 ✓），再动发射 ✓
//!
//! **（第 182 轮）上一轮"没查准"的那格查实了 —— 而且比预想的好** ✓
//!
//! - **指令名与编号**（`crates/pyawa-core/src/opcode_metadata.rs`，实测）：`BUILD_LIST` ＝ **46**、
//!   `BUILD_MAP` ＝ **47**、`BUILD_SET` ＝ **48**（反向表也都有 ✓）
//! - **发射器已经在用**：`compile.rs:1600` 有 `opcode::opcode("BUILD_LIST")`（`*` 实参那条路径 ✓）
//!   ⇒ "名字能不能解析"这件事**早就有先例** ✓，不用再犹豫 ✓
//! - **执行器也已经实现**：`executor.rs:4244` 的 `"BUILD_TUPLE" | "BUILD_LIST" | "BUILD_SET"` 那一臂 ✓
//! - ⇒ 列表字面量缺的**只有解析／AST／发射**三处 ✓（运行期是现成的 ✓）—— 下一轮的范围比预想小 ✓
//! - ⇒ 结论：第 181 轮那条"⚠ 没查准"可以撤掉 ✓（查证方式：`grep -rln` 找文件 ＋ 读原文 ✓，
//!   不是靠名字猜 ✓）
//!
//! **（第 185 轮）纠正第 181 轮的一处"外推" ✗ —— `{`／`}` 的词素根本不存在**
//!
//! - 第 181 轮我写"词法**已有** `[`／`]`／`{`／`}`"，依据只是看到了 `LeftBracket` 那几行 ⇒
//!   **外推**了 `{`／`}` ✓。第 185 轮按硬规矩**读原文**（`grep -n "    LeftBrace," ...`）⇒
//!   `LeftBrace`／`RightBrace` **都没有** ✗（只有 `LeftParen`／`RightParen`／`LeftBracket`／
//!   `RightBracket`／`Colon` ✓）
//! - ⇒ **字典字面量比列表多一步**：要先在**词法**里加 `{`／`}`（词素枚举 ＋ 字符映射 ✓）
//! - 参照形状（本轮实测）：`x = {1: 2}` ⇒ `LOAD_SMALL_INT(1) LOAD_SMALL_INT(2) BUILD_MAP(1)` ✓
//!   （**键先值后** ✓）、常量表 `['1', 'None']` ✓；`x = {}` ⇒ `BUILD_MAP(0)` ✓（第 180 轮测过）
//! - **这是我第一次在"硬规矩"立下之后还犯同一个错** ✗（凭相邻线索外推、没读原文）⇒ 记在此处：
//!   规矩不是只在**下结论**时用，**写计划／写坐标**时同样适用 ✓
//!
//! **（第 186 轮）词法器里 `[` 那一行的原文（原样转录，供加 `{`／`}` 用）**
//!
//! - `                    return Err(CompileError::Unsupported(`
//! - `                        "负号／减法尚未接线（本层只做注解里的 '->'）".to_owned(),`
//! - `                    ));`
//! - `                }`
//! - `            }`
//! - `            '[' => {`
//! - `                let start = column!(index);`
//! - `                lexemes.push(Lexeme::LeftBracket);`
//! - `                spans.push(Span::new(line, line, start, start + 1));`
//! - `                index += 1;`
//! - `            }`
//! - `            ']' => {`
//! - `                let start = column!(index);`
//! - `                lexemes.push(Lexeme::RightBracket);`
//!
//! - 上一轮我按"从 `Lexeme::LeftBracket` 派生"去找生产式，结果命中的是**解析器的臂** ✗
//!   （断言拦住、未写入 ✓）；真相：生产式在 `compile.rs` 的**词法器**里（上面那一段 ✓），
//!   枚举变体在 2033 行（我档里写的 2016 是**旧行号** ✗）
//! - ⇒ 下一轮照上面那一行的**格式**加 `{`／`}` 两条 ✓（不是猜 ✓），再照列表那一套做 AST／解析／发射
//!
//! ---
//!
//! **（第 195 轮）M1 收尾：`pa_exec_string` 落地，`T-AB-1` 真编译真运行 ✓**
//!
//! - **`pa_exec_string` 落地**（`§15.3`／`AB-60`）：`include/pa.h` 补三条执行入口的声明；
//!   `mode` 只认 `"python"`／`"pyawa"`（其余含空串与 `NULL` ⇒ `6`），解析失败 ⇒ `2`，
//!   脚本异常 ⇒ `1` ＋ `pa_errmsg`，**未接线的指令** ⇒ `5`（`AB-22`：与"已实现但拒绝"分开）。
//!   实现走现成的 `compile` ＋ `instantiate` ＋ `execute`；模块体在实例的**全局命名空间**里跑
//!   （与 `pa_register`／`pa_getglobal` 同一份）⇒「脚本调到宿主函数」这条链路端到端成立
//!   （`crates/pyawa-abi/tests/abi.rs` 的新用例逐条钉住）。
//! - **检查档位按 `docs/SPEC-c-abi.md` §15.3 的注暂缓**（本版一律浅层＝`TS-31` 的默认档；
//!   编译器目前不按档位改发射，深层与浅层无可观察差异）；`AB-7` **不算**被满足。
//! - **`pa_exec_file`／`pa_exec_bytecode`**：声明并如实报"未提供"（`5`）——前者要能力层 I/O
//!   （`IM-15`）、后者要 `.pyac` 装载器（`P3-12`）。
//! - **引导期补一格（顺手修掉的真相缺口）**：`Instance::new` 现在注册内部类型 `CodeObject`
//!   （与 `Frame` 同款处理、同样不进 `TS-41` 的表）。`compile::instantiate` 与
//!   `Instance::code_with_qualname` 都写着 `expect("…在引导期已登记")`，而在此之前**只有测试
//!   自己登记过才成立** ✗（生产路径会 panic）。连带把两处测试脚手架
//!   （`tests/common/mod.rs`、`tests/frame_layout.rs`）从"自建第二份同名类型"改成取引导期那份：
//!   两份同名类型会让 `MAKE_FUNCTION` 的**类型身份**比对错位（本轮实测 6 个用例因此变红，改回
//!   一份后全绿 ✓）。
//! - **`MS-21`／`T-AB-1`**：`examples/m1.c`（**45 行** ≤ 50）＋ `tests/ci/t_ab_1.py`
//!   （`cargo build -p pyawa-abi` → `cc` **真编译真链接** staticlib → **真运行** → 断言输出 `42`；
//!   **缺 `cc` 即红**，不跳过）。判据四条都在：创建实例 → 执行脚本 → 注入宿主函数 → 取回值。
//! - **定格数字（第 195 轮实测）**：`cargo test --workspace` ⇒ **410 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**65** 个二进制、**410** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 196 轮）M1 ②：足迹报告出数 ✓**（`§13-17` 的提示项，是 `§13-19` 与 M5 的**基线**）
//!
//! - 落地：`tools/footprint_host.c`（测量宿主：`pa_create`／`pa_exec_string` 的 `CLOCK_MONOTONIC`
//!   时长 ＋ 本进程 RSS／`VmHWM`）＋ `tools/measure_footprint.py`（驱动：`cargo build` → `cc`
//!   链静态库 → debug／release 两档 → **同轮现测** `python3 -c pass` 作对照；缺 `cc`／`python3` 即红）
//! - **数值与口径的唯一出处**是 `docs/DESIGN.md` 的"Pyawa（M1 最小内核）实测基线"——此处**不复述**
//!   （一处真相）。要点：release 档 VM 引导 **48 µs**、最简执行 **1.9 µs**、宿主整程 **1.05 ms**、
//!   峰值 RSS **3.4 MB**；debug 档引导 **365 µs**（差一个数量级 ⇒ 对外只引 release）
//! - 对 `§13-19` 的可用事实：单个空实例的常驻增量约 **0.7 MB**——"拆不拆容器专属 gc 链"仍**未定**
//!
//! **（第 197 轮）`P1-13` 落地：`pa_options` 过界，`AB-7` 的档位子句**已满足** ✓**
//!
//! - **`compile()` 多一个显式输入** `optimization: u8`（`BC-16`／`IM-21` 的五要素里，此前只有
//!   模式与档位是真输入）⇒ **48 处调用点**补一个实参（12 个文件，绝大多数是测试）：逐文件按
//!   **精确字面量**替换（不是正则），再靠编译器逐个兜底 ✓。本层**还没有优化器** ⇒ 优化级
//!   目前**不改发射**（`compile` 的文档写明，不是漏用）。
//! - **`pa.h`／`pyawa-abi`**：`pa_options { size, check_tier, optimization }`（尺寸标记，`AB-61`，
//!   惯例同 `AB-43`／`AB-51`）；`pa_exec_string`／`pa_exec_file` 各多收一个 `const pa_options *`
//!   （**可 `NULL`** ⇒ 浅层 ＋ 默认优化级）；有界读，`size` 盖不住字段／档位不是 `0`／`1`／
//!   优化级超出 `u8` ⇒ `6`（**禁止**静默降级）。
//! - **档位真的改发射**（不只是"收下"）：`crates/pyawa-abi/tests/abi.rs` 的端到端验收——同一份
//!   `def f(x: int) -> int` 源码，**深层** ⇒ `f("hello")` 归责 `TypeBoundaryError`（`TS-12`），
//!   **浅层**（`NULL`）⇒ 照常成功。`AB-7` 的档位／优化级子句因此从"暂缓"改判**已满足**。
//! - `examples/m1.c` 跟着改（两条 `pa_exec_string` 传 `NULL`）；`T-AB-1` 仍绿。
//! - **定格数字（第 197 轮实测）**：`cargo test --workspace` ⇒ **412 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**65** 个二进制、**412** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 198 轮）M2 对拍 harness 的减配首版落地 ✓**（`MS-6`…`MS-15`／`MS-24`）
//!
//! - `crates/pyawa-abi/tests/conformance.rs` ＋ `tests/conformance/corpus/`（自建 9 条，
//!   `MS-13` ①）：两侧各跑一次（参照 `python3`；被测走 `pa_exec_string`，且在**子进程**里跑
//!   ⇒ `MS-15` 的超时与崩溃隔离对两侧都成立），比**退出码 ＋ 未捕获异常（类型／消息）＋
//!  探针值**。`stdout`／`stderr` **不比**（`print` 未落地，`CM-26`）——**尚未落地**，不是差异。
//!   首轮 **9/9 通过 · 0 已知差异 · 0 新差异**；自检（`MS-12`）全绿；报告落 `target/conformance/`。
//! - **抓到三处"尚未实现"**（按 `MS-19` 的适用范围**不进语料**，记在 §9.2 与
//!   `tests/conformance/README.md`）：**下标表达式**（`x = a[1]`）、**括号表达式**（`x = (1)`）、
//!   **类对象属性读**（`C.v` ⇒ `'type' object has no attribute 'v'`）。
//! - 另一处**集成缺口**：ABI 实例**没有 `builtins` 映射** ⇒ `ValueError`／`len` 一类名字取不到
//!   （`builtins` 模块归 `P3-14`／`CM-14`）。
//! - **抓到并已修一处可观察缺口**（`MS-19`：可观察语义缺口必须修）：`pa_exec_string` 跑模块时不补
//!   `__name__` ⇒ **任何 `class` 语句都报 `NameError`**（类体序言要读它）。修法：未绑定时补
//!   `"__main__"`、宿主绑过**不覆盖**（`python3 -c`／脚本同款）；验收 `crates/pyawa-abi/tests/abi.rs`。
//! - **定格数字（第 198 轮实测）**：`cargo test --workspace` ⇒ **416 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**66** 个二进制、**416** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 199 轮）`P1-11` 第一刀：任意精度的纯算术核心 ✓**（`TS-45`；接线随后逐笔做）
//!
//! - `src/bigint.rs`：**与对象模型解耦**的任意精度整数——加／减／乘、**floor** 除法与取模、
//!   幂、比较、`hash`、十进制互转、`to_f64`（**正确舍入**：前 54 位 ＋ 最近偶数；溢出给 `±inf`，
//!   映射 `OverflowError` 是调用点的事）。内部是 **2^32 进制小端**、规范化。
//! - 口径全部对着参照**实测**钉住（`tools/gen_int_fixture.py` → `tests/fixture-int-3.14.json`，
//!   151 条算术 ＋ 27 条比较 ＋ 13 条 `hash` ＋ 4 条除零 ＋ 5 条 float ＋ 4300 位上限两条消息）：
//!   `//` 是 **floor**、`%` 取**除数**的符号；`hash = sign × (|x| mod 2^61-1)`、`-1` 改判 `-2`
//!   （含点名值 `hash(2**100) == 549755813888`）；`to_decimal` **不设**位数上限（4300 是策略）。
//! - **顺带修掉一个真 bug**：`//`／`%` 原先用 `div_euclid`／`rem_euclid`，**负除数**上与参照不一致
//!   （实测 `7 // -2 == -4`、`7 % -2 == -1`，euclid 给 `-3`／`1`）。两处现在都走 `bigint` 核心
//!   （一处真相）；`operator` 夹具补了 6 行负除数并重生成。
//! - **仍未接线**（下一刀）：`int` 载荷（仍是 `i64`，越界如实报未接线）、`BINARY_OP` 的大整数路径、
//!   `repr`／`str` 的 4300 位上限、与 `float` 互转的调用点、`bool`／小整数单例的关系。
//! - **定格数字（第 199 轮实测）**：`cargo test --workspace` ⇒ **427 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**67** 个二进制、**427** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 200 轮）`P1-11` 第二刀：`int` 载荷两态接线 ✓**（任意精度进入执行器）
//!
//! - `IntValue`（`Small(i64)`／`Big(BigInt)`）成为 `IntObject` 的载荷——**同一个 `int` 类型对象**
//!   （`type(2**100) is int`），`OM-23` 的小整数单例照旧（大整数不进单例表）。
//! - `Instance` 多了 [`Instance::int_of`]（**按类型分派用**）与 [`Instance::new_int_value`]；
//!   `int_value` 保持"`i64` 快路径"语义（大整数给 `None`）——**用得快路径的地方一律改过**，
//!   否则大整数会被误判（真值＝假、等值＝身份、比较＝不可比）。
//! - 执行器：四则／整除／取模／幂、一元 `-`／`+`／`abs`、大小比较、等值比较、真值、`repr`、
//!   `int()` 构造（含十进制串解析）全走任意精度；`%`／`//` 的 floor 语义同核心。
//! - **踩过并修掉的两个自伤 bug**（都记在这里，免得后人重犯）：
//!   ① `new_int` ↔ `new_int_value` **互相递归** ⇒ 非单例值（如 `300`）爆栈（`gdb` 抓到的）；
//!      修法：直接分配抽成私有 `alloc_int`，两条公开入口不再互调。
//!   ② 单例区间外的值走 `int_value` 会得到 `None` ⇒ 真值判定会把它当假——三处都改成 `int_of`。
//! - 仍未接线：大整数上的位运算／移位与 `__format__`、`repr`／`str` 的 **4300 位上限**、
//!   与 `float` 互转的调用点、**ABI 的大整数通道**（`pa_tointeger` 如实返 `5`）。
//! - **定格数字（第 200 轮实测）**：`cargo test --workspace` ⇒ **432 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**67** 个二进制、**432** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 201 轮）`P1-11` 第三刀：`int`↔`str` 位数上限的输入方向 ＋ `sys` 两个入口 ✓**
//!
//! - `TS-45` ①：`int('<十进制串>')` 的位数上限（默认 **4300**、`0` ＝ 不限）——**输入方向**已接线：
//!   超限报 `ValueError`，消息**逐字**照参照实测（`Exceeds the limit (… digits) … value has N digits`）。
//!   实测口径：**前导零也计入**、正负号与下划线不计、正好上限位可过。
//! - 状态**按实例存**（`CX-3`）：`Instance::int_max_str_digits`／`set_int_max_str_digits`
//!   ＋ 两个实测常量（`INT_MAX_STR_DIGITS_DEFAULT` ＝ 4300、`INT_MAX_STR_DIGITS_THRESHOLD` ＝ 640）。
//! - `pyawa-stdlib` 的 `sys` 多了 `get_int_max_str_digits()`／`set_int_max_str_digits(n)`；
//!   `set_` 的五种非法形态（`(0, 640)` 区间、非整数、超出 C `int`、少给／多给实参）与
//!   `get_` 收实参，六条消息全部照参照实测。
//! - 夹具按 **API 面**分开：转换的边界事实留 `tools/gen_int_fixture.py`，`sys` 两个入口的
//!   API 面归 `tools/gen_sys_fixture.py`（各自的住处，避免两处真相）。三个生成脚本复跑**字节一致** ✓。
//! - **仍未接线**：位数上限的**输出方向**（`repr(huge)`／`str(huge)`）——需要 `repr`／`str` 槽
//!   能表达失败（`OM-11` 扩的剩余项，`lib.rs` 的 C 档①里早记着），那是下一刀。
//! - **定格数字（第 201 轮实测）**：`cargo test --workspace` ⇒ **434 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**67** 个二进制、**434** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 202 轮）`OM-11` 扩的 repr／str 那一格 ＋ `TS-45` ①的输出方向 ✓**
//!
//! - `ReprFn`／`StrFn`：`Option<String>` ⇒ **`Result<String, ExecError>`**（`OM-11` 扩：
//!   "每个槽位的签名必须能表达失败"；`C` 档①点名的剩余项之一）。整条 repr／str 链
//!   （`object_repr(_native)`／`object_str(_native)`／`object_ascii`／`element_repr`／`element_str`）
//!   随之返回 `Result`；20 个槽实现 ＋ 40 余处调用点跟着改（多数是测试加 `.expect`）。
//! - **顺带修掉一处吞异常**：`override_text`／`element_repr` 原先把 `__repr__`／`__str__`
//!   覆写里抛的异常**吞掉**（只记在实例上，文档还写着"属已知偏差"）⇒ 现在如实上抛，与参照一致。
//!   同轮踩过一个自伤：改成 `Result` 时把"类型字典里没有这个名字 ⇒ 返回 `None`"的早退丢了，
//!   `exceptions.rs` 的夹具立刻报 `AttributeError`（**夹具抓到的**，不是我事后想到的）。
//! - **`TS-45` ①的输出方向**：`repr(huge)`／`str(huge)` 超过 `sys.get_int_max_str_digits()` ⇒
//!   `ValueError`（实测口径：这条消息**不带** `value has N digits`，输入方向那条带）。
//! - **定格数字（第 202 轮实测）**：`cargo test --workspace` ⇒ **435 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**67** 个二进制、**435** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 203 轮）`P1-11` 收官两格：位运算／位移 ＋ `int`↔`float` ✓**
//!
//! - `BigInt` 补上 `& | ^`（**补码语义**，负数无限符号扩展）、`~`（`-x-1`）、`<<`（乘 `2^n`）、
//!   `>>`（**floor**，与 `//` 同口径）。实现骨架：摊成同宽补码 ⇒ 逐 limb 运算 ⇒ 变回符号-幅值
//!   （`to_twos_complement`／`from_twos_complement`）；`>>` 的 floor 靠"被丢掉的低位里有 1 就再减一"。
//! - `<<` 超出 `MAX_SHIFT_LIMBS` 回 `None` ⇒ 调用点报 **`MemoryError`**（参照在 `1 << 2**62`
//!   实测就是 `MemoryError`，**消息为空**）；负位移量报 `ValueError: negative shift count`；
//!   `1 >> 2**62 == 0`（实测：不报错）。**实现上限**写在常量注释里，不假装能算。
//! - 执行器删掉了 `bitwise_i64`（旧的"超出 i64 就报未实现"那条路），一元 `~` 同步。
//! - `int`↔`float`：`float(<整数>)` 正确舍入（溢出报实测的 `OverflowError: int too large to
//!   convert to float`）、`float(<浮点>)` 原值；`int(<浮点>)` **向零截断** ＋ `inf`／`nan` 的
//!   实测消息；`int(<大 double>)` 走 [`crate::bigint::BigInt::from_f64_truncated`]（尾数×2^指数
//!   不动点分解）⇒ **精确**——**不能**借道 `i64`：Rust 的 `as` 转换会**静默饱和**（`TS-45` 明禁）。
//! - **实测纠了我一次**：我起初按"`int(1e300)` 是 1 后面 300 个 0"写断言，夹具给出的是
//!   `1000000000000000052504…`（double 并不精确等于 `10^300`）。数字进夹具，断言照夹具写。
//! - **仍未接线**：大整数的 `__format__`、ABI 的大整数通道、`float('<串>')` 的解析。
//! - **定格数字（第 203 轮实测）**：`cargo test --workspace` ⇒ **440 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**67** 个二进制、**440** 项）；
//!   `t_ab_1.py` ⇒ 绿。
//!
//! **（第 204 轮）大整数的 `__format__` ✓ ＋ 格式化模块两处旧偏差**
//!
//! - `format::format_big_int(&BigInt, &Spec, max_str_digits)`：整数码（`d`／`n`／`b`／`o`／
//!   `x`／`X`／`c`）走任意精度（新 `BigInt::to_radix`）；浮点码先 `to_f64`，超大整数撞
//!   `OverflowError`。`i64` 不再单开一条路（删掉 `format_int` 薄壳）——**一条真相**。
//! - 两条**实测**口径：位数上限**管**十进制码（`format(10**5000)` 报 `ValueError`）、
//!   **不管**十六进制码；`c` 码"装不下 C long"与"落在 Unicode 外"各有一条消息。
//! - **顺手修掉两处早先就存在的偏差**（都是新夹具行抓出来的，不是我想起来的）：
//!   ① `pad` 把 `0` 与显式对齐当互斥 ⇒ `format(42, '=+040')` 被填成空格（参照是 `+000…042`）；
//!   ② `g`／`G` 把精度当**小数位** ⇒ `format(1e30, 'g')` 摊成 31 位数字（参照是 `1e+30`）。
//! - 同轮第三次被**实测**纠正：我在测试里写 `hex.starts_with('1')`（想当然 `10**5000` 的十六进制
//!   以 1 开头），实际是 `31e20801…` ⇒ 改成把参照的整串带回夹具逐字对拍。**别猜，去量。**
//! - **定格数字（第 204 轮实测）**：`cargo test --workspace` ⇒ **441 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**67** 个二进制、**441** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 205 轮）进 `P1-12`：`bytes` 类型面第一刀 ✓**
//!
//! - 类型本身以前**只有名字**（`bytes_iterator` 在表里、`bytes` 没有实例）⇒ 这一刀把
//!   `BytesObject`（`Vec<u8>`）与 `bytes` 类型对象（`new`／`repr`／`str`）建起来。
//! - 构造：空／计数／`bytes`／整数 `list`／`tuple`／`str`+UTF-8；**七条失败消息照实测**
//!   （`bytes(256)` 其实是**成功**的 256 个零字节——这条最容易想当然写错）。
//! - 观测面：`len`／整数索引／迭代（`bytes_iterator`）／等值／字典序。
//! - `repr` 的转义按**字节**判：可打印 ASCII 原样、其余 `\xNN`（合法 UTF-8 也照转，
//!   实测 `repr(b'caf\xc3\xa9') == "b'caf\\xc3\\xa9'"`）。
//! - **记录但不测**：切片要 `slice` 类型（`TS-42` 的 M3+）；`hash` 的实测规则是
//!   "与同内容 ASCII `str` 相同"，但 `hash()` 本身还没接线。
//! - **定格数字（第 205 轮实测）**：`cargo test --workspace` ⇒ **449 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**68** 个二进制、**449** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 206 轮）`P1-12` 第二刀：`bytes` 字面量 ✓**
//!
//! - 编译器认 `b'…'`／`B"…"`：**词法层**就把转义解成字节（`\n \t \r \\ \' \" \a \b \f \v`、
//!   `\xNN`、`\ooo`）；非 ASCII 字符与坏 `\x` 照参照实测的文本报（`\x` 那条还带
//!   `at position N`——位置是反斜杠相对字面量内容的下标，实测 `b'a\x1'` ⇒ 1）。
//!   三种没实测过的转义（`\u`／`\U`／`\N{}`）如实报未实现。
//! - AST／常量池各多一项 `Bytes`；`instantiate` 建 `BytesObject`；`.pyac` 的常量编码多一个
//!   tag（8 ＝ 长度 ＋ 原始字节），解码同步。
//! - `b'ab' + b'cd'` 与 `'a' + 'b'` 同一条路：**编译期**折成常量（`gen_compile_fixture.py`
//!   里那三条新用例把**指令流与常量池**逐字节对拍过）；运行期的 `bytes + bytes` 走
//!   `concat_public`。
//! - **定格数字（第 206 轮实测）**：`cargo test --workspace` ⇒ **452 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**68** 个二进制、**452** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 207 轮）`P1-12` 第三刀：`bytes` 方法面第一批 ✓**
//!
//! - 机制与生成器族共用：`bytes` 的 `getattr` 槽**现造**绑定方法对象（`OM-11`）。
//! - 12 个方法：`hex`／`decode`／`startswith`／`endswith`／`find`／`count`／`replace`／
//!   `upper`／`lower`／`strip`／`split`／`join`；20 条实测用例（结果一律用 `repr` 对拍）。
//! - **四条容易静默写错的都去量了**，其中两条我原来的写法就是错的：
//!   ① `strip(实参)` 是**字节集合**语义（`b'  ab  '.strip(b'a')` 原样返回，不是去空白）；
//!   ② `replace(b'', b'x')` 在**每字节之间**插一遍（`b'abc'` ⇒ `b'xaxbxcx'`）；
//!   ③ `split(b'')` ⇒ `ValueError: empty separator`；
//!   ④ `join` 收到非 bytes 项的消息**不给类型名加引号**（`…, int found`）。
//! - **定格数字（第 207 轮实测）**：`cargo test --workspace` ⇒ **453 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**68** 个二进制、**453** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 208 轮）`P1-12` 第四刀：`slice` 类型 ＋ 切片 ✓**
//!
//! - `SliceObject`（三个 `Option<i64>`）＋ `slice` 类型对象的 `new`／`repr` 槽 ＋
//!   `Instance::new_slice`；`repr` 形状照实测（`slice(1, 2, 3)`／`slice(None, None, None)`）。
//! - 切片求值按 CPython 的 `slice.indices()` 口径**手写**：负下标先加长度、再按步长方向夹到
//!   `[lower, upper]`；`step == 0` 报实测的 `ValueError: slice step cannot be zero`。
//!   `bytes`／`str`／`list`／`tuple` 走**同一套**边界（`str` 按**字符**切）。
//! - 夹具 `tools/gen_slice_fixture.py`：16 种切法 × 四族 ＝ 64 条 ＋ `slice` 的 5 条 repr
//!   ＋ 3 条错误。切片语义**跨类型共用** ⇒ 顺手把 `gen_bytes_fixture.py` 里那份切片段落撤了
//!   （**一处真相**）。
//! - 踩点留痕：探针第一版把 `value` 取在"列表切片赋值"**之后**，于是夹具里的接收者被那条
//!   探测就地改掉了（`[10, 1, 2, 3, 30, 40, 50]`）——修成"先取 `value`，再跑任何会改内容的探测"。
//! - **定格数字（第 208 轮实测）**：`cargo test --workspace` ⇒ **456 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**69** 个二进制、**456** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 209 轮）队列下一站 `marshal`（`CM-27`）✓**
//!
//! - **自有二进制格式**（版本 1，`crates/pyawa-stdlib/src/marshal_module.rs`）：tag ＋ 载荷；
//!   整数／大整数／浮点／`str`／`bytes`／`tuple`／`list`／`dict`／`set` 齐全。
//! - **循环引用用引用表**（`TAG_REF`）：参照 3.14 实测**也支持**（`l = []; l.append(l)` 往返回来
//!   还是自引用）⇒ 这条不是我们自创的口径；穿过 tuple／set 的环表示不了，如实报错并登记。
//! - **版本号是我们自己的**（参照实测 `5`）——`CM-27` 的"自有格式"判据在测试里断言
//!   `version != REFERENCE_VERSION`；**不追**字节兼容（规范明说属实现定义行为）。
//! - 错误口径照实测：空输入／未知 tag／截断三条与参照同句；格式版本不对是我们自己的消息
//!   （我们自有格式的第一字节就是版本号 ⇒ 参照那套 tag 口径不适用）。
//! - `dump`／`load` 要**文件对象**（fs 域／M3+）⇒ API 面齐备但如实报未实现。
//! - **给 stdlib 的安全面**：新增 `Instance::new_set`／`list_items`／`dict_entries`／
//!   `set_items`／`list_append`／`dict_insert_raw`／`set_insert_raw`（stdlib 是
//!   `forbid(unsafe_code)`，容器载荷只能走安全入口）。
//! - **定格数字（第 209 轮实测）**：`cargo test --workspace` ⇒ **462 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**462** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 210 轮）`P1-12` 第五刀：`bytes` 方法面第二批 ＋ `in` 的两条路 ✓**
//!
//! - 12 个方法（`rfind`／`index`／`rindex`／`removeprefix`／`removesuffix`／`lstrip`／`rstrip`／
//!   `zfill`／`splitlines`／`isdigit`／`isspace`／`__contains__`）；夹具 `methods` 段现 39 条。
//! - 三条容易想当然、实测纠正过的口径：`removeprefix`／`removesuffix` 没匹配时**原样返回**；
//!   `zfill` 的符号在**最前**（`b'-12'.zfill(5) == b'-0012'`）；`isdigit`／`isspace` 要
//!   **整串非空且全为**对应字符。
//! - **`in` 有两条路**：`CONTAINS_OP` → 执行器的 `contains`；方法面 → `__contains__`。
//!   两条都接上（只接一条就会出现"`b'a' in x` 与方法调用结果不一致"）。
//! - 同轮踩点：往 Rust 源码里塞字面量 `\n`／`\r` 时被这一层的字符串处理吃掉，写进去变成**真换行**，
//!   编译器直接报 `byte constant must be escaped`——修法是**显式拼**（`chr(92)`）而不是少一层转义。
//! - **定格数字（第 210 轮实测）**：`cargo test --workspace` ⇒ **463 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**463** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 211 轮）AB-62 整数十进制桥 ＋ 一处 M2 地基级的残留 ✓**
//!
//! - **AB-62**：`pa_tointstring`（任意整数 → 十进制**借用**视图）与 `pa_pushintstring`（十进制 →
//!   整数压栈）。两条都走 `str` 槽／`int()` 那条路（**一处真相**）：位数上限、接受哪些写法、
//!   消息全部跟着它走。`bytes` 按裁定用**既有**的 `pa_pushbytes`／`pa_tobytes`（从桩改真实现）。
//! - **对拍 harness 的收益立刻兑现**：新语料 `big_int_add.py` 一跑就抓到执行器的
//!   **i64／单例残留**——`BINARY_OP` 整条指令要求"结果落在单例区间内"，于是 `200 * 200` 与
//!   19 位字面量都报 `IntOutOfRange`。现在二元运算走 `concat_public`／`arithmetic_public`
//!   （`+` 顺带接上 `str`／`bytes`／`list`／`tuple` 拼接）、一元走 `unary_public`、
//!   `itertools.accumulate` 同一条路；删掉 `as_int`／`binary_op`／`push_int_result` 与作废的
//!   `ExecError::IntOutOfRange`。
//! - **两处测试曾在断言旧行为**（`200 * 200` 报错、`i64::MAX + 1` 编不过）⇒ 按真实口径重写。
//!   这轮再次印证：*"测试绿"只说明它测的那些成立*，口径变了必须回去读断言。
//! - **常量折叠差异**（已登记）：常量池只有 `Constant::Int(i64)` ⇒ `i64::MAX + 1` **不折**，
//!   交运行期算（语义等价、指令流不同）。
//! - **定格数字（第 211 轮实测）**：`cargo test --workspace` ⇒ **466 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**466** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **11/11**（0 已知差异、0 新差异）。
//! - **待裁**：`pa_type` 对 `bytes` 目前报 `PA_THANDLE`（`pa_tag` 无 bytes）——加 `PA_TBYTES` 属新面。

//! **（第 212 轮）编译器表达式面（M2 的地基）✓**
//!
//! - 二元 11 个（`+ - * / // % ** & | ^ << >>`）＋一元 3 个（`+ - ~`）进编译器：
//!   词法补 9 个单元、语法补完整优先级阶梯（`| < ^ < & < << >> < + - < * / // % < 一元 < **`，
//!   `**` 右结合且右侧可接一元 ⇒ `-2**2 == -4`）、发射走 `BINARY_OP`（下标按**符号**查表，`BC-39`）。
//! - **`+x` 不是 `UNARY_POSITIVE`**：3.14 实测是 `CALL_INTRINSIC_1 INTRINSIC_UNARY_POSITIVE`
//!   （那条指令 3.12 就没了）——下表按**名字**取，不写死下标。
//! - 常量折叠逐运算符接；三种**故意不折**（`/` 折成 float、除数为 0 在 `compile()` 就抛、
//!   负指数折成 float），每种都写了理由。
//! - **位置表**：一元取目标、未折叠二元取整段（都实测）；**嵌套二元**的收尾取内层复合表达式跨度
//!   （参照内部传播细节）⇒ **不猜**，夹具标 `positions_covered = false`，指令流仍逐字节比。
//! - 运行期补 `/`（结果 float、除零消息与 `//`／`%` 同句、大整数超 double 报 `OverflowError`），
//!   `NB_TRUE_DIVIDE` 一并接上；对拍 harness 现在也能渲染 **float**。
//! - 对拍语料 **13/13**：新增 `operators.py`（二元＋一元＋优先级）与 `true_division.py`（浮点结果）。
//! - **定格数字（第 212 轮实测）**：`cargo test --workspace` ⇒ **467 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**467** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 213 轮）括号／元组字面量／下标（读与写）✓**
//!
//! - **括号**是纯分组（**不加 AST 节点**——参照也不多发指令）；`()`／`(a, b)`／`(a,)` 是元组字面量。
//! - **元组字面量**：全常量折成**常量元组**并走既有的 `pending` 延迟登记（实测 `x = (1, 2)` 的
//!   `co_consts` 是 `[1, None, (1,2)]`：最左叶子先入表、元组在 `LOAD_CONST None` **之后**）；
//!   否则 `BUILD_TUPLE n`。裸元组 `x = 1, 2` 走新的「表达式列表」解析（**只给语句层用**——
//!   调用实参的逗号仍是分隔符）。
//! - **下标读** `a[i]`：3.14 没有单独的取下标指令 ⇒ `LOAD 容器; LOAD 键; BINARY_OP NB_SUBSCR`；
//!   后缀循环覆盖 `a[i][j]`／`f()[0]`／`a.b[0]`（**`a[0].b` 仍在外**，写进了 `P1-10` 的未接线）。
//! - **下标写** `a[i] = v`：发射顺序实测是「**值 → 容器 → 键 → `STORE_SUBSCR`**」，
//!   位置取**目标下标**那段。第一版我取"整条语句"⇒ 被夹具当场打回（`(0,8)` vs `(0,4)`）。
//! - **位置边界再次复现同一条规律**：嵌套复合表达式（嵌套二元／嵌套下标）的后继加载与收尾都取
//!   **内层**那段跨度 ⇒ 归入既有的 `positions_covered = false`（指令流与常量池仍逐字节比）。
//! - 对拍语料 **14/14**（新增 `subscript.py`：下标读＋写＋元组字面量）；编译夹具补 17 条。
//! - **定格数字（第 213 轮实测）**：`cargo test --workspace` ⇒ **467 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**467** 项）；
//!   `t_ab_1.py` ⇒ 绿。

//! **（第 214 轮）切片（编译器 ＋ 执行器）✓ —— 表达式面 ② 收尾**
//!
//! - 三种形态逐字节对拍：**常量界**折成 `Constant::Slice` 进**常量池**（`LOAD_CONST slice(1, 2, None)`，
//!   入表在 `None` **之前**）；**两段非常量**走 `BINARY_SLICE`（它**自己就是取下标**，
//!   第一版我多发了一条 `BINARY_OP []`，被夹具打回）；**三段**走 `BUILD_SLICE 3` ＋ `BINARY_OP []`。
//! - 缺的界**显式压 `LOAD_CONST None`**；超指令按"相邻两两"打（三段切片的四个操作数打两对）。
//! - 位置：`BINARY_SLICE` 取整段、`BUILD_SLICE` 取切片那段；**两段非常量切片的存入／收尾取目标**
//!   （普通下标与三段切片取整段）——"是不是复合"这条判据按**键的形态**分。
//! - 执行器：`BUILD_SLICE`／`BINARY_SLICE` 新建，切片对象**一律经 `slice` 类型的构造槽**（一处真相）；
//!   **list 切片写**支持长度变化与扩展切片（长度不等报参照实测的 `ValueError`）。
//! - 语料 `slices.py` **又抓出一处**：切片写的下标路径当时不认切片键 ⇒ 顺带补上。
//! - **定格数字（第 214 轮实测）**：`cargo test --workspace` ⇒ **468 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**468** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **15/15**。

//! **（第 215 轮）复核清单两条 ＋ `sys` 合约核实 ✓**
//!
//! - **③ `sys`**：合约在 `SPEC-c-modules.md` §5.2.3（早先已写），"不依赖能力域"那部分
//!   **已落地且 6 项测试全绿**（身份三条／语言版本两条／常量三条／`argv`·`path`·`modules` 形态／
//!   `getrefcount`／`int`↔`str` 上限两个入口）。`stdout`／`stderr` 按合约归 `_io`（`CM-26`）**仍在范围外**。
//! - **④a 探针不得有副作用**：机扫 22 个生成器 ＋ 逐读主体可变的几个 ⇒ **一处违规**
//!   （`gen_slice_fixture.py` 的 `__setitem__` 探针排在取 `value` 之前，第 213 轮已修）；
//!   其余主体**每用例重建**、迭代器**每探针各建**。
//! - **④b 安全入口不得漏布局（`OM-6`）**：安全入口只交**不透明句柄**与拷贝，stdlib 只构造
//!   core 公开的**原生函数／属性**类型，不碰容器载荷字段；无 `&mut` 别名、无容量 API ⇒ 合规。
//!   附记：`pyawa-abi` **内部**用核心布局类型是允许的（`OM-6` 禁的是"出现在 C ABI 签名里"，
//!   而 `pa.h` 只有不透明句柄）。
//! - **台账**：`§9.2` 里 `P1-7`／`P1-8` 按证据加删除线，`P1-6`／`P1-10` 标"大部分已收口"。
//! - **定格数字（第 215 轮实测）**：`cargo test --workspace` ⇒ **468 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **11/11**；
//!   `selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**468** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **15/15**。

//! **（第 217 轮）`not`／`is`／`in` 进编译器 ✓**
//!
//! - **比较族**：`is`／`is not` ⇒ `IS_OP`、`in`／`not in` ⇒ `CONTAINS_OP`（arg 0／1，`BC-58`）；
//!   运行期这两条**早就**接线（本轮只补编译器）。六族在函数里都打 `LOAD_FAST_BORROW_LOAD_FAST_BORROW`。
//! - **`not` 四种下场**（第一处"按上下文改发射"的地方）：值上下文里 `not <名字>` ⇒ `TO_BOOL; UNARY_NOT`；
//!   `not (a is b)`／`not (a in b)` ⇒ **翻转比较参数**；`not (a < b)` ⇒ 比较带 `bool(...)` 位 ＋ `UNARY_NOT`；
//!   **条件上下文** ⇒ `not` **推进跳转**（`if not a:` 用 `POP_JUMP_IF_TRUE`，没有 `UNARY_NOT`）；
//!   双重 `not` 抵消（只留 `TO_BOOL`）。
//! - 折叠：`not 0`／`not 1` ⇒ `bool` 常量；`is`／`in` 的常量形态**不折**（与参照一致）。
//! - 位置表又清出三条**参照内部**传播细节（两族收尾跨度不同、`not` 推进后的收尾、双重 `not` 取内层）
//!   ⇒ 标 `positions_covered=false` 并写明理由；指令流与常量池仍逐字节比。
//! - 语料 `membership.py` **又抓出一处真错**：`not in` 的识别里把 `in` 当成 `Name`（它是关键字单元）
//!   ——与上一条"`in` 不是 `Name`"是同一类错，两处都栽过。
//! - **定格数字（第 217 轮实测）**：`cargo test --workspace` ⇒ **471 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**471** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **16/16**。

//! **（第 218／219 轮）`and`／`or` ＋ 增强赋值 ✓**
//!
//! - **`and`／`or`**（3.14 的形态与 3.12 之前完全不同）：值上下文是
//!   `COPY 1; TO_BOOL; POP_JUMP_IF_*; NOT_TAKEN; POP_TOP`，**末操作数当值求**；嵌套**融合**
//!   （内层非末操作数跳到内层末操作数 `NOT_TAKEN` 之后的落点，内层末操作数按外层继承的条件跳）；
//!   条件上下文**不保留值**（无 `COPY`／`POP_TOP`），非末操作数按自身极性跳、落点看"决定值是否就是
//!   `cond`"（**体入口**或跳过体）。折叠取"决定结果的那个操作数"的位置。
//! - **借用/拥有加载**：boolop 的直接裸名操作数用 `LOAD_FAST`（`COPY` 要两份引用），其余仍借用。
//! - **增强赋值**：名字／属性／下标三种目标的栈序逐一实测；运行期 `NB_INPLACE_*` 接上——
//!   不可变类型等价基运算，**`list` 的 `+=` 就地 extend**（别名可见），`set`／`dict` 的就地运算
//!   如实报未接线（不悄悄换成重新绑定）。
//! - **语料两次抓出真错**：`s += 'b'` 走错入口（`arithmetic_public` vs `concat_public`）；
//!   以及 `not in` 里把 `in` 当成 `Name`（它是关键字单元）。
//! - 位置表：boolop 操作数与增强赋值的目标/后继指令是参照的**粘性 loc** ⇒ 那批用例标
//!   `positions_covered=false`（指令流与常量池仍逐字节比）。
//! - **定格数字（第 218／219 轮实测）**：`cargo test --workspace` ⇒ **471 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**471** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **18/18**。

//! **（第 220 轮）`elif` ＋ `MS-17` 行号级检查 ✓（含一次覆盖事故的修复）**
//!
//! - **先认错**：第 218 轮加 `and`／`or` 时的一次批量正则把 **43 条用例的 `covered` 误改成 `False`**
//!   （连理由也覆盖了）⇒ 它们被**整段跳过**、指令流根本没在对拍。本轮全部恢复并把理由改正确。
//!   教训：**批量正则改夹具登记，必须回头逐条读**（与"探针有副作用"同类的自查）。
//! - **`elif`**：参照实测与「`else:` 里套 `if`」同形 ⇒ `parse_if_chain` 递归；顺手修掉
//!   **链尾隐式 return 每层各补一条**的 bug（参照：`n` 条分支 ⇒ `n+1` 对）。
//! - **`MS-17`**：`positions_covered=false` 只跳过**列跨度**；**行号与 `co_lines()` 照比**
//!   （新增 `lines_covered` 标志，同样由理由前缀驱动）。
//! - 行号级检查**抓出 17 条真缺口**（4 循环 ＋ 7 `if` 族 ＋ 6 其它）：参照粘性 loc 让体沿用条件的行号。
//!   逐条写明实测差异；**这是已知缺口**，不是"已属观测面"（要不要实现粘性 loc 模型待裁）。
//! - **定格数字（第 220 轮实测）**：`cargo test --workspace` ⇒ **471 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**471** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **18/18**。

//! **（第 221 轮）统一后缀链 ＋ 矩阵乘 `@`／`@=` ✓**
//!
//! - **统一后缀链**：`.`／`(`／`[` 由三段 `while` 合成**一条循环** ⇒ `a[0].b`／`a.b[0].c`／
//!   `a[0][1].b`／`f()[0].b` 都能解析。中途踩坑：给每段插 `continue` 时按花括号配平，**格式串里的
//!   `{}`** 让配平失手（`continue` 插进实参循环 ⇒ **死循环**）⇒ 改用按块边界切分；另外循环缺 `break`。
//! - **`@`／`@=`**：`NB_MATRIX_MULTIPLY`（4）与 `NB_INPLACE_MATRIX_MULTIPLY`（17）都接上；
//!   运行期对未支持类型**如实报** `TypeError: unsupported operand type(s) for @: 'int' and 'int'`
//!   （语料用**未捕获异常**对拍，两侧同一条消息）。
//! - 语料**又抓出两处**：执行器漏了非就地的 `NB_MATRIX_MULTIPLY`；`items[0].v = …` 是
//!   **链式赋值目标**（尚未接线，已记为缺口）。
//! - 后缀链的位置传播未推出（最后一个复合子表达式的跨度会粘到后续）⇒ 6 条链式用例标未覆盖。
//! - **`break` 的难点已量清**：参照把**循环后代码复制到 break 路径**（`for` 补 `POP_TOP`、`while` 补 `NOP`），
//!   正常退出那条另有副本 ⇒ 需要**块结构模型**（与粘性 loc 同源；规则未推，故本轮不硬拼）。
//! - **定格数字（第 221 轮实测）**：`cargo test --workspace` ⇒ **471 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**471** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **20/20**。

//! **（第 222 轮）链式赋值目标 ＋ `pass` ＋ 空模块 ✓**
//!
//! - **目标链统一**：语句层原来"裸名字／属性／下标"三段各写一遍 ⇒ 合成**一个目标链解析器**
//!   （`名字` 后接任意串 `[键]`／`.名字`，按**最后一跳**选 `STORE_ATTR`／`STORE_SUBSCR`；
//!   增强赋值同理）⇒ `items[0].v = […]` 编得过（上一轮语料抓出的缺口）。
//! - **`pass`**：实测**不产生任何指令**（连 `NOP` 都没有）⇒ `Statement::Pass(位置)`，
//!   发射时只把位置留给收尾（`epilogue_span`）；**空模块**同样可编（去掉"没有语句"的报错）。
//!   这条让 `class C: pass`／`def f(): pass` 的行号级检查从"未覆盖"变**通过**。
//! - **`try`／`except` 侦察**：运行期**已就绪**（`PUSH_EXC_INFO`／`CHECK_EXC_MATCH`／`POP_EXCEPT`／
//!   `RERAISE` 与异常表调度都在）⇒ 纯编译器工作，下一轮做。
//! - **定格数字（第 222 轮实测）**：`cargo test --workspace` ⇒ **471 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **20/20**。

//! **（第 223 轮）`break`／`continue` ✓（`continue` 逐字节；`break` 语义一致、布局不同）**
//!
//! - **`continue`**：`JUMP_BACKWARD` 回循环起点 ⇒ **指令流与参照逐字节一致**；
//!   之后的同块语句是**死代码**（参照不发）。
//! - **`break`**：`for` 先 `POP_TOP` 掉迭代器，`break` 跳过 `else` 体；但参照把"循环后的代码"
//!   **复制**到 break 路径（`while` 还先发 `NOP`）⇒ 本层用 `JUMP_FORWARD` 跳到循环之后，
//!   **语义一致、布局不同**。按"不许硬拼"：那两条形状**不进编译夹具**，由语料
//!   `break_continue.py` 守（含 `break` 跳过 `else`／`while` 里 `continue`）⇒ **21/21**。
//! - 顺带两处流分析（实测差异）：终止语句之后的死代码不发；体必然终止时循环尾回跳不发。
//! - `return` 在循环体内先 `POP_TOP` 的清理是**另一处块结构差异**，已记为缺口。
//! - **定格数字（第 223 轮实测）**：`cargo test --workspace` ⇒ **471 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **21/21**。

//! **（第 224 轮）`try`／`except` ✓（编译器发射 ＋ `BC-54` 异常表第一次到达运行期）**
//!
//! - **管线**：`CompiledUnit` 新增 `exceptiontable`（此前 `instantiate` 硬编码**空表** ⇒ 异常表
//!   根本到不了运行期），`.pyac` 编解码补上该字段。
//! - **发射**照参照实测：`PUSH_EXC_INFO` **只发一次**（后续处理块只做类型检查）、
//!   `CHECK_EXC_MATCH`／`POP_JUMP_IF_FALSE`／`NOT_TAKEN`、**有 `as 名字` 时 `STORE_NAME` 直接
//!   吃掉异常实例**（不先 `POP_TOP`）、`POP_EXCEPT` ＋ 名字清理、清理块 `RERAISE 0`／
//!   `COPY 3; POP_EXCEPT; RERAISE 1`；异常表按 6-bit varint 编码。
//! - **两处真 bug**（测试当场抓到）：最后一个处理块末尾漏 `JUMP_FORWARD` ⇒ `StackUnderflow`；
//!   内层 `raise` 把"作用域要收尾"标志置假、但异常被外层 `try` 接住 ⇒ 末尾少一对隐式 return。
//! - **布局**与 `break` 同口径（跳到公共末端，不复制参照的块结构）⇒ 语义一致、布局不同。
//! - **定格数字（第 224 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**472** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **22/22**。

//! **（第 225 轮）修一处自认的测量 bug：夹具位置配对错位 ⇒ 覆盖标志改为实测驱动**
//!
//! - **错在哪**：`tools/gen_compile_fixture.py` 用 `zip(get_instructions(code), code.co_positions())`
//!   取逐指令位置——但 `co_positions()` 按**码元**含 inline cache，`get_instructions()` 默认不显示 cache
//!   ⇒ 前面只要有带 cache 的指令，后面就整体错位。正确配对是每条指令的 `Instruction.positions`。
//!   后果：第 217–224 轮里一大批"位置没对齐"的**理由与标记其实是这个测量 bug**（还据此写下过
//!   "参照让 `if` 体沿用条件的行"之类推断）⇒ 已修、已重测、已更正 `PLAN`。
//! - **改造**：新增 `tools/compile-positions-census.tsv`（源码 → 理由）作为**唯一事实**，
//!   生成器按它打 `positions_covered`／`lines_covered`；SOURCES 里 92 条过时理由全部清掉。
//! - **重测结果（第 225 轮）**：244 条用例里**位置可比 102 条**、**行号可比 230 条**；
//!   差异 139 条（含 11 条"参照位置是 `None`，本层表达不了「缺失」"）＋行号 14 条。
//! - 同轮真修了一处编译器行为：`for`／`while` 的回跳位置取**循环体最后一条**（参照如此）。
//!   另试了 `if` 收尾跨度一版，普查显示**无效果** ⇒ 已撤回。
//! - **定格数字（第 225 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **22/22**。

//! **（第 226 轮）按正确配对重推位置规则：位置可比 102 → 129、行号可比 230 → 236**
//!
//! 第 225 轮修好夹具生成器的配对错位后，参照的逐指令位置第一次可信 ⇒ 本轮重推并修掉六处规则：
//! **`LOAD_ATTR` 取属性表达式自身**（原来取**对象**的跨度，是错位数据留下的错规则）、
//! **`RETURN_VALUE` 只有字面量常量取值自身**（其余取整条 `return`；第 221 轮那两条"下标／属性取值跨度"
//! 也是错位产物）、**循环回跳粘性继承上一条指令**、**`for` 收尾取可迭代对象**、
//! **无 `else` 的 `if`／`while` 收尾取条件尾指令**、**`elif` 链尾巴取最末子句条件尾**、
//! **有 `else` 时收尾跟 else 那条路**。
//!
//! 余下：位置 107 条、行号 8 条（逐条在 `tools/compile-positions-census.tsv`；5 条行号缺口属带注解
//! `def` 的 `__annotate__` 合成单元，3 条属 `else` 分支之后的收尾传播）。
//! - **定格数字（第 226 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **22/22**。

//! **（第 227 轮）行号级 244/244 全绿（`MS-17` 无豁免）＋ 位置可比 129 → 143**
//!
//! 再收五条规则（都是"正确配对后的实测"）：`else` 体之后的收尾跟 else 路（`for…else`／`if/elif/else`）、
//! 函数收尾改用 `epilogue_span`、`and`／`or` **骨架指令**取整个布尔表达式的跨度、
//! `__annotate__` 里加载注解类型取**注解自身**的跨度（新增 `Parameter.annotation_span`／
//! `Statement::Def.returns_span`）、`Assign` 的 `epilogue_span` 提到两种形态共用（原来函数里
//! `x = <局部名>` 那条路径**漏设**）。
//!
//! ⇒ 夹具 244 条**行号全部可比**（`MS-17` 的行号级要求现在**没有任何豁免**）；余下 98 条只差列跨度
//! （11 条是"参照位置是 `None`／本层表达不了缺失"，1 条是嵌套注解子项跨度，其余是逐族列跨度传播）。
//! - **定格数字（第 227 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **22/22**。

//! **（第 228 轮）赋值/增强赋值"按目标取跨度"⇒ 位置可比 143 → 221（行号保持 244/244）**
//!
//! 又清掉两条**错位测量**留下的错规则：普通赋值的 `STORE_*`／作用域收尾**一律取目标**跨度
//! （`x = a + 1`／`a[1]`／`a.b`／`f()` 的 `STORE_NAME` 都是 `(0,1)`；原"复合右值取整段"整段删除），
//! 增强赋值的 `BINARY_OP` 取整条语句而 `STORE_*`／收尾取**目标／目标链**
//! （`x %= 2` ⇒ `(0,1)`、`a.b += 2` ⇒ `(0,3)`、`a[i] += 2` ⇒ `(0,4)`）。
//!
//! 余下 **20 条只差列跨度**（11 条"参照位置是 `None`"、4 条布尔嵌套、2 条 `not not`、2 条链式目标、
//! 1 条嵌套注解子项），逐条在 `tools/compile-positions-census.tsv`；行号级 **244/244 无豁免**。
//! - **定格数字（第 228 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（**70** 个二进制、**472** 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **22/22**。

//! **（第 229 轮）块结构模型：`break`／`try` 的布局逐字节对齐（A 步收口）**
//!
//! - `emit_block` 改**带余部**遍历（`emit_statement(statement, rest)`）＋ 块尾标签；作用域体统一走它。
//! - **`break`** ＝ `POP_TOP`／`NOP` ＋ **就地复制"循环之后的语句"** ＋ 作用域收尾。
//! - **`try`** ＝ `NOP` 打头；套体／各处理块出口各重放余部＋收尾；`PUSH_EXC_INFO` 只发一次；
//!   不匹配 → 下一块（最后一块 → `RERAISE 0`；**裸 `except:` 不发**）；`as 名字` 时清理区先来
//!   一遍"名字清理 ＋ `RERAISE 1`"；作用域收尾按"所有出口是否终止"判定。
//! - **源码序预登记**（`pre_intern`）：`co_names` 与"小整数进常量表"按**编译顺序**（复制路径会抢先登记）。
//! - 夹具 **+12 条**（7 `break`、5 `try`）**指令流／常量池／名字逐字节**；合成指令的 `None`
//!   位置／行号逐条写明"本层表达不了缺失"。当前 **256** 条用例：位置可比 **228**、行号可比 **252**。
//! - 仍未做：**循环体内的 `return`**（参照用"跳进共享块"／`SWAP;POP_TOP`，属另一族）；
//!   `raise` 不清理迭代器（已实测记录）。
//! - 过程教训（自认）：一次误替换吞掉了 `Emitter` 结构体／impl 与前若干函数（约 370 行），
//!   已从 HEAD 取回区段并逐条重贴，随后全闸门验证。
//! - **定格数字（第 229 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **22/22**。

//! **（第 230 轮）`with`（单条目）：指令流／常量池／名字逐字节 ＋ 方法调用表达式语句**
//!
//! - 照实测骨架：`LOAD_SPECIAL __exit__`／`__enter__` ＋ `CALL`，正常出口 `CALL 3`，清理块
//!   `PUSH_EXC_INFO; WITH_EXCEPT_START; … RERAISE 2`，末尾 `COPY 3; POP_EXCEPT; RERAISE 1`；
//!   异常表两条（受保护区 → 清理块 `depth` 2；清理块 → 末尾 `depth` 4）。
//! - 夹具 **+3 条**逐字节对上；语料 `with_statement.py`（直行路径）⇒ 对拍 **23/23**。
//! - 顺带补 `obj.method(…)` 这类**方法调用表达式语句**（此前误报"未接线"）。
//! - **待修（已写进 `PLAN` §9）**：`with` 的**异常出口**没真正调到 `__exit__`
//!   （异常确实抛出且外层接住，但 `__exit__` 只加 1 次计数而非 2 次）；**多项** `with` 暂报 `Unsupported`。
//! - **定格数字（第 230 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **23/23**。

//! **（第 231 轮）`with` 收口：异常出口 ＋ 多项**
//!
//! - **根因（两处都是"参照的 `with` 异常表条目带 `lasti`"）**：
//!   ① `WITH_EXCEPT_START` 的取项要按「异常／prev／`lasti`／self／可调用」数（原来按 3／4 取，
//!   `__exit__` 根本调不到 ⇒ 语料里 `exit_total` 少 1）；② 每层清理块后面各跟一份自己的
//!   `COPY 3; POP_EXCEPT; RERAISE 1`。
//! - 多项 `with` 也接上：逐项 `LOAD_SPECIAL` 进栈、**逆序**退出、**逆序**清理；内层"处理过"
//!   跳回外层退出调用（`JUMP_BACKWARD_NO_INTERRUPT`）；异常表按层给 `depth = 2×层数`。
//! - 夹具 **+1 条**（两项）⇒ 共 260 条；语料扩到**多项 ＋ 异常路径**（`last_exit` 观察内层先退）⇒ **23/23**。
//! - 顺带修正既有的 `tests/with_statement.rs`（手工汇编）——它原本按"不带 `lasti`"的形状写，与参照不符。
//! - **定格数字（第 231 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **23/23**。

//! **（第 232 轮）`lambda`：指令流／常量池／名字／位置逐字节**
//!
//! - **解析**：`parse_lambda`（形参表与 `def` 同族：位置／`*args`／裸 `*` 后的仅关键字／`**kw`／默认值；
//!   **无**注解与 `/`），体是一条表达式；`lambda` 是 `Name`（不是词法关键字），在 `parse_atom` 里分流。
//! - **发射**：嵌套单元 `co_name`／`co_qualname` 都是 `<lambda>`（函数里是 `<f>.<locals>.<lambda>`），
//!   体就是那条表达式的 `Return`；与 `def` **共用**新抽出的 `emit_function_object`
//!   （默认值元组／仅关键字映射 → `LOAD_CONST <code>` → `MAKE_FUNCTION` → `SET_FUNCTION_ATTRIBUTE` 16→2→1）。
//! - **顺带修**：`compile_scope` 的 `co_flags` 之前不算 `0x10`（`CO_NESTED`）——嵌套 `def` 尚未接线
//!   所以没暴露；现在按 qualname 里的 `.<locals>.` 判定（`lambda` 嵌在函数里 ⇒ `flags = 19` ✓）。
//! - 夹具 **+5 条**（无参／`x+1`／默认值＋`*a`＋`**k`／当实参／函数里的 lambda）⇒ 265 条、
//!   位置可比 233、行号可比 258；语料 `lambda_expr.py`（默认值／仅关键字／当实参）⇒ **24/24**。
//! - **定格数字（第 232 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **24/24**。

//! **（第 233 轮）布尔嵌套的骨架跨度逐层取（位置可比 233 → 234）＋ 推导式侦察**
//!
//! - **规则**：布尔链里"某操作数之后的骨架指令"取**拥有该操作数的那个布尔节点**的跨度；
//!   **末操作数**之后的骨架归**父层**（实测 `a and b or c` 里 `a` 之后是内层 `and` 的 `(4,11)`、
//!   `b` 之后是外层 `or` 的 `(4,16)`）。`emit_test_value` 的递归里按层设／还原该跨度。
//! - 效果：`x = a and b or c` 等已转绿；位置差异 **139 → 19**（11 条"参照位置是 `None`"、
//!   1 条嵌套注解子项、7 条其它列跨度族），位置可比 **234**、行号可比 **258**（行号 0 缺口）。
//! - **推导式侦察（下一块，已量清）**：3.12+ 推导式**内联**（`LOAD_FAST_AND_CLEAR` 保存外层同名局部 ＋
//!   `BUILD_LIST`／`LIST_APPEND` ＋ 融合指令 ＋ **整段异常表保护**）；缺的运行时只有
//!   `LOAD_FAST_AND_CLEAR` 与 `STORE_FAST_LOAD_FAST` 两条 opcode（其余都已实现）。
//! - **定格数字（第 233 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **24/24**。

//! **（第 235 轮）清单推导式：内联形态逐字节（夹具 4 条 ＋ 语料模块级全绿）**
//!
//! - **三条实测规则**：① `STORE_FAST_LOAD_FAST` 压回的那份值只抵消**紧接着的一次**目标读取；
//!   ② `if` 形状 `TO_BOOL; POP_JUMP_IF_TRUE → 元素; NOT_TAKEN; JUMP_BACKWARD → 循环`；
//!   ③ **清理块外提**到所在语句块末尾（模块在作用域收尾后、函数在 `RETURN_VALUE` 后）。
//!   推导式目标**只在推导式内部**当局部（模块级同名变量别处仍是 `STORE_NAME`／`LOAD_NAME`）。
//! - **运行期**：补 `LOAD_FAST_AND_CLEAR`／`STORE_FAST_LOAD_FAST`；`STORE_FAST` 遇 NULL 哨兵＝清空槽；
//!   `LIST_APPEND`／`SET_ADD`／`MAP_ADD` 的取容器改为 `peek_from_top(oparg)`（`PEEK` 从**弹出后的
//!   新栈顶**数；`FOR_ITER` 不弹迭代器）——既有的 `tests/containers.rs` 手工用例已按参照形状改正。
//! - **仍未收口**：函数作用域里那条推导式运行期**多压一份**元素（字节却对得上，路径待定位）、
//!   多重 `for`、集合／字典推导式；另发现"全常量列表字面量"参照会折成 `LIST_EXTEND`（与本轮无关）。
//! - **定格数字（第 235 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **25/25**；夹具 268 条（位置可比 237、行号可比 259）。

//! **（第 236 轮）函数作用域推导式收口 ＋ 集合推导式 ＋ 修一处 SIGSEGV**
//!
//! - **函数作用域那条**根因：`emit_two_operands` 见"两个局部名"就打成 `LOAD_FAST_BORROW_LOAD_FAST_BORROW`，
//!   而左操作数本应被 `STORE_FAST_LOAD_FAST` 压回的那份值抵消 ⇒ **多压一份** ⇒ `LIST_APPEND` 取到迭代器。
//!   护栏：先看 `pending_fused_load`，命中就只发右操作数。
//! - **集合推导式**：`Comprehension { kind: List | Set }`（只换 `BUILD_SET`／`SET_ADD`），夹具两条逐字节；
//!   语料 `comprehension_set.py`（去重／`if`／成员判定）⇒ 对拍 **27/27**。
//! - **修 SIGSEGV**：`contains` 把 `set` 与 `dict` 合在一支、**把 set 强转 `DictObject`** 再遍历 `entries()`
//!   ⇒ 类型混淆读越界（编译器此前造不出集合故未触发）⇒ set 走自己那份（`SetObject::items()`）。
//! - **仍未接线**：字典推导式（含元组目标）、多重 `for`、集合字面量 `{1, 2}`。
//! - **定格数字（第 236 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **27/27**；夹具 271 条（位置可比 240、行号可比 260）。

//! **（第 237 轮）推导式收口：字典／元组目标／多重 `for`／条件链**
//!
//! - 夹具 **+5 条**全部逐字节：`{k: 1 …}`、`{k: k + 1 … if k}`、`{k: v for k, v in s}`、
//!   `[a + b for a in s for b in t]`、`[x for x in s if p if q]`；语料 `comprehension_dict.py` ⇒ **28/28**。
//! - **实测要点**：`ADD` 的 oparg ＝ 1＋层数（两层 ⇒ `LIST_APPEND 3`）；保存/还原用 `SWAP 目标数+1`
//!   与**逆序** `STORE_FAST`；存目标可与"紧接着的读"打成 `STORE_FAST_LOAD_FAST`（外层不融合）；
//!   元组目标 `UNPACK_SEQUENCE` ＋ `STORE_FAST_STORE_FAST`；**条件链**为真跳**下一条**（短路语义，
//!   此前写成"都跳元素"被夹具抓出）；字典的键可由融合值提供、键值最左都是局部名时打超指令。
//! - **未接线**：集合字面量 `{1, 2}`；元组目标只支持两项；全常量列表字面量参照折 `LIST_EXTEND`。
//! - **定格数字（第 237 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **28/28**；夹具 276 条（位置可比 245、行号可比 260）。

//! **（第 238 轮）f-string 收口（② 完成）：指令流／常量池／名字／位置逐字节**
//!
//! - 3.14 的家族是 `FORMAT_SIMPLE`／`FORMAT_WITH_SPEC`／`CONVERT_VALUE`／`BUILD_STRING`（四条
//!   指令表与执行器里早就有）⇒ 本轮全在编译器：**词法**认 `f`／`rf`／`fr` 前缀（收成
//!   `Lexeme::FStr { contents, offset }`，`offset` 是内容起始列）⇒ **切片**（`{{`／`}}` 转义、
//!   单个 `}` 报错、按 `!`／`:` 分割、规格递归）⇒ 插值里的表达式**重新词法并平移跨度** ⇒ 发射。
//! - 位点细节（实测）：字面段取**字面文字**那段；插值段取整个 `{…}`；规格内部的 `BUILD_STRING`
//!   取**规格那段**（`{x:>{w}}` ⇒ `(8,13)`）而 `FORMAT_WITH_SPEC` 仍取 `{…}`；纯字面量的 f-string
//!   在解析期降成 `Str` 并取**内容**跨度（`f"a"` ⇒ `(6,7)`），空内容才取整条。
//! - 夹具 **+9 条**逐字节；语料 `fstring_expr.py` ⇒ 对拍 **29/29**。
//! - **未接线**：f-string／普通字符串里的**转义**、跨行插值、三引号。
//! - **定格数字（第 238 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **29/29**；夹具 285 条（位置可比 254、行号可比 269）。

//! **（第 239 轮）列跨度：`not not` 收口；链式目标试后回退**
//!
//! - **规则**：偶数个 `not` 抵消时，留下的 `TO_BOOL`／`COMPARE_OP` 取**最内层 `not`** 的跨度
//!   （奇数取最外层）；`is`／`in` 族一律取最外层。⇒ 夹具两条由"未覆盖"转为**真比对通过**
//!   （位置可比 254 → **256**，案卷 19 → **17** 条）。
//! - **试过回退**：链式目标改取"目标链跨度"能修 2 条、却让类体 `self.x = 1` 一族 **5 条**失配 ⇒ 回退（净亏 3）。
//! - **案卷刷新教训（已写进案卷头）**：重建脚本每轮都会把理由**缩短**，导致"参照位点是 `None`"
//!   这类条目在下一次重建时被过滤掉（第 239 轮就丢过 5 条与注解 1 条）⇒ 刷新要从**提交版**取回再增量改。
//! - **剩 17 条**：11 条 `None`（要动位点表数据结构）、1 条嵌套注解子项、3 条带括号布尔链、2 条链式目标。
//! - **定格数字（第 239 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **29/29**；夹具 285 条（位置可比 256、行号可比 269）。

//! **（第 240 轮）列跨度：带括号的布尔链收口（位置可比 256 → 259）**
//!
//! - **统一规则**：`and`／`or` 节点的跨度取**它自己那段解析的 token 区间**——外层链从第一个 token
//!   （可能是 `(`）到最后一个 token（可能是 `)`），括号内的那层从它自己的第一个 token 起。
//!   `x = (a and b) or (c and d)` 的骨架因此是 `(4,26)`、内层 `a and b` 是 `(5,12)`；
//!   比第 233 轮"按操作数首尾算"更准（无括号时等价）。
//! - 实现要点：`parse_and_test`／`parse_or_test` **在函数入口**记 `start_span`，收尾用
//!   `start_span.to(spans[cursor - 1])`。记点若放到"解析完第一个操作数之后"就会停在 `and` 上，
//!   三条原本通过的用例会集体失配（本轮踩过，已挪回入口）。
//! - ⇒ 夹具三条由"未覆盖"转为**真比对通过**：位置可比 **256 → 259**，案卷 **17 → 14**。
//! - **剩 14 条**：11 条 `None`（要动位点表数据结构）、1 条嵌套注解子项、2 条链式目标
//!   （"目标链跨度"能修 2 条但会让类体 `self.x = 1` 一族 5 条失配 ⇒ 回退，规则待推）。
//! - **定格数字（第 240 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **29/29**；夹具 285 条（位置可比 259、行号可比 269）。

//! **（第 241 轮）列跨度：链式目标收口 ⇒ 第 233 轮那 7 条可推族全部收口（位置可比 259 → 261）**
//!
//! - **规则**：赋值目标的 `STORE_ATTR`／收尾跨度分两种——**链里含下标**（`a[0].b = v`）取**目标链**那段的
//!   `(0,6)`；**纯属性链**（类体 `self.x = 1`）取**整条语句**的 `(3,3,8,18)`。
//!   第 239 轮"一刀切取目标链"正是因此净亏 3 条（带坏纯属性链 5 条）⇒ 本轮按 `contains_subscript`
//!   分流：2 条链式目标转绿、类体一族保持绿。
//! - ⇒ 位置可比 **259 → 261**，案卷 **14 → 12**；**第 233 轮普查里可推的 7 条全部收口**
//!   （2 链式目标、3 带括号布尔链、2 `not not`）。
//! - **剩 12 条**：11 条"参照位点是 `None`"＋1 条嵌套注解子项 —— 要**动本层位点表的数据结构**
//!   （让它能表达"缺失"），属规格边界，等用户裁定。
//! - **定格数字（第 241 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **29/29**；夹具 285 条（位置可比 261、行号可比 269）。

//! **（第 242 轮）集合字面量 `{a, b}`（不需裁定的缺口收口）**
//!
//! - `{1, 2}`／`{a, b}`／`{a}` 此前是**语法错误**；现在解析成 `SetLiteral(items)`、发射逐元素
//!   再 `BUILD_SET n`。夹具 **+3 条**逐字节；语料 `set_literal.py` ⇒ 对拍 **30/30**。
//! - **未接线**：参照对"**≥3 个全常量**元素"折成 `BUILD_SET 0; LOAD_CONST frozenset(…); SET_UPDATE 1`
//!   （`{1, 2, 3}`；两个元素不折）——要加 `Constant::FrozenSet` 与折阈值，属另一族。
//! - **定格数字（第 242 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **30/30**；夹具 288 条（位置可比 264、行号可比 272）；
//!   位置案卷 **12 条**（11 条 `None` ＋ 1 条嵌套注解，均需改位点表数据结构）。
//!
//! **本轮目标（第 12–20 轮）终局盘点**：A 完成；B 完成 4／5（`with`／`lambda`／推导式／f-string，
//! `import` 待规格）；C 的位置差异 139 → **12**（可推的 7 条全部收口，余 12 条要动位点表数据结构）。

//! **（第 243 轮）`BC-4` 扩：位置元素可空（能力缺口收口）＋ 它暴露的一批真差异**
//!
//! - **表示**：位置四元组每项 `Option<u32>`；发射侧 `emit_core(Option<Span>, …)` ＋ `emit_none`；
//!   **合成指令**按参照给全 `None`（类体 `MAKE_CELL`、`try` 的 `PUSH_EXC_INFO`、`try`／`with`／
//!   推导式的清理块、`as 名字` 的清理副本）。
//! - **可观察面**：`co_positions()` 缺项交 `None`；`co_lines()` 行号可 `None`；执行器取行遇缺失落
//!   `firstlineno`；`.pyac` 每元素加**存在位**（自有格式）。
//! - **夹具改成严格逐项比对**（不再"有 `None` 就跳过"）⇒ 立刻暴露并修好：
//!   ① 类里方法的 `co_flags` 多 `0x8000000`（`CO_METHOD`）；② 隐式收尾在**函数作用域**也补、
//!   且只在体能落下来时补；③ `AssignAttr` 的 `target_span` ＋"值＋对象"超指令；
//!   ④ 处理块路径的粘性跨度与 `RERAISE 0` 取最后处理块；⑤ 推导式骨架／`ADD`／条件跳转的跨度。
//! - **仍未对齐**：19 条已登记在 `tools/compile-positions-census.tsv`（**真正的列跨度差异**，
//!   不是能力缺口）。
//! - **定格数字（第 243 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **30/30**；夹具 288 条（位置可比 262、行号可比 272）。

//! **（第 244 轮）`import` 语句的编译器侧：9 条形态逐字节**
//!
//! - 语句：`import <点分名> [as <名>] (, …)*` 与 `from <点*><模块> import <名表>|*`（含括号表）；
//!   发射照实测：`LOAD_SMALL_INT <层级>; LOAD_CONST <fromlist>; IMPORT_NAME <模块>` ＋
//!   `IMPORT_FROM`／`STORE`／末尾 `POP_TOP`；`*` 走 `CALL_INTRINSIC_1 2`。
//! - 两条实测细节：**含点的别名**才发 `IMPORT_FROM`（`import b as c` 直接 `STORE c`）；
//!   函数里的导入名字是**局部**（`STORE_FAST`，`collect_locals` 里登记）。
//! - **运行期加载器未做**：规格 `IM-30` 要求 finder 落在 **Python 层**（继承
//!   `_bootstrap_external.FileFinder`）、`IM-31` 要求 loader 走能力层 ⇒ 与 **M3（`Lib/`）** 绑定，
//!   本层**不**用 Rust 私写顶替；`T-IM-1`…`T-IM-10` 待那一步。
//! - **自认**：上一轮自动"登记差异"的脚本把 `tools/gen_compile_fixture.py` 里含 `\n` 的源码串写坏
//!   （已随 `4bd1e0d` 提交）；本轮从 `3c845a8` 取回并重生成，全绿——那几条差异其实已被本轮修复治好。
//! - **定格数字（第 244 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **30/30**；
//!   夹具 **297** 条（位置可比 271、行号可比 281）。

//! **（第 245 轮）位置案卷清理：19 → 2 条，位置可比 271 → 284**
//!
//! - **修掉两条真差异**（推导式元素跨度）：字典推导式的条件跳转取"键:值"整段；
//!   `{k: v for k, v in s}` 的 `STORE_FAST_STORE_FAST` 取**首个目标名**的跨度。
//! - **删掉 11 条陈旧条目**：那些是 `BC-4` 扩之前的**能力缺口**（"表达不了 `None`"），
//!   第 243 轮补齐后已不是差异 ⇒ 删掉后这些用例**真正开始比对**（位置可比 271 → 284）。
//! - **剩 2 条**：① 嵌套注解子项（要改注解的数据结构、保留子跨度 —— 规格边界，待裁）；
//!   ② `with` 体内 `return` 的 `RETURN_VALUE` 取 `with` 上下文跨度（规则待推）。
//! - **定格数字（第 245 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **30/30**；
//!   夹具 **297** 条（位置可比 284、行号可比 281）。

//! **（第 246 轮）`try` 的 `else`／`finally`：三种布局逐字节**
//!
//! - `try/except/else`：套体 → **`else` 体** → 余部＋收尾（`else` **不在**受保护区内）；
//!   `try/finally`：正常路径就地发 finally，异常路径 `PUSH_EXC_INFO` ＋**再发一遍** ＋ `RERAISE`
//!   ＋ 清理三连；`except … finally`：处理块跑完 `JUMP_BACKWARD_NO_INTERRUPT` **跳回**正常路径的
//!   finally＋余部＋收尾，处理块链之后另发 finally 的异常路径（异常表四条）。
//! - **`co_names`／局部槽次序＝CPython 的编译顺序**（脱糖：内层 try/except 先、finally 后）
//!   ⇒ `body → else → 处理块 → finally`（写错时被夹具的 `names` 对比当场抓住）。
//! - 夹具 **+3 条**逐字节；语料 `try_else_finally.py`（`else` 只在无异常时跑、`finally` 三条路、
//!   `return` 路径上的 finally）⇒ 对拍 **31/31**。
//! - **定格数字（第 246 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致（70 个二进制、472 项）；
//!   `t_ab_1.py` ⇒ 绿；对拍语料 ⇒ **31/31**。

//! **（第 247 轮）循环体内的 `return`：每个外层 `for` 先丢迭代器（A 的最后一处遗留）**
//!
//! - **规则**（实测）：值是**常量** ⇒ 先 `POP_TOP`×n 再取值；其余 ⇒ 先取值再 `SWAP 2; POP_TOP`×n；
//!   `while` 不计；嵌套 `for` 每个丢一次；丢弃指令的位点与 `RETURN_VALUE` 同一条规则。
//! - **`break` 的复制路径在循环外** ⇒ 复制时把循环帧临时出栈（否则复制件里的 `return` 会多丢一次）。
//! - 夹具 **+8 条**（7 条逐字节通过）；语料 `return_in_loop.py` ⇒ 对拍 **32/32**。
//! - **仍登记 1 条**：循环体末尾是"体终止的 `if`"时参照把条件取反、回边换边 ⇒ 需要 **For/If 联合窥孔**。
//! - **定格数字（第 247 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **32/32**；
//!   夹具 **307** 条（位置可比 293、行号可比 288），案卷 **3** 条。

//! **（第 248 轮）For/If 联合窥孔：循环体末尾"体不落到末尾的 `if`"取反 ＋ 回边换边**
//!
//! - 规则：循环体**最后一条**是无 `else` 的 `if`、且体**不落到末尾**（`return`／`break`／`continue`）⇒
//!   `POP_JUMP_IF_TRUE → 体; NOT_TAKEN; JUMP_BACKWARD → 循环头; 体`（`for` 与 `while` 都适用；
//!   体**能**落到末尾、或这是 `if/else` 时不取反）。
//! - 实现：`emit_block` 标出候选（`in_loop_body` 只吃一次，嵌套块看不到）；`If` 臂**代发回边**，
//!   循环臂用**同一判据**让位（一处真相）。
//! - 夹具 **+6 条**全部逐字节；语料 `reversed_loop_tail.py` ⇒ **33/33**；**案卷 3 → 2**。
//! - **定格数字（第 248 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **33/33**；
//!   夹具 **312** 条（位置可比 299、行号可比 293）。

//! **（第 249 轮）集合字面量折叠：`Constant::FrozenSet`**
//!
//! - **规则**（实测）：集合字面量 **≥3 个元素且全常量** ⇒ `BUILD_SET 0; LOAD_CONST frozenset({…});
//!   SET_UPDATE 1`（`{1}`／`{1,2}`／含非常量 ⇒ 照旧 `BUILD_SET n`）；`{1,1,2}` 也折、去重。
//! - **折叠常量延迟入池**（与 `200 + 100` 同一条路）：`x = {1,2,3}` ⇒ `[1, None, frozenset]`。
//! - `.pyac` 标签 **10**（9 被 `Slice` 占用）；物化成集合对象；渲染 `frozenset:<排序元素>`。
//! - **运行期**：`sequence_items` 补集合分支（`SET_UPDATE` 的源是折叠常量）。
//! - 夹具 **+5 条**逐字节；语料 `set_folding.py` ⇒ **34/34**。
//! - **定格数字（第 249 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **34/34**；
//!   夹具 **317** 条（位置可比 304、行号可比 298）。

//! **（第 250 轮）字符串转义**
//!
//! - **已接线**：`\n`／`\t`／`\r`／`\\`／引号／`\a\b\f\v`／行继续／`\ooo`／`\xNN`／`\uNNNN`／`\UNNNNNNNN`；
//!   认不出的原样留下。解码抽成 `lex_string_escape`（普通串与原始串共用）；`r`／`rf` 前缀原样留反斜杠。
//! - **多行跨度**：字符串词素跨行（手写增行时**行首索引**同步更新，否则末列算成全局偏移）。
//! - **如实标两处未实现**：`\N{…}`（要 Unicode 名字表，M3 数据面）、f-string 字面段里的转义
//!   （要"源偏移 ↔ 解码后偏移"映射）。老测试里那条"转义未接线"的断言已换成真正的未实现项。
//! - 脚手架补 `\r`／`\b`／`\f`；**对拍探针的值里不能含真换行**（行式协议）⇒ 语料探布尔。
//! - 夹具 **+12 条**（10 条逐字节）；语料 `string_escapes.py` ⇒ **35/35**。
//! - **定格数字（第 250 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **35/35**；
//!   夹具 **328** 条（位置可比 313、行号可比 309）。

//! **（第 251 轮）f-string 字面段的源偏移映射**
//!
//! - `Lexeme::FStr` 携带**原文**（不解码）＋ `raw`；**切段时**按**源下标**解码
//!   （与普通字符串共用 `lex_string_escape`）⇒ 字面段位点天然按源算（`f"a\n{b}"` 的
//!   `LOAD_CONST` 是 `(6,9)`，`\n` 占源 2 列、解码后 1 列不再混用）。
//! - `rf'…'` 反斜杠原样留下；插值里的表达式拿到的也是原文。
//! - 上一轮标"未实现"的 `f"a\n{b}"` 转回 covered；语料 `fstring_escapes.py` ⇒ **36/36**。
//! - **定格数字（第 251 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **36/36**；
//!   夹具 **328** 条（位置可比 313、行号可比 309）。

//! **（第 252 轮）三引号／跨行 f-string ＋ 一处流程自认**
//!
//! - **三引号**在普通串、f-string、原始串三处都认，收尾连着三个同种引号。
//! - **f-string 切段逐字符跟踪行列**：跨行字面段／插值的位点落到真实行列（与参照一致）；
//!   正文起始列用"前缀列 ＋ 词内偏移"（跨行后当前行首会变，直接用会**下溢**）；
//!   `spec_span` 起点＝冒号那一列；宏展开不带括号 ⇒ 含 `if` 的表达式要先算好再传。
//! - **自认**：第 251 轮新增用例其实**没进夹具**——生成器里一条用例的 Python 源码转义写错
//!   （少一层反斜杠 ⇒ `SyntaxError`），而我把 `--emit` 输出吞掉 ⇒ 静默失败、夹具停在旧版，
//!   我却据此说已验证。**规矩**：生成夹具必须看退出码，条数对不上就不能继续。
//! - 夹具 **+4 条**；语料 `multiline_strings.py` ⇒ **37/37**。
//! - **定格数字（第 252 轮实测）**：`cargo test --workspace` ⇒ **472 passed / 0 failed**；
//!   `cargo check --workspace --all-targets` ⇒ **0 警告**；`check.py` ⇒ **12/12**；
//!   `selftest.py` ⇒ **22 项**；`stability.py` ⇒ 三连一致；`t_ab_1.py` ⇒ 绿；语料 ⇒ **37/37**；
//!   夹具 **332** 条（位置可比 318、行号可比 313）。
#![deny(unsafe_op_in_unsafe_fn)]

pub mod argdecode;
pub mod bigint;
mod builtin_objects;
mod cell;
mod classes;
mod code;
pub mod compile;
pub mod builtin_types;
pub mod decode;
pub mod executor;
mod format;
pub mod flags;
mod frame;
mod header;
mod instance;
mod macros;
pub mod opcode;
pub mod opcode_metadata;
mod refcount;
mod singleton;
mod type_object;
mod value;

pub use builtin_objects::{
    free_fixed_layout, python_level_finalize, AttributeObject, BoolObject, DictObject,
    ExceptionObject, FloatObject, FunctionObject, BuiltinFunctionObject, GeneratorObject,
    IntObject, IteratorObject, ListObject, MethodObject, NativeFn, NoneObject, NullObject,
    PlainObject, SetObject, StrObject, TupleObject,
};
pub use cell::CellObject;
pub use code::{code_getattr, CodeObject};
pub use executor::{
    attribute_read, attribute_write, call_value, execute, subscript_read, subscript_write,
    values_equal_public, ExecError, ExecOutcome,
};
pub use format::repr_float;
pub use format::SpecError;
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject, HEADER_SIZE_BYTES};
pub use instance::{Instance, INT_MAX_STR_DIGITS_DEFAULT, INT_MAX_STR_DIGITS_THRESHOLD};
pub use refcount::{Borrowed, Owned, PyRef};
pub use singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
pub use type_object::{
    HostDealloc, HostTraverse, HostVisit, Slots, TypeObject, GENERIC_ALLOCATION,
    HAS_INSTANCE_DICT,
};
pub use value::Value;
