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
//!   码元逐条对拍）——**调用、容器、属性与下标、异常、生成器尚未接线**
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
//! - 字节码 §10 的**模式匹配族**：`MATCH_SEQUENCE`／`MATCH_MAPPING`（净 +1）、
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
//! - `TS-42` 的 **M2 阶梯**其余部分：函数对象／迭代器对象／`BaseException` 层次（表里已有 80 项）
//! - 字节码 §10 容器族的其余指令：`BUILD_SLICE`（要 M3+ 的 `slice` 类型）、`DICT_UPDATE`／
//!   `DICT_MERGE`（要字典源与重复键的 `TypeError`，异常对象未接线）
//! - **`OM-11` 的 `getattr`／`setattr` 槽位**：现在的查找顺序（实例字典 → 类型 MRO → 报错）
//!   写死在执行器里；类型可覆写的槽位与数据描述符随类型系统接线
//! - 取绑定方法（`obj.method` **不调用**）：要 `method` 类型，`TS-42` 排在后面的阶梯
//! - `LOAD_SUPER_ATTR`：要 `super()` 的 `__class__` cell
//! - 调用族的其余部分：闭包（`COPY_FREE_VARS`／`MAKE_CELL`／`LOAD_DEREF`…）、注解
//!   （`SET_FUNCTION_ATTRIBUTE` 的 `16`）、`CALL_FUNCTION_EX`（`*args`／`**kwargs` 展开）、
//!   生成器与协程；内建可调用与**绑定方法**（后者要属性族）
//! - 异常对象：所以绑定错误现在只能报**类别**（`T-BC-18` 的 `TypeError` 与消息待接线）
//! - `OM-11` 的 **`getattr`／`setattr` 槽位**（形状按 `SPEC-bytecode.md` §10 的注"由实现自选"：
//!   返回新引用／`None`）＋ `BC-4` 的第一批 `co_*` **计算型属性**
//!   （`co_name`／`co_qualname`／`co_filename`／`co_firstlineno`／`co_argcount` 一族／
//!   `co_varnames`／`co_names`／`co_consts`），
//!   走槽位而不是给内建类型旁路
//! - 字节码 §10 的**模式匹配族**：`MATCH_SEQUENCE`／`MATCH_MAPPING`（净 +1）、
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
//! - 字节码 §10 的**迭代族**：`GET_ITER`／`FOR_ITER`／`END_FOR`／`POP_ITER`／`GET_LEN`，
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
//! - `OM-14` 其余：**子类分派槽位**（`call`／`init` 一类要被 Python 子类覆写的那几个——
//!   要类创建钩子与绑定方法）、宿主对象的 `new` 槽位
//! - `repr`／`str` 其余：**容器元素**的 `repr` 还只走原生槽位（`TS-44` 说语义走属性通道，
//!   故元素上的 Python 级 `__repr__` 覆写暂时不生效——那要在载荷的槽位里回调执行器）、
//!   绑定方法 `repr` 里的 **qualname**（现在是 `co_name`）、`float` 的边界写法
//! - `CALL_INTRINSIC_1` 其余：`ASYNC_GEN_WRAP`、`PRINT`、`IMPORT_STAR`，以及 PEP 695 那一组
//!   （`TYPEVAR`／`PARAMSPEC`／`TYPEALIAS`／`SUBSCRIPT_GENERIC`／`PREP_RERAISE_STAR`…）
//!   ——**实测卡在依赖上**：`type X = int` 产出的是 `typing.TypeAliasType`、泛型参数是
//!   `typing.TypeVar`，两者都在 `Lib/typing.py` 里、**不是内建类型**（`TS-41` 的探测表里没有，
//!   故不能另造一个类型顶替）。⇒ PEP 695 要等 `P3-14` 的 `Lib/typing` 先落地
//! - 星号调用其余：`DICT_MERGE` 的同名键错误（要函数的 qualname）、
//!   `CALL_FUNCTION_EX` 的映射协议（现在只认 `dict`）
//! - 模式匹配族其余：`MATCH_CLASS` 的**位置形参**（本层只接了关键字形参）与"属性是方法"
//!   那一支（要绑定方法对象）、`MATCH_KEYS` 的 `__getitem__` 协议（现在只认 `dict`）
//! - 格式化族其余：迷你语言里**没实现**的写法（`n` 的本地化、数值的自定义填充细节、
//!   `.N` ＋ `g` 的组合等）在 `src/format.rs` 的文件头逐条列着，命中时如实报未实现
//! - 生成器族的其余面：`SEND` 只接线了**生成器**（普通迭代器那条随后补）、
//!   `GET_AWAITABLE`／`coroutine`／`async_generator`（`await` 那一半）、`CLEANUP_THROW`、
//!   生成器对象的方法（`send`／`throw`／`close`——要方法绑定与属性通道）、
//!   `CALL_INTRINSIC_1` 的 `STOPITERATION_ERROR` 与 `GeneratorExit`
//! - `with`（`BEFORE_WITH`／`WITH_EXCEPT_START`）、`except*`（intrinsic 族）与
//!   `sys.exc_info()` 的 Python 可见形态
//! - `__traceback__` 的追加与 `lasti` 的还原（`BC-60` 点名的最后一条，要 traceback 对象）
//! - `str(e)`／`repr`（`KeyError` 的 `args` 已是那个键，但还没法把它显示成 `'nope'`）
//! - `T-BC-22` 的**夹具对拍**面：`try`／`except`／`else`／`finally`／`with`／`except*` 的发射序列
//!   与可观察行为——表已经对拍（`tests/fixture-code-3.14.json`），派发用镜像骨架的手写用例锁住；
//!   逐程序的完整对拍要等 `MS-` 的 conformance harness
//! - 字节码 §10 的其余族：`§2.4` 的 `co_*`、生成器与协程、格式化、模式匹配、PEP 695
//! - **迭代协议**（`OM-11` 的 `iter` 槽位）：现在只有 tuple／list／dict／set／str 可迭代，
//!   用户类型要 `__iter__`／`__next__` 才能进 `for``
//!
//! **类型槽位的形状**（`OM-11`）：`§10` 的注允许实现自选 Rust 签名；`AB-37` 要求
//! `pa_newtype` 带**子类分派槽**，而 `§13-2` 的改动矩阵说"事后追加槽位 ＝ 主版本 +1"——
//! 所以这套形状定下来就是长期契约，动手前值得过一眼。
//! - `TS-40` 数值塔的其余部分（`int`／`float`／`complex` 的互操作与提升）
//! - 字节码 §10 起步指令集的其余部分（控制流、调用、容器、属性与下标、异常、生成器、
//!   格式化、模式匹配、PEP 695）与 §11 的下降规则
//! - 字节码 §2.4 的完整 `co_*` 表面（`co_names`／`co_varnames`／`co_positions()`…）
//! - 对象模型 §10 弱引用：**OM-27** 的 ② "先清弱引用"目前只是顺序上的占位点
//! - **OM-28** `gc` 模块的可见行为（需要模块系统，不属本层）
//! - 对象模型 §11 宿主对象（**OM-34**…**OM-37**）、**OM-13** C3 线性化、**OM-14** 宿主类型注册
//! - **OM-11** 槽位表里 `getattr`／`setattr`／`call`／`hash`／`richcompare`／`iter`／
//!   `repr`／`str` 这八个槽位（它们的签名取决于值表示与类型系统）
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

#![deny(unsafe_op_in_unsafe_fn)]

pub mod argdecode;
mod builtin_objects;
mod cell;
mod classes;
mod code;
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
    AttributeObject, BoolObject, DictObject, ExceptionObject, FloatObject, FunctionObject,
    BuiltinFunctionObject, GeneratorObject, IntObject, IteratorObject, ListObject, MethodObject,
    NativeFn, NoneObject, NullObject, PlainObject, SetObject, StrObject, TupleObject,
};
pub use cell::CellObject;
pub use code::CodeObject;
pub use executor::{
    attribute_read, attribute_write, call_value, execute, subscript_read, subscript_write,
    values_equal_public, ExecError, ExecOutcome,
};
pub use format::SpecError;
pub use frame::{Frame, FrameError};
pub use header::{Header, PyObject};
pub use instance::Instance;
pub use refcount::{Borrowed, Owned, PyRef};
pub use singleton::{Singletons, SMALL_INT_MAX, SMALL_INT_MIN};
pub use type_object::{Slots, TypeObject, HAS_INSTANCE_DICT};
pub use value::Value;
