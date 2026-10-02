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
//!   ⇒ 下一步给测试侧装内建即可（第 111 轮踩过这一点）
//!
//! **（第 129 轮）那条"已知规则"修掉了**：`Call` 的发射在**函数作用域里、被调用者是全局名**时，
//! 改发 `LOAD_GLOBAL <下标 << 1 | 1>`（"压 NULL"由**低位**承担）且**不发** `PUSH_NULL`；
//! 其余形态（模块级 `LOAD_NAME`／本地名／属性调用）照旧 —— 实测依据是
//! `def f(): raise ValueError(1)` 的参照字节码（`LOAD_GLOBAL 1; LOAD_SMALL_INT 1; CALL 1`）。
//! ⇒ 那条语料的**指令流现在逐字节一致** ✓；只剩**位点**不同（参照把 `raise <调用>` 那几条记在
//! **被调用者**的跨度上）⇒ 按实情标"位置表未对齐"。
//! 另外：`raise X from Y` 那一支本层把 `None` 当名字（`names` 多出 `None`）⇒ 语料暂未入库；
//! **端到端测试仍未加**（测试实例没装内建 ⇒ `ValueError` 解析不到），下一步补。
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
//! ⇒ `operator` 现在落地 **25** 个函数（比较 10 ＋ 算术 6 ＋ 一元 4 ＋ 位运算 5），
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
//! ⇒ `operator` 累计 **30** 个函数。
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
//! - `operator`（`SPEC-c-modules.md` §5.2.7）累计 **30** 个函数：比较 12 ＋ 算术 6 ＋ 一元 4 ＋
//!   位运算 5 ＋ `inv`／`index`／`contains` 3；生成脚本、夹具、单元测试随实现同笔入库
//!
//! **验证面**
//! - 语料**每一条都进指令比对**（此前的 `covered=False` 只该用于"位点豁免"，那是
//!   `positions_covered` 的职责）＋ 指令**条数**断言（防 `zip` 静默截断）
//! - 17 个生成脚本全部可复现（重跑不改工作区）
//!
//! **（第 140 轮）`operator.concat` 落地，并顺带修好 `add` 的序列行为**：核心新增
//! `executor::concat_public` —— `str`／`list`／`tuple` 拼接，其余落到 `arithmetic_public` 的 `+`
//! （整数相加、报实测消息）。**实测**：参照里 `concat(['a'], ['b'])` 与 `add(['a'], ['b'])`
//! **都是拼接** ⇒ 两者共用同一条路 ✓（此前我们的 `add` 只认整数，对序列直接报消息 ✗ ——
//! 那是一条**未记录的差异**，这一轮一并修掉 ✓）。⇒ `operator` 累计 **31** 个函数。
//!
//! **（第 141 轮）对账结果**（数字都是本轮实测，不是回忆）：
//! - `README` 的规格计数：**共 12 份、已写 12 份、待写 0 份**（由 `check.py` 机械校验）
//! - `itertools` **18** 个函数、`operator` **31** 个函数（与 §5.2.6／§5.2.7 的记载一致）
//! - 编译语料 **97** 条；**17** 个生成脚本全部可复现（重跑一遍，工作区零改动）
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
//! **C. 等你裁定才能动的四件**
//! ① `NewFn` 槽加异常通道（`chain.from_iterable` 前置）② `marshal` 义务边界
//! ③ `int` 宽度／溢出（任意精度是独立阶段）④ 把"持引用字段必须被 traverse／clear 覆盖"
//!    这条不变量立成 `CONSTRAINTS.md` 的编号条款（**编号待定**，现在只有测试形态）
//!
//! **（第 147 轮）`operator.call` 落地**（3.11 新增）：把实参转给可调用对象（`args.split_first`
//! ＋ `call_value`）。**实测消息**是 `call expected at least 1 argument, got 0`（我第一次硬编码了
//! 另一句 ✗，被实测纠正 ⇒ 现在按实测拼）；`call(1)` ⇒ `'int' object is not callable`（那由
//! `call_value` 的老路径报，已实测过）。⇒ `operator` 累计 **32** 个函数。
//!
//! **（第 149 轮）`operator.length_hint` 落地**：有长度给长度、没有给 `default`（默认 0）——
//! 走核心的**安全**入口 `Instance::length_of`（stdlib 禁 `unsafe`，这条路正好合适）。
//! 实测四条：`([1,2])`⇒2、`("abc")`⇒3、`(5)`⇒0、`(5, 9)`⇒9。⇒ `operator` 累计 **33** 个函数。
//!
//! **（第 150 轮）交接定格数字**（都用命令实测，不是估的）：
//!
//! - `cargo test --workspace` ⇒ **397 passed / 0 failed**
//! - `cargo check --workspace --all-targets` ⇒ **0** 警告/错误
//! - `python3 tests/ci/check.py` ⇒ **11/11**；`selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致
//! - 编译语料 **97** 条；`operator` **33** 个函数；`itertools` **18** 个函数
//! - 本地 `dev` 领先 `origin/dev` **109** 笔（未推送）
//!
//! **（第 155 轮复核）交接定格数字**（都用命令实测，不是估的）：
//!
//! - `cargo test --workspace` ⇒ **397 passed / 0 failed**
//! - `cargo check --workspace --all-targets` ⇒ **0** 警告/错误
//! - `check.py` ⇒ **11/11**；`selftest.py` ⇒ **20 项**；`stability.py` ⇒ 三连一致
//! - 编译语料 **97** 条；`operator` **33** 个函数；`itertools` **18** 个函数
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
//! **（第 169 轮）②`OM-11` 扩的性质变了：槽位今天**根本不会失败**，而且有一条静默错误行为** ✗
//!
//! - 事实：14 处 `with_new(...)` 给的都是**永不返回 `None`** 的实现（在 `*_new` 的函数体里
//!   `grep` 到的 `None` 收尾是 **0 个**）⇒ "`Option` 能表达失败"目前是**纸面能力** ✗
//! - 更要紧的：**纠正（第 175 轮读原文）**：`int_new` 是 `if !args.is_empty() { return None; }` ⇒ 有实参时
//!   **拒绝**（调用方报 `TypeError: cannot create 'int' instances` ✗ 类型与消息都不对，但**不是**静默给 0）
//!   —— 我此前把它写成"`int("a")` 会静默给 0" ✗，那是**错的**（只看到收尾那句 `Some(new_int(0))` 就推断了 ✗） —— 这不是"缺通道"，是**可观察的错误行为**（`MS-19` 只允许把参照未规定
//!   的东西登记为差异；这个参照**规定了**：`int("a")` 抛 `ValueError` ⇒ **必须修**）
//! - ⇒ ② 的真实形状：**(a) 通道**（签名改 `Result`，机械 ✓ 约一轮）＋ **(b) 每个类型的失败语义**
//!   （实参不匹配时按**实测消息**抛错 ✓ 多轮，且必须**先测参照**再写 ✗禁手写）
//! - ⇒ 本轮**没有**动签名：先把这个性质变化报给用户并按 ①②③ 问顺序（通道先行还是逐类型先行）
//!   —— 因为 (b) 的工作量远大于裁决里"约 6 处"的估计，且它会牵出"当前占位实现还有哪些"
//!
//! **（第 170 轮）②(a) 通道改动：一次失败的批量替换（已回退，记教训）** ✗
//!
//! - 我按"别名 ＋ 13 处签名 ＋ 调用点透传"一次性批量改，其中用了"行首 `Some(` → `Ok(`"这种
//!   **按模式**替换 ✗ ⇒ 撞到了**其它**返回 `Option<String>` 的函数（`builtin_function_repr`、
//!   `asend_repr` 等 3＋ 处）⇒ 编译错
//! - 更糟的是我第 169 轮的结论"14 处实现里一个 `None` 都没有"是**错的** ✗ —— 这次编译报错显示
//!   确有函数按实参分派并返回 `None`（如 `builtin_objects.rs` ≈2119 那个）⇒ 我当时那条 `grep`
//!   **模式写错了**（管道过滤把行号前缀匹配掉了 ✓），我没复核就下了结论 ✗
//! - ⇒ 处置：**立即 `git checkout` 回退三个文件**（tree 回到绿 ✓），本笔记留档
//! - ⇒ 正确做法（下一轮）：**逐个函数**改，先 `grep -n "pub unsafe fn [a-z_]*_new"` 拿到名单，
//!   再对每一个函数**单独**读全文、单独改签名与返回点（`None` 的每一处都要按**实测**给 `Err`）⇒
//!   不许再按模式批量替换 ✗；每改 3–4 个就跑一次 `cargo build` ✓
//! - **教训**：`grep` 的模式错了会给出**自信的错误结论** ✗ —— 下结论前必须复核一次（换一种查法 ✓）
//!
//! **（第 171 轮）②(a) 的准确名单（原始输出，不由我转述）**：
//!
//! - `builtin_objects.rs` 行 2115: plain_new(
//! - `builtin_objects.rs` 行 2130: attribute_new(
//! - `builtin_objects.rs` 行 2144: int_new(
//! - `builtin_objects.rs` 行 2156: bool_new(
//! - `builtin_objects.rs` 行 2171: float_new(
//! - `builtin_objects.rs` 行 2188: list_new(
//! - `builtin_objects.rs` 行 2205: dict_new(
//! - `builtin_objects.rs` 行 2222: set_new(
//! - `builtin_objects.rs` 行 2239: tuple_new(
//! - `builtin_objects.rs` 行 2253: str_new(
//! - `builtin_objects.rs` 行 2267: exception_new(
//! - 该文件里"四个空格开头的 `None`"共 **13** 处（**含非 `new` 函数** ⇒ 逐函数读时要区分 ✓；
//!   第 169 轮我那条错结论就是没做这个区分 ✗）
//! - 下一轮做法（照第 170 轮教训）：从上表**逐个**读全文 → 单独改签名 → `None` 处按**实测**给 `Err` →
//!   每 3–4 个跑一次 `cargo build` ✓；**禁止按模式批量替换** ✗
//! - `pyawa-core/tests/` 侧还有一处要同步：`executor.rs` 的唯一调用点透传（改完签名后一起动 ✓）
//!
//! **（第 172 轮）②(b) 的材料：11 个 `new` 的失败消息（原始实测输出）**
//!
//! 命令：对每个构造器给一个明显错的实参，捕获 `类型: 消息`（未做任何转述 ✓）：
//!
//! - `int('a')          ` ⇒ ValueError: invalid literal for int() with base 10: 'a'`
//! - `int([])           ` ⇒ TypeError: int() argument must be a string, a bytes-like object or a real number, not 'list'`
//! - `float('x')        ` ⇒ ValueError: could not convert string to float: 'x'`
//! - `float([])         ` ⇒ TypeError: float() argument must be a string or a real number, not 'list'`
//! - `str(1,2,3)        ` ⇒ TypeError: str() argument 'encoding' must be str, not int`
//! - `list(5)           ` ⇒ TypeError: 'int' object is not iterable`
//! - `tuple(5)          ` ⇒ TypeError: 'int' object is not iterable`
//! - `dict(5)           ` ⇒ TypeError: 'int' object is not iterable`
//! - `set(5)            ` ⇒ TypeError: 'int' object is not iterable`
//! - `object(1)         ` ⇒ TypeError: object() takes no arguments`
//! - `bool(1,2)         ` ⇒ TypeError: bool expected at most 1 argument, got 2`
//! - `ValueError(a=1)   ` ⇒ TypeError: ValueError() takes no keyword arguments`
//!
//! - ⇒ 这些就是 `NewFn` 槽在 `Err` 里该带的原因（`int("a")` 那条尤其重要：今天我们**只接零参形态** ⇒ 有实参一律拒绝 ✗）
//! - 按纪律，夹具要与**实现**同笔入库 ⇒ 本轮只存材料；下一轮起逐批实现、每批带上生成脚本与夹具 ✓
//!
//! **（第 173 轮）矛盾查清：`None` 收尾确实存在（是我三次"模式匹配"下错结论）** ✗
//!
//! - **读原文**（`sed -n 2110,2160p`）看到：
//!   `plain_new` 里 `if !args.is_empty() { return None; }`（≈2119）、
//!   `int_new` 里同样一句（≈2150）⇒ **第 170 轮的编译报错是对的** ✓
//! - 我一连三次栽在"模式匹配下结论"上：
//!   ① 第 169 轮：`grep | grep -c` 把行号前缀算进去 ⇒ 得出"一个 `None` 都没有" ✗
//!   ② 第 171 轮：把那份错结论当"原始输出"写进档（其实是错的 grep 结果）✗
//!   ③ 第 173 轮：Python 正则 `^\s+(return )?None,?\s*$` **漏了分号** ⇒ 又得出"没有 `None`" ✗
//! - ⇒ **硬规矩（从本轮起）**：判断"某函数里有没有某写法"**一律读原文**（`sed` 打印那一段）✓；
//!   模式匹配只能用来**定位行号**，**不能用来下结论** ✗
//! - **语义也读清了**（就地可得，不必猜）：`plain_new` 的 `None` ＝ `object() takes no arguments`（实测 ✓）；
//!   `int_new` 的 `None` ＝ "只接线了零参形态"（它自己的文档注释就写着"从字符串／其它类型构造随后补" ✓）
//!   ⇒ 后者**不是** Python 异常语义，而是**未实现** ⇒ 该报 `ExecError::Unsupported`
//!   （与 `TS-45` 那条"落地前越界必须如实报未实现"同一精神 ✓）；`int("a")` 真报 `ValueError` 要等
//!   (b) 的转换逻辑落地 ✓
//! - ⇒ ②(a) 与 (b) **不可分离** ✗（我的"通道先行"提议是错的 ✗）：要改签名就得给每个 `None` 一个
//!   正确的失败值，而"正确"取决于该处是"未实现"还是"该抛 Python 异常" ✓

#![deny(unsafe_op_in_unsafe_fn)]

pub mod argdecode;
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
pub use format::SpecError;
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject, HEADER_SIZE_BYTES};
pub use instance::Instance;
pub use refcount::{Borrowed, Owned, PyRef};
pub use singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
pub use type_object::{
    HostDealloc, HostTraverse, HostVisit, Slots, TypeObject, GENERIC_ALLOCATION,
    HAS_INSTANCE_DICT,
};
pub use value::Value;
