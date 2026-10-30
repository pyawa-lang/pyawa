//! `builtins` 模块的**纯计算面**（`PLAN` §9.4 第 4 条）。
//!
//! 契约（`CM-4` 的"从 Python 看到的 API 与语义"）写在 `docs/SPEC-c-modules.md` §5.2.2。
//! 只做**不需要能力域、也不需要输出通道**的那些内建：需要平台（`print` 之类）的留给
//! `P3-14`／待裁口径。
//!
//! 本模块**只用安全函数**定义原生：核心的 `NativeFn` 是 `unsafe fn` 指针，而**安全 `fn`
//! 可以强转成它**（同一 ABI）⇒ stdlib 在 `#![forbid(unsafe_code)]` 下也能落地原生函数。

use core::ptr::NonNull;

use pyawa_core::bigint::BigInt;
use pyawa_core::{ExecError, Header, Instance};

/// 模块名（`builtins`）。
pub const NAME: &str = "builtins";

/// 本模块落地的内建函数名（按名字排序；测试与合约核对用）。
pub const IMPLEMENTED: &[&str] = &[
    "abs", "all", "any", "ascii", "bin", "bool", "callable", "chr", "dict", "dir", "enumerate", "float", "delattr", "getattr", "hasattr", "hash",
    "exec", "filter", "globals", "hex", "id", "int", "isinstance", "issubclass", "iter", "len", "list", "map", "max", "min", "next", "oct",
    "open",
    "ord", "range", "repr",
    "set", "setattr", "sorted", "str", "sum", "tuple", "type", "vars",
];

/// 建 `builtins` 模块的命名空间（**新引用** 的 `dict`）。
///
/// 内容：上表那些内建函数 ＋ **`__build_class__`**（由核心在引导期建好，`OM-14`）＋
/// `__name__`。**没有** `print`（要输出通道，口径待裁）、也没有需要能力域的那些。
pub fn build(instance: &Instance) -> NonNull<Header> {
    let namespace = instance.new_dict();
    let natives: &[(&str, pyawa_core::NativeFn)] = &[
        ("abs", abs_native as pyawa_core::NativeFn),
        ("ascii", ascii_native as pyawa_core::NativeFn),
        // **构造器一族**（第 130 轮）：`_bootstrap.py` 起手就缺 `list` ✓；都按 CPython 的最小面接线 ✓
        ("bool", bool_native as pyawa_core::NativeFn),
        ("dict", dict_native as pyawa_core::NativeFn),
        ("float", float_native as pyawa_core::NativeFn),
        ("getattr", getattr_native as pyawa_core::NativeFn),
        ("hasattr", hasattr_native as pyawa_core::NativeFn),
        ("setattr", setattr_native as pyawa_core::NativeFn),
        ("delattr", delattr_native as pyawa_core::NativeFn),
        // **`vars`** ✓（task-25 ①）：`importlib.metadata` 的 `vars(self).update(…)` 一族 ✓。
        ("vars", vars_native as pyawa_core::NativeFn),
        ("int", int_native as pyawa_core::NativeFn),
        ("list", list_native as pyawa_core::NativeFn),
        ("set", set_native as pyawa_core::NativeFn),
        ("str", str_native as pyawa_core::NativeFn),
        ("tuple", tuple_native as pyawa_core::NativeFn),
        ("type", type_native as pyawa_core::NativeFn),
        ("all", all_native as pyawa_core::NativeFn),
        ("any", any_native as pyawa_core::NativeFn),
        ("bin", bin_native as pyawa_core::NativeFn),
        ("callable", callable_native as pyawa_core::NativeFn),
        ("chr", chr_native as pyawa_core::NativeFn),
        ("hex", hex_native as pyawa_core::NativeFn),
        // **`hash()`**（task-31）：int／float／bool／str／bytes／tuple／None／complex（见下方实现块）
        ("hash", hash_native as pyawa_core::NativeFn),
        ("isinstance", isinstance_native as pyawa_core::NativeFn),
        ("__import__", import_native as pyawa_core::NativeFn),
        ("issubclass", issubclass_native as pyawa_core::NativeFn),
        ("len", len_native as pyawa_core::NativeFn),
        // **`dir`**（第 406 轮 ✓）：`pickle.py:190` 导入期就调 `dir()` ✓（核心实现，本层只挂名 ✓）。
        ("dir", pyawa_core::dir_new as pyawa_core::NativeFn),
        // **`next`**（第 142 轮）：`_bootstrap.py` 与语料都要它 ✓ ⇒ 复用执行器的 `advance` ✓
        ("next", next_native as pyawa_core::NativeFn),
        ("globals", globals_native as pyawa_core::NativeFn),
        ("eval", eval_native as pyawa_core::NativeFn),
        ("exec", exec_native as pyawa_core::NativeFn),
        // **`compile()`**（task-43）：core 的实现（`pyawa_core::compile_native`）——这里只注册 ✓。
        ("compile", pyawa_core::compile_native as pyawa_core::NativeFn),
        ("id", id_native as pyawa_core::NativeFn),
        ("iter", iter_native as pyawa_core::NativeFn),
        // **`reversed`** ✓（第 227 轮）：`_collections_abc.py:75` 要它 ✓。
        ("reversed", pyawa_core::reversed_new as pyawa_core::NativeFn),
        // **`zip`** ✓（第 229 轮）：`_collections_abc.py:81` 与 `os.py:563` 都要它 ✓。
        ("zip", pyawa_core::zip_new as pyawa_core::NativeFn),
        // **`super`** ✓（第 233 轮）：`Lib/abc.py` 的 `ABCMeta.__new__` 要它 ✓。
        ("super", pyawa_core::super_new as pyawa_core::NativeFn),
        // **`map`／`filter`** ✓（第 338 轮）：**急求值**（返回列表 ✓）—— 偏差见 core 里的说明 ✓。
        ("map", pyawa_core::map_new as pyawa_core::NativeFn),
        // **`enumerate`** ✓（第 347 轮）：**急求值**（返回 `(下标, 元素)` 的列表 ✓）—— 偏差见 core 的说明 ✓。
        ("enumerate", pyawa_core::enumerate_new as pyawa_core::NativeFn),
        ("filter", pyawa_core::filter_new as pyawa_core::NativeFn),
        ("max", max_native as pyawa_core::NativeFn),
        ("min", min_native as pyawa_core::NativeFn),
        ("oct", oct_native as pyawa_core::NativeFn),
        ("ord", ord_native as pyawa_core::NativeFn),
        // **`print`**（`CM-26` 的硬边界：走 `sys.stdout` ⇒ `_io` ⇒ `fs` 域 ✓，**禁止**临时 sink ✓）
        ("print", print_native as pyawa_core::NativeFn),
        // **`open`** ✓（task-18 ✓）：**一处真相**是 `_io` 那个 [`crate::io_module::open_native`] ✓
        // —— 这里只把**同一个实现**接到 `builtins` 上 ✓（不另写一份 ✓）。
        ("open", crate::io_module::open_native as pyawa_core::NativeFn),
        ("pow", pow_native as pyawa_core::NativeFn),
        ("repr", repr_native as pyawa_core::NativeFn),
        ("round", round_native as pyawa_core::NativeFn),
        ("divmod", divmod_native as pyawa_core::NativeFn),
        ("sorted", sorted_native as pyawa_core::NativeFn),
        ("sum", sum_native as pyawa_core::NativeFn),
    ];
    for (name, handler) in natives {
        let function = make_native(instance, name, *handler);
        instance.dict_set(namespace, name, function);
    }
    // **`object`**（第 132 轮）：参照里它是所有类的根 ✓（`class X(object)` 是很常见的写法 ✓，
    // `_bootstrap.py` 里就有 ✓）。类型对象在引导期已登记 ✓ ⇒ 按名字取出来放进名字空间 ✓。
    // 注意：`int`／`str` 等**内建类型名**本层目前是 native 函数（不是类型对象 ✗）⇒
    // `class X(int)` 与 `int.from_bytes` 这类用法要等"内建类型化"那一轮 ✓（已登记 ✓）。
    if let Some(object_type) = instance.type_named("object") {
        instance.dict_set(namespace, "object", object_type.cast());
    }
    // **`slice`**（第 148 轮）：类型对象**早已登记** ✓（`P1-10`／`slice_new` ✓）⇒ 与 `object` 同一
    // 手法：按名字取出来放进名字空间 ✓（此前 `slice(1, 3)` 报 `NameError` ✗）。
    // **异常类型进名字空间**（第 163 轮）：这些类型**早就建好了** ✓（`type_named(\"ValueError\")` 有 ✓），
    //   但**从没放进内建名字空间** ✗ ⇒ 实测 `ValueError` ⇒ `NameError: name 'ValueError' is not defined` ✓
    //   ⇒ 于是 `try: raise ValueError(...) / except ValueError:` 这类**最基础的**异常匹配也走不通 ✗
    //   （`Lib/` 里每一处 `try/except` 回退都靠它 ✓）。
    //   名字取自 `TS-41` 的探测表 ✓（**一处真相** ✓，不另抄一份 ✗）；判定用 `BaseException` 子类 ✓。
    //   **先 `retain` 再交给字典** ✓ —— `dict_set` 是「接管一份引用」的规矩 ✓（第 161 轮的堆损坏就是这么来的 ✗）。
    if let Some(base_exception) = instance.type_named("BaseException") {
        for entry in pyawa_core::builtin_types::BUILTIN_TYPES {
            let Some(ty) = instance.type_named(entry.name) else {
                continue;
            };
            if !instance.is_subtype(ty, base_exception) {
                continue;
            }
            instance.dict_set(namespace, entry.name, ty.cast());
        }
    }
    // **`classmethod`**（第 158 轮）：与 `slice`／`object` 同一手法 ✓（类型在引导期已登记 ✓）。
    // **先 `retain` 再交给字典** ✓（第 274 轮真 bug ✗）：`dict_set` 走的是"**接管**一份引用"的规矩 ✓
    // ⇒ 直接传 `type_named` 的**借用**视图 ✗ ⇒ 字典以为自己持有、注册表也以为 ✓ ⇒ 双双释放 ✗
    // ⇒ 字典里留下**悬垂条目** ✓（`MS-25` 的 `tcache`／`double-linked list` 崩溃就是这么来的 ✓；
    //   实测凶手键名 **`classmethod`** ✓：`super_zero_arg` 用例每次都能复现 ✓）。
    if let Some(classmethod_type) = instance.type_named("classmethod") {
        instance.dict_set(namespace, "classmethod", classmethod_type.cast());
    }
    if let Some(slice_type) = instance.type_named("slice") {
        instance.dict_set(namespace, "slice", slice_type.cast());
    }
    // **形态类／描述符类型**（第 152 轮）：这三个类型**早就在探测表里** ✓（`builtin_types.rs`
    // 有 `property`／`staticmethod`／`classmethod` ✓）；本层先按名字把它们放进名字空间 ✓
    //（与 `object`／`slice` 同一手法 ✓）；**描述符协议（`__get__`）未接** ✗ ⇒
    // `@property` 这类**还不能真正生效** ✗（已如实登记 ✓）。
    for descriptor_name in ["property", "staticmethod", "classmethod"] {
        if let Some(ty) = instance.type_named(descriptor_name) {
            // **先 `retain` 再交给字典** ✓（第 161 轮定位到的真因 ✓）：`dict_set` 走的是
            //   "**接管**一份引用"的规矩 ✓ ⇒ 直接传类型对象会让**注册表与字典都以为自己持有**
            //   同一份引用 ✗ ⇒ 双双释放 ⇒ 实测 100% 可复现的 `corrupted size vs. prev_size` ✓
            //   （`object`／`slice` 那两处之所以没事 ✓，是因为它们的引用计数另有来源 ✓）。
            instance.dict_set(namespace, descriptor_name, ty.cast());
        }
    }
    // `__build_class__`：核心在引导期已经建好（`OM-14`），这里原样放进 `builtins`
    if let Some(build_class) = instance.build_class() {
        instance.dict_set(namespace, "__build_class__", build_class);
    }
    // **内建类型化（第 181 轮，用户裁定 A ✓）**：`int` 这个名字应当是**类型对象** ✓（不是 native ✗）——
    // 这样 `int.__name__`／`int.__dict__`／`isinstance(x, int)`／`class X(int)` 才成立 ✓。
    // 构造逻辑本来就在 `int` 类型的 `new` 槽里 ✓ ⇒ 这里只**改指** ✓（**先 `retain` 再交给字典** ✓）。
    // 名字与**类型对象**一一对应地改指 ✓ —— 逐个来 ✓：**先只接已经验过构造槽的** ✓
    // （`int` 的 `new` 槽本就在 ✓；`type` 就是元类型 ✓，调用它即建类 ✓）。
    // 其余（`list`／`dict`／…）等各自的构造槽核过再改 ✗ —— 免得把 `list(...)` 这类构造弄坏 ✓。
    // **只留 `int`** ✗：`type` 改指元类型会让 `type()` 无参调用**变宽** ✗（参照抛 `cannot create
    // 'type' instances` ✓）——元类型的 **call 槽**得先按参照语义接好 ✓ 再改指 ✓。
    // **一次一个名字** ✓（第 182 轮节奏 ✓ —— 第 181 轮一次改五个当场红 ✗）：本轮只加 `dict` ✓。
    // `dict` 的类型对象已接 `dict_new` ✓、还带 `getattr` **方法面** ✓（`fromkeys` 就挂那儿 ✓）。
    // `type` 也改指**元类型** ✓（第 183 轮：劫持"调用类"的毛病已修 ✓ ——
    // `call_callable` 现在只在**元类型自身**被调用时走它的 call 槽 ✓，`C(...)` 一律走实例化 ✓）。
    // **逐个改指** ✓（第 184 轮：一次一个名字 ＋ 每次跑闸门 ✓ —— 第 181 轮一次五个当场红 ✗）。
    for name in ["int", "dict", "type", "list", "tuple", "set", "frozenset", "str", "range", "memoryview", "complex", "float", "bool", "bytes", "bytearray", "slice", "object"] {
    // **`str` 已改指** ✓（第 185 轮：`str_new` 的构造槽补齐了 ✓ —— 第 184 轮退回的原因 ✓）。
        if let Some(ty) = instance.type_named(name) {
            instance.dict_set(namespace, name, ty.cast());
        }
    }
    // **类级方法进类型字典** ✓（第 182 轮）：`dict` 的类型对象已惰性挂上命名空间 ✓ ⇒ 把 `fromkeys` 放进去 ✓。
    if let Some(dict_type) = instance.type_named("dict") {
        if let Some(type_namespace) = instance.type_namespace(dict_type.cast()) {
            let method = make_native(instance, "fromkeys", dict_fromkeys_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "fromkeys", method);
        }
    }
    // **`function.__code__`／`__globals__` 进 `function` 的类型字典** ✓（第 214 轮）：
    // `Lib/types.py` 要 `type(FunctionType.__code__)` ✓ 与 `type(FunctionType.__globals__)` ✓
    // —— 类级取法先前取不到 ✗（与 `str.join` **同款** ✓）。
    if let Some(function_type) = instance.type_named("function") {
        if let Some(type_namespace) = instance.type_namespace(function_type.cast()) {
            let code_method = make_native(instance, "__code__", pyawa_core::function_code_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__code__", code_method);
            let globals_method = make_native(instance, "__globals__", pyawa_core::function_globals_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__globals__", globals_method);
            // **`function.__get__`** ✓（第 410 轮，task-21 ✓）：`enum.py` 的 `_is_descriptor` 靠
            // `hasattr(obj, '__get__')` 把类体里的**普通函数**认成描述符 ✓（否则 `Flag._get_value`
            // 会被当成枚举成员 ✗）。
            let get_method = make_native(
                instance,
                "__get__",
                pyawa_core::function_descriptor_get as pyawa_core::NativeFn,
            );
            instance.dict_set(type_namespace, "__get__", get_method);
        }
    }

    // **`str.join` 进 `str` 的类型字典** ✓（第 212 轮）：`Lib/types.py:52` 是 `type(str.join)` ✓ ——
    // 方法面只挂在**类型的 `getattr` 槽**上 ✗ ⇒ 类级取法（`str.join`）先前取不到 ✗。
    // **一次一个名字** ✓（第 181 轮的教训 ✓）：本轮只加 `join` ✓。
    if let Some(str_type) = instance.type_named("str") {
        if let Some(type_namespace) = instance.type_namespace(str_type.cast()) {
            if let Some(handler) = pyawa_core::str_method_native("join") {
                let method = make_native(instance, "join", handler);
                instance.dict_set(type_namespace, "join", method);
            }
        }
    }

    // **`object.__new__` 进 `object` 的命名空间** ✓（第 193 轮）：参照里它**独立存在** ✓
    // ⇒ `@object.__new__` 不再被"元类那一层"顶掉 ✓（`Lib/genericpath.py:194` ✓）。
    if let Some(object_type) = instance.type_named("object") {
        if let Some(object_namespace) = instance.type_namespace(object_type.cast()) {
            let new_method = make_native(
                instance,
                "__new__",
                pyawa_core::object_new_native as pyawa_core::NativeFn,
            );
            instance.dict_set(object_namespace, "__new__", new_method);
            // **数据类型也要"看起来有 `__new__`"** ✓（第 252 轮）：`Lib/enum.py` 的 `_find_data_type_`
            // 靠 `'__new__' in base.__dict__` 认数据类型 ✓（第 247 轮探针实测本层缺它 ⇒ `ReprEnum` 族
            // 97 个模块 ✗）。**必须复用同一个 `new_method` 对象** ✓：构造分派
            // （`executor/call.rs`）会拿 `type_lookup(class, "__new__")` 与 `object` 的那一个**比指针** ✓，
            // 同一个才跳过 ✓ ⇒ 构造语义不变 ✓（第 248 轮挂裸函数就坏在这里 ✗）。
            // **各数据类挂自己的 `__new__`** ✓（第 410 轮，task-21 G2 ✓）：先前这 8 个类型全塞
            // **同一个** `object.__new__` 占位 ✗ ⇒ `str.__new__(子类, "abc")`／`int.__new__(子类, 5)`
            // 报 `object.__new__() takes exactly one argument` ✗（真 `enum.py` 的 `StrEnum.__new__`、
            // `Lib/http/__init__.py` 的 `HTTPMethod.__new__`／`HTTPStatus.__new__` 全压在这格 ✓）。
            // `new_method`（`object.__new__`）**只留在 `object` 上** ✓，这里换成本类型**自己的**
            // 原生实现（`T.__new__(子类, …)` 造**子类型**实例 ✓）；`has_own` 检查照旧 ✓ ——
            // `tuple` 的 core 注册真实现不被盖 ✓（namedtuple 的 `_tuple_new` ✓）。
            for (data_type, handler) in [
                ("str", pyawa_core::str_dunder_new as pyawa_core::NativeFn),
                ("int", pyawa_core::int_dunder_new as pyawa_core::NativeFn),
                ("float", pyawa_core::float_dunder_new as pyawa_core::NativeFn),
                ("bytes", pyawa_core::bytes_dunder_new as pyawa_core::NativeFn),
                ("list", pyawa_core::list_dunder_new as pyawa_core::NativeFn),
                ("dict", pyawa_core::dict_dunder_new as pyawa_core::NativeFn),
                ("set", pyawa_core::set_dunder_new as pyawa_core::NativeFn),
                ("tuple", pyawa_core::tuple_new_native as pyawa_core::NativeFn),
            ] {
                if let Some(ty) = instance.type_named(data_type) {
                    if let Some(namespace) = instance.type_namespace(ty.cast()) {
                        let has_own = instance
                            .type_lookup_owner(ty, "__new__")
                            .is_some_and(|(owner, _)| owner == ty);
                        if !has_own {
                            let own = make_native(instance, "__new__", handler);
                            instance.dict_set(namespace, "__new__", own);
                        }
                    }
                }
            }
        }
    }

    // **`complex.__new__`**（task-31／P3-34）：**构造语义在 stdlib**（core 只给"数字面"的
    // `new` 槽当"类型可构造"的保险）。按 `executor/call.rs` 的既有口径，类型字典里**非默认**的
    // `__new__` **优先于** new 槽 ⇒ 字符串解析、0/1/2 参与关键字文案都走这里 ✓。
    // 挂法照 `object.__new__` 那一段的规矩：类型字典 ＋ `make_native` ✓。
    if let Some(complex_type) = instance.type_named("complex") {
        let own = make_native(
            instance,
            "__new__",
            complex_dunder_new_native as pyawa_core::NativeFn,
        );
        instance.set_type_attribute(complex_type, "__new__", own);
    }

    // **数据类型的 `__str__`／`__repr__` 进类型字典** ✓（第 411 轮，task-22 G3 ✓）：
    // `enum.py` 的 `ReprEnum` 分支读 `member_type.__str__`／`__repr__` 给 `StrEnum`／`IntEnum`
    // 装成员格式化 ✓；先前落到元类兜底的 `object.__str__` ✗ ⇒ `str(StrEnum 成员)` 出 repr 形 ✗。
    // **只给 `str`／`int`／`float` 挂** ✓（`bytes`／容器族的参照 `__str__` 就是 `object.__str__`
    // ⇒ 落到实例 `__repr__` 覆写才对 ✓；给它们挂会抢掉子类（namedtuple 等）的 `__repr__` ✗）。
    for data_type in ["str", "int", "float"] {
        if let Some(ty) = instance.type_named(data_type) {
            if let Some(namespace) = instance.type_namespace(ty.cast()) {
                let str_method = make_native(
                    instance,
                    "__str__",
                    pyawa_core::data_dunder_str_native as pyawa_core::NativeFn,
                );
                instance.dict_set(namespace, "__str__", str_method);
                let repr_method = make_native(
                    instance,
                    "__repr__",
                    pyawa_core::data_dunder_repr_native as pyawa_core::NativeFn,
                );
                instance.dict_set(namespace, "__repr__", repr_method);
            }
        }
    }

    // **`type.__new__` 进 `type` 的命名空间** ✓（第 235 轮：配套的潜在 bug 本轮已修 ✓）。
    if let Some(type_type) = instance.type_named("type") {
        if let Some(type_namespace) = instance.type_namespace(type_type.cast()) {
            let new_method =
                make_native(instance, "__new__", pyawa_core::type_new_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__new__", new_method);
        }
    }

    // **（历史注记）** ✗：上一轮发现"`type` 命名空间里**多任一条** ⇒ 类级描述符访问就坏 ✗"——
    // 那是**独立的**潜在缺陷 ✓，本轮先修它、再把上面这段装回来 ✓。
    // **（历史注记，保留供追溯）**：
    // 只要 `type` 的命名空间里**多出任何一条**（`__new__` 或随便一个名字 ✓，实测用 `__probe__` 也一样 ✗），
    // **类级描述符访问**就坏 ✗：`class R: __get__…` ＋ `class E: r = R()` ⇒ `E.r` 返回**描述符本身** ✗
    //（参照给 `"readonly"` ✓），而 `E().r` **正常** ✓（`tests/conformance/corpus/descriptor_protocol.py` 的 A6 ✓）。
    // ⇒ 那是一条**独立的**（先前潜伏 ✓）缺陷 ✓，**先修它**再把 `__new__` 挂上来 ✓。
    // 本轮保留 `type_new_native` 与抽出的核心 ✓（**待命** ✓），链子暂时仍停在 `_abc_impl` ✗。

    // **描述符协议：`property.__get__` 进类型字典** ✓（第 279 轮接线 ✗ —— 此前只有"未接"的登记 ✓）：
    // `attribute_lookup` 的描述符分支认的**就是类型字典里的 `__get__`** ✓（`type_lookup` ✓）
    // ⇒ 挂在这里（**类型字典**，不是实例／槽位 ✓）类级与实例级访问走同一条 ✓。
    // **必须有它** ✗：`Lib/importlib/_bootstrap.py` 的 `spec.has_location`／`spec.cached`／`spec.parent`
    // 都是 `@property` ✓ —— 缺了它，属性访问交出 **property 对象本身** ✗（实测挡住 M3 的 import 链 ✓）。
    if let Some(property_type) = instance.type_named("property") {
        if let Some(type_namespace) = instance.type_namespace(property_type.cast()) {
            let get_method = make_native(
                instance,
                "__get__",
                pyawa_core::property_descriptor_get as pyawa_core::NativeFn,
            );
            instance.dict_set(type_namespace, "__get__", get_method);
        }
    }

    // **`dict.__init__` 进 `dict` 的命名空间** ✓（第 104 轮）：`dict.__new__` 只建空映射 ✓
    // ⇒ 填内容靠它 ✓（源可以是映射或成对的可迭代 ✓）。
    if let Some(dict_type) = instance.type_named("dict") {
        if let Some(type_namespace) = instance.type_namespace(dict_type.cast()) {
            let method = make_native(
                instance,
                "__init__",
                pyawa_core::dict_init_native as pyawa_core::NativeFn,
            );
            instance.dict_set(type_namespace, "__init__", method);
        }
    }

    // **`weakref.__new__` 进 `weakref` 的类型字典** ✓（task-7）：`ref.__new__` 在参照里是
    // weakref 类型**自己的** `tp_new` ✓ ⇒ Python 子类（`class KeyedRef(weakref.ref)` 的
    // `weakref.ref.__new__(type, ob, callback)` ✓，`Lib/importlib/_bootstrap.py` 的嵌套
    // `KeyedRef` ＋ `Lib/weakref.py` 的模块级 `KeyedRef` ✓）必须取到它 ✓；缺了它属性通道会
    // 翻到 `object.__new__` ✗ ⇒ 通用 `AttributeObject` 载荷，**目标根本没处存** ✗。
    if let Some(weakref_type) = instance.type_named("weakref") {
        if let Some(namespace) = instance.type_namespace(weakref_type.cast()) {
            let has_own = instance
                .type_lookup_owner(weakref_type, "__new__")
                .is_some_and(|(owner, _)| owner == weakref_type);
            if !has_own {
                let own = make_native(
                    instance,
                    "__new__",
                    pyawa_core::weakref_dunder_new as pyawa_core::NativeFn,
                );
                instance.dict_set(namespace, "__new__", own);
            }
        }
    }

    // **`object.__init__` 进 `object` 的命名空间** ✓（第 210 轮）。
    if let Some(object_type) = instance.type_named("object") {
        if let Some(type_namespace) = instance.type_namespace(object_type.cast()) {
            let method = make_native(instance, "__init__", pyawa_core::object_init_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__init__", method);
            // **`__str__`／`__repr__`** ✓（第 210 轮）：默认 `<X object at 0x…>` ✓（`Lib/types.py` 要 ✓）。
            let str_method = make_native(instance, "__str__", pyawa_core::object_text_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__str__", str_method);
            let repr_method = make_native(instance, "__repr__", pyawa_core::object_text_native as pyawa_core::NativeFn);
            instance.dict_set(type_namespace, "__repr__", repr_method);
        }
    }
    // **`Ellipsis`／`NotImplemented` 两个名字** ✓（第 215 轮）：`Lib/types.py` 要 `type(Ellipsis)` ✓
    // 与 `type(NotImplemented)` ✓（`EllipsisType`／`NotImplementedType` ✓）。
    // `None`／`True`／`False` 走**解析字面量** ✓，不在这一档 ✓。
    instance.dict_set(namespace, "Ellipsis", instance.singletons().ellipsis());
    instance.dict_set(namespace, "NotImplemented", instance.not_implemented());

    // **类型级描述符安装** ✓（`P3-17` 同族 ＋ `functools.wraps.__name__` 的正路）：
    // function 的 `__name__`／`__qualname__`／`__doc__`／`__module__` 可写 getset 数据描述符、
    // property 的 `__set__`／`__delete__`、staticmethod／classmethod 的 `__get__`。
    // 实现与**幂等**都在 core 的 `executor::protocol::install_builtin_descriptors` ✓ ——
    // 那里与上面这些安装段同一口径（类型字典 ✓、按实例 ✓、`OM-15` ✓）。
    pyawa_core::executor::protocol::install_builtin_descriptors(instance);

    let module_name = instance.new_str(NAME);
    instance.dict_set(namespace, "__name__", module_name);
    namespace
}

/// **`print`**（最小面）：`print(*args)` —— 实参**必须已经是 `str`** ✓（其余形态待 `str()` 落地 ✗）。
///
/// 目的地是**组合根装好的那个 `sys.stdout` 对象**（内建名字空间里的 `__stdout__` ✓）——
/// 字节经 `_io` 的文本层走 `fs` 域的 `write` 槽 ✓（`CM-26`：**不设临时 sink** ✓）。
fn print_native(
    instance: &Instance,
    _self_object: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let builtins = instance.builtins().ok_or(ExecError::Unsupported {
        opcode: 0,
        what: "`print` 需要内建名字空间（组合根装配）",
    })?;
    // 身份检查：`sys.stdout` 那个对象必须在（目的地与它是同一个 ✓）
    let _stdout = instance
        .dict_get(builtins, "__stdout__")
        .ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`sys.stdout` 尚未装配（`print` ⇒ `sys.stdout` ⇒ `_io` ⇒ `fs`）",
        })?;
    // 句柄与 `stdout` 对象**同源**（组合根一起装 ✓）：对象是身份、句柄是落点 ✓
    let handle_object = instance
        .dict_get(builtins, "__stdout_handle__")
        .ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`sys.stdout` 的句柄尚未装配（`print` ⇒ `sys.stdout` ⇒ `_io` ⇒ `fs`）",
        })?;
    let handle = instance
        .int_value(handle_object)
        .ok_or(ExecError::Unsupported {
            opcode: 0,
            what: "`__stdout_handle__` 不是整数",
        })? as u64;

    let mut bytes: Vec<u8> = Vec::new();
    for (index, argument) in args.iter().enumerate() {
        if index > 0 {
            bytes.push(b' ');
        }
        // **非 `str` 实参走 `str(x)` 的同一处实现**（第 330 轮）：`print(1)`、`print([1, 2])`、
        // `print(None)` 这些都是 `Lib/` 里遍地都是的写法 ✓ —— 先前一律如实拒绝 ✗
        // （消息还写着"`str()` 落地前" ✓，而 `str()` 早就落地了 ✓ ⇒ 这里补上 ✓）。
        // 走核心的 [`Instance::object_str_native`] ✓（一处真相：与 `str(x)` 同一个渲染 ✓）。
        let rendered = match instance.text_of(*argument) {
            Some(text) => text.to_owned(),
            None => instance.object_str_native(*argument)?,
        };
        bytes.extend_from_slice(rendered.as_bytes());
    }
    bytes.push(b'\n');
    crate::_io_module::write_bytes(instance, handle, &bytes)?;
    // **必须 `retain`**（返回给 VM 的是**新引用** ✓）：少了它，调用方会释放单例 ⇒
    // 双重释放 ⇒ 进程退出时报 `tcache_thread_shutdown(): unaligned tcache chunk detected` ✗
    // （第 94 轮实测：`print` 少了这一句 ✗）
    Ok(instance.retain(instance.singletons().none()))
}

/// 造一个原生可调用对象（**新引用**）。
pub(crate) fn make_native(instance: &Instance, name: &str, handler: pyawa_core::NativeFn) -> NonNull<Header> {
    let ty = instance
        .type_named("builtin_function_or_method")
        .expect("builtin_function_or_method 在引导期已登记（OM-13）");
    let object = instance.alloc(pyawa_core::BuiltinFunctionObject::new(
        ty,
        // `TypeObject::name` 要 `&'static str`：内建函数名是常量，泄漏一份即可
        Box::leak(name.to_owned().into_boxed_str()),
        core::cell::Cell::new(handler),
    ));
    object.into_raw().cast::<Header>()
}

/// 参数个数检查：不够就给统一的 `TypeError`（消息照参照实现的口径写）。
/// `dict.fromkeys(iterable, value=None)`（第 182 轮）：**先占位** ✗ —— 实现（遍历可迭代对象）随后补 ✓。
///
/// 为什么先占位：`Lib/types.py` 只需**取到**这个名字 ✓（它做 `type(dict.__dict__['fromkeys'])` ✓），
/// 而**类级方法**必须在**类型字典**里 ✓（实例方法面 `dict_getattr` 看不到它 ✗）。调用时**如实报未接线** ✓，
/// 不静默给错值 ✗。
// **`dict.fromkeys`** ✓：真实实现在 **core**（要看容器内部 ✓）⇒ 这里只转发 ✓（**一处真相** ✓）。
fn dict_fromkeys_native(
    instance: &Instance,
    bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **本 crate `forbid(unsafe_code)`** ✗ ⇒ core 那个跨 crate 的入口是 **`safe fn`** ✓（第 184 轮 ✓）。
    pyawa_core::dict_fromkeys_native(instance, bound, args, kwargs)
}

fn need_args(
    instance: &Instance,
    name: &str,
    args: &[NonNull<Header>],
    count: usize,
) -> Result<(), ExecError> {
    if args.len() < count {
        let message = format!(
            "{name}() takes at least {count} argument{} ({} given)",
            if count == 1 { "" } else { "s" },
            args.len()
        );
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    Ok(())
}

/// 类型名（诊断消息里用）。
fn type_name(instance: &Instance, object: NonNull<Header>) -> String {
    instance.type_name(instance.type_of(object))
}

/// 这个对象是不是 **`complex` 类型**（含子类；只查类型子类型，不碰载荷）。
fn is_complex_instance(instance: &Instance, object: NonNull<Header>) -> bool {
    instance
        .type_named("complex")
        .is_some_and(|complex| instance.is_subtype(instance.type_of(object), complex))
}

// --------------------------------------------------------------------------- //
// `hash()`（task-31，P3-34 面）：int／float／bool／str／bytes／tuple／None／complex
// --------------------------------------------------------------------------- //
//
// **口径**（本机 `python3` 3.14.4 逐值实测）：
// - int：core 的 `IntValue::hash`（`sign × (|x| mod 2^61-1)`、`-1` 改判 `-2`，一处真相）；
// - float／complex：core 的 `executor::hash_double_value`／`hash_complex_parts`（同一处真相）；
// - None：`0xFCA86420`（`hash(None)` ⇒ 4238894112）；
// - str／bytes：**siphash13（零密钥）**。参照默认每进程随机化（与默认 `python3` 不可比）；
//   本层**没有随机化**（固定零密钥）⇒ 与 `PYTHONHASHSEED=0 python3` 逐值一致（ASCII／bytes
//   已对拍；非 ASCII 照 CPython 的"按最大码点选内部宽度"口径：≤0xFF 取 1 字节、≤0xFFFF 取
//   u16 LE、否则 u32 LE）。**如实偏差**：无哈希随机化。
// - tuple：CPython 3.8+ 的 xxHash 组合口径（与参照逐值一致，不分进程）；
// - NaN：参照按对象地址哈希（进程内随机）⇒ 这里用 core 的 `hash_double_value` 口径（0）——
//   如实登记为偏差（core 的锚点也这么写）；
// - 参照明确不可哈希的 ⇒ `TypeError: unhashable type: 'X'`；其余**未接线**的可哈希内建 ⇒
//   如实 `NotImplementedError`（不拿身份哈希冒充）；用户类走它的 `__hash__`（身份哈希协议面）。
const HASH_NONE: i64 = 0x0FCA_86420;

fn hash_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "hash() takes no keyword arguments"));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("hash() takes exactly one argument ({} given)", args.len()),
        ));
    }
    Ok(instance.new_int(hash_object(instance, args[0])?))
}

fn hash_object(instance: &Instance, object: NonNull<Header>) -> Result<i64, ExecError> {
    let ty = instance.type_of(object);
    if ty == instance.singletons().none_type() {
        return Ok(HASH_NONE);
    }
    if let Some(integer) = instance.int_of(object) {
        return Ok(integer.hash());
    }
    if let Some(number) = instance.float_value(object) {
        return Ok(pyawa_core::executor::hash_double_value(number));
    }
    if let Some(text) = instance.text_of(object) {
        return Ok(hash_text(text));
    }
    if let Some(bytes) = instance.bytes_value(object) {
        return Ok(hash_bytes(bytes));
    }
    if let Some(items) = instance.tuple_items(object) {
        let mut lanes = Vec::with_capacity(items.len());
        for item in items {
            lanes.push(hash_object(instance, item)?);
        }
        return Ok(hash_tuple(&lanes));
    }
    if let Some((real, imag)) = instance.complex_value(object) {
        return Ok(pyawa_core::executor::hash_complex_parts(real, imag));
    }
    // 参照明确不可哈希的内建（参照置 `__hash__ = None`）⇒ 与参照同文案。
    let name = type_name(instance, object);
    for unhashable in ["list", "dict", "set", "bytearray"] {
        if instance
            .type_named(unhashable)
            .is_some_and(|base| instance.is_subtype(ty, base))
        {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("unhashable type: '{name}'"),
            ));
        }
    }
    // 用户类：参照走 `object.__hash__`（身份哈希）⇒ 本层调它的 `__hash__`（同一协议面）。
    let is_builtin = pyawa_core::builtin_types::BUILTIN_TYPES
        .iter()
        .any(|entry| entry.name == name);
    if !is_builtin {
        if let Some(hash_method) = instance.attribute_optional_of(object, "__hash__")? {
            if hash_method != instance.singletons().none() {
                let bound = type_name(instance, hash_method) == "method";
                let result = if bound {
                    pyawa_core::executor::call_value(instance, hash_method, &[], &[])?
                } else {
                    pyawa_core::executor::call_value(instance, hash_method, &[object], &[])?
                };
                let value = instance.int_value(result);
                instance.release(result);
                if let Some(value) = value {
                    return Ok(value);
                }
            } else {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("unhashable type: '{name}'"),
                ));
            }
        }
    }
    // 可哈希但本轮没接的内建（frozenset 等）：如实未实现，不拿身份哈希冒充。
    Err(instance.raise_builtin_error(
        "NotImplementedError",
        &format!(
            "`hash()`：本层只接 int／float／bool／str／bytes／tuple／None／complex（`{name}` 未接线，如实报）"
        ),
    ))
}

/// `str` 的哈希：空串 ⇒ 0；否则按 CPython 的**内部宽度**取字节再走 siphash13（零密钥）。
fn hash_text(text: &str) -> i64 {
    if text.is_empty() {
        return 0;
    }
    let mut largest = 0u32;
    for character in text.chars() {
        largest = largest.max(character as u32);
    }
    let bytes: Vec<u8> = if largest <= 0xFF {
        text.chars().map(|character| character as u8).collect()
    } else if largest <= 0xFFFF {
        text.encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect()
    } else {
        text.chars()
            .flat_map(|character| (character as u32).to_le_bytes())
            .collect()
    };
    hash_bytes(&bytes)
}

/// `bytes` 的哈希（siphash13 零密钥；空串 ⇒ 0）。
fn hash_bytes(bytes: &[u8]) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    let mut value = siphash13_zero(bytes) as i64;
    if value == -1 {
        value = -2;
    }
    value
}

/// CPython 3.8+ 的 tuple 哈希组合（xxHash 的四个常量 ＋ 31 位旋转）。
fn hash_tuple(lanes: &[i64]) -> i64 {
    const PRIME1: u64 = 11_400_714_785_074_694_791;
    const PRIME2: u64 = 14_029_467_366_897_019_727;
    const PRIME5: u64 = 2_870_177_450_012_600_261;
    let mut acc = PRIME5;
    for lane in lanes {
        acc = acc.wrapping_add((*lane as u64).wrapping_mul(PRIME2));
        acc = acc.rotate_left(31);
        acc = acc.wrapping_mul(PRIME1);
    }
    acc = acc.wrapping_add((lanes.len() as u64) ^ (PRIME5 ^ 3_527_539));
    if acc == u64::MAX {
        return 1_546_275_796;
    }
    acc as i64
}

/// `SipHash-1-3`（**零密钥**；CPython 的 `_Py_HashBytes` 同算法，`PYTHONHASHSEED=0` 即零密钥）。
fn siphash13_zero(data: &[u8]) -> u64 {
    fn round(state: &mut [u64; 4]) {
        state[0] = state[0].wrapping_add(state[1]);
        state[1] = state[1].rotate_left(13);
        state[1] ^= state[0];
        state[0] = state[0].rotate_left(32);
        state[2] = state[2].wrapping_add(state[3]);
        state[3] = state[3].rotate_left(16);
        state[3] ^= state[2];
        state[0] = state[0].wrapping_add(state[3]);
        state[3] = state[3].rotate_left(21);
        state[3] ^= state[0];
        state[2] = state[2].wrapping_add(state[1]);
        state[1] = state[1].rotate_left(17);
        state[1] ^= state[2];
        state[2] = state[2].rotate_left(32);
    }
    let mut state = [
        0x736f_6d65_7073_6575u64,
        0x646f_7261_6e64_6f6d,
        0x6c79_6765_6e65_7261,
        0x7465_6462_7974_6573,
    ];
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        let block = u64::from_le_bytes(chunk.try_into().expect("8 字节块"));
        state[3] ^= block;
        round(&mut state);
        state[0] ^= block;
    }
    let remainder = chunks.remainder();
    let mut tail = ((data.len() as u64) & 0xFF) << 56;
    for (index, byte) in remainder.iter().enumerate() {
        tail |= u64::from(*byte) << (8 * index);
    }
    state[3] ^= tail;
    round(&mut state);
    state[0] ^= tail;
    state[2] ^= 0xFF;
    for _ in 0..3 {
        round(&mut state);
    }
    state[0] ^ state[1] ^ state[2] ^ state[3]
}

// --------------------------------------------------------------------------- //
// `complex()` 构造器（task-31／P3-34）：语义挂在 `complex.__new__` 上。
// --------------------------------------------------------------------------- //
//
// 参照 3.14.4 实测（全部在 `target/recon/t31` 的对拍脚本里）：
// - `complex()` ⇒ `0j`；`complex(1)`／`complex(1.5)` ⇒ 实数部分；`complex(1+2j)` 原值；
// - 单参**位置**实参是 `str` ⇒ 解析：`"1"`／`"1j"`／`"(1+2j)"`／`" 1+2j "`／`"-1-2j"`／
//   `"infj"`／`"nan+nanj"`／`"1_0j"`（合法下划线）等；**非法** ⇒
//   `ValueError: complex() arg is a malformed string`；下划线位置非法 ⇒
//   `ValueError: could not convert string to complex: '<原串>'`（照参照的两种文案）；
// - 单参 `__complex__` 协议（返回必须是 complex，否则 `__complex__ returned non-complex (type X)`）；
//   没有 `__complex__` 时退 `__float__`（返回非 float ⇒ `<类型>.__float__ returned non-float (type X)`）；
// - 两参形态：`real`／`imag` 各取**实数**（int／float／`__float__`）；`complex` 实参照参照的
//   历史口径按复数运算收（`complex(1+2j, 3)` ⇒ `(1+5j)`），但 `__complex__` 只对 `real` 收
//   （`imag` 只认实数协议 ⇒ `complex() argument 'imag' must be a real number, not X`）；
// - 关键字：`complex(real=1)`／`complex(imag=2)`／`complex(1, imag=2)` 合法；关键字 `real="1"`
//   是**两参形态** ⇒ 不解析字符串（`must be a real number, not str`）；
// - 计数／文案优先级：关键字数 >2 ⇒ `takes at most 2 keyword arguments (N given)`；位置+关键字
//   总数 >2 ⇒ `takes at most 2 arguments (N given)`；未知关键字 ⇒ `got an unexpected keyword
//   argument '<名>'`；位置与同名关键字同给 ⇒ `argument for complex() given by name ('real')
//   and position (1)`；
// - **不发的警告**（如实偏差）：参照对"两参里给 complex／`__complex__` 的 real"发
//   `DeprecationWarning`，本层没有该警告通道（结果值一致）。
//
// **如实偏差**：`complex` 的 Python 子类构造未接线（core 的 `new_complex` 只造基类实例 ⇒
// 本层对 `complex.__new__(子类, …)` 如实 `NotImplementedError`，不造错类型的实例）。

fn complex_dunder_new_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(class) = args.first().copied() else {
        return Err(instance.raise_builtin_error("TypeError", "complex.__new__() 缺少 class 实参"));
    };
    let Some(complex_type) = instance.type_named("complex") else {
        return Err(instance.raise_builtin_error("NotImplementedError", "`complex` 类型未登记"));
    };
    if class != complex_type.cast::<Header>() {
        return Err(instance.raise_builtin_error(
            "NotImplementedError",
            "`complex` 的子类构造（`class C(complex)`）本层未接线：core 的 `new_complex` 只造基类实例（如实报）",
        ));
    }
    // 参照实测：`complex(z) is z` ⇒ **True**（单参、恰是 `complex`、无关键字 ⇒ 直接返回原对象）。
    if kwargs.is_empty() && args.len() == 2 {
        if instance.type_of(args[1]) == complex_type {
            return Ok(instance.retain(args[1]));
        }
    }
    let (real, imag) = complex_from_arguments(instance, &args[1..], kwargs)?;
    Ok(instance.new_complex(real, imag))
}

/// 把一次 `complex(...)` 调用的实参解析成 `(real, imag)`（口径见上方注释）。
fn complex_from_arguments(
    instance: &Instance,
    positional: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<(f64, f64), ExecError> {
    if kwargs.len() > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("complex() takes at most 2 keyword arguments ({} given)", kwargs.len()),
        ));
    }
    let total = positional.len() + kwargs.len();
    if total > 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("complex() takes at most 2 arguments ({total} given)"),
        ));
    }
    let mut real_keyword: Option<NonNull<Header>> = None;
    let mut imag_keyword: Option<NonNull<Header>> = None;
    for (key, value) in kwargs {
        let Some(name) = instance.text_of(*key) else { continue };
        match name {
            "real" => real_keyword = Some(*value),
            "imag" => imag_keyword = Some(*value),
            other => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("complex() got an unexpected keyword argument '{other}'"),
                ))
            }
        }
    }
    if !positional.is_empty() && real_keyword.is_some() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "argument for complex() given by name ('real') and position (1)",
        ));
    }
    if positional.len() >= 2 && imag_keyword.is_some() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "argument for complex() given by name ('imag') and position (2)",
        ));
    }
    // **单实参形态**：只有 1 个位置实参、没有 imag（也没有 real 关键字）⇒ 允许字符串／协议。
    if positional.len() == 1 && real_keyword.is_none() && imag_keyword.is_none() {
        return complex_single_argument(instance, positional[0]);
    }
    // **两实参形态**（含"只给关键字"与缺省值）。
    let real = match positional.first().copied().or(real_keyword) {
        Some(value) => complex_two_argument_real(instance, value, "real")?,
        None => (0.0, 0.0, false),
    };
    let imag = match positional.get(1).copied().or(imag_keyword) {
        Some(value) => complex_two_argument_imag(instance, value, "imag")?,
        None => (0.0, 0.0, false),
    };
    // 两侧都是**实数** ⇒ 原样成对（**保住 `-0.0` 的符号位**：参照 `complex(1.0, -0.0)`
    // ⇒ `(1-0j)`、`complex(0.0, -0.0)` ⇒ `-0j`）；任一侧是复数 ⇒ 参照的历史口径
    // `real + imag * 1j`（实测 `complex(1+2j, 3)` ⇒ `(1+5j)`、`complex(1, 3j)` ⇒ `(-2+0j)`）。
    if real.2 || imag.2 {
        Ok((real.0 - imag.1, real.1 + imag.0))
    } else {
        Ok((real.0, imag.0))
    }
}

/// 单实参转换（str 解析／complex／int／float／`__complex__`／`__float__`）。
fn complex_single_argument(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<(f64, f64), ExecError> {
    if let Some(text) = instance.text_of(value) {
        let text = text.to_owned();
        return complex_parse_text(instance, &text);
    }
    if let Some((real, imag)) = instance.complex_value(value) {
        return Ok((real, imag));
    }
    if let Some(number) = number_as_f64(instance, value)? {
        return Ok((number, 0.0));
    }
    if let Some((real, imag)) = complex_dunder_value(instance, value)? {
        return Ok((real, imag));
    }
    if let Some(number) = float_dunder_value(instance, value)? {
        return Ok((number, 0.0));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("complex() argument must be a string or a number, not {}", type_name(instance, value)),
    ))
}

/// 两参形态的 `real`：int／float／`__float__`；`complex`（含 `__complex__`）照历史口径收。
fn complex_two_argument_real(
    instance: &Instance,
    value: NonNull<Header>,
    argument: &str,
) -> Result<(f64, f64, bool), ExecError> {
    if let Some((real, imag)) = instance.complex_value(value) {
        return Ok((real, imag, true));
    }
    if let Some(number) = number_as_f64(instance, value)? {
        return Ok((number, 0.0, false));
    }
    if let Some((real, imag)) = complex_dunder_value(instance, value)? {
        return Ok((real, imag, true));
    }
    if let Some(number) = float_dunder_value(instance, value)? {
        return Ok((number, 0.0, false));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!(
            "complex() argument '{argument}' must be a real number, not {}",
            type_name(instance, value)
        ),
    ))
}

/// 两参形态的 `imag`：只认实数协议（int／float／`__float__`／complex 的历史口径）。
fn complex_two_argument_imag(
    instance: &Instance,
    value: NonNull<Header>,
    argument: &str,
) -> Result<(f64, f64, bool), ExecError> {
    if let Some((real, imag)) = instance.complex_value(value) {
        return Ok((real, imag, true));
    }
    if let Some(number) = number_as_f64(instance, value)? {
        return Ok((number, 0.0, false));
    }
    if let Some(number) = float_dunder_value(instance, value)? {
        return Ok((number, 0.0, false));
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!(
            "complex() argument '{argument}' must be a real number, not {}",
            type_name(instance, value)
        ),
    ))
}

/// int／float 折成 `f64`（整数太大 ⇒ 参照的 `OverflowError: int too large to convert to float`）。
fn number_as_f64(instance: &Instance, value: NonNull<Header>) -> Result<Option<f64>, ExecError> {
    if let Some(number) = instance.float_value(value) {
        return Ok(Some(number));
    }
    if let Some(integer) = instance.int_of(value) {
        let number = integer.to_bigint().to_f64();
        if number.is_infinite() {
            return Err(instance.raise_builtin_error(
                "OverflowError",
                "int too large to convert to float",
            ));
        }
        return Ok(Some(number));
    }
    Ok(None)
}

/// 调 `__float__`（没有 ⇒ `None`）；返回非 float ⇒ 参照的 `__float__ returned non-float` 文案。
fn float_dunder_value(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Option<f64>, ExecError> {
    let Some(method) = instance.attribute_optional_of(value, "__float__")? else {
        return Ok(None);
    };
    if method == instance.singletons().none() {
        return Ok(None);
    }
    let bound = type_name(instance, method) == "method";
    let result = if bound {
        pyawa_core::executor::call_value(instance, method, &[], &[])?
    } else {
        pyawa_core::executor::call_value(instance, method, &[value], &[])?
    };
    if let Some(number) = instance.float_value(result) {
        instance.release(result);
        return Ok(Some(number));
    }
    let returned = type_name(instance, result);
    instance.release(result);
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!(
            "{}.__float__ returned non-float (type {returned})",
            type_name(instance, value)
        ),
    ))
}

/// 调 `__complex__`（没有 ⇒ `None`）；返回非 complex ⇒ 参照的 `__complex__ returned non-complex` 文案。
fn complex_dunder_value(
    instance: &Instance,
    value: NonNull<Header>,
) -> Result<Option<(f64, f64)>, ExecError> {
    let Some(method) = instance.attribute_optional_of(value, "__complex__")? else {
        return Ok(None);
    };
    if method == instance.singletons().none() {
        return Ok(None);
    }
    let bound = type_name(instance, method) == "method";
    let result = if bound {
        pyawa_core::executor::call_value(instance, method, &[], &[])?
    } else {
        pyawa_core::executor::call_value(instance, method, &[value], &[])?
    };
    if let Some((real, imag)) = instance.complex_value(result) {
        instance.release(result);
        return Ok(Some((real, imag)));
    }
    let returned = type_name(instance, result);
    instance.release(result);
    Err(instance.raise_builtin_error(
        "TypeError",
        &format!("__complex__ returned non-complex (type {returned})"),
    ))
}

/// `complex("…")` 的**字符串解析**（CPython `complex_subtype_from_string` 的口径，逐条实测）。
///
/// 规则：两端去空白 → 字符白名单（ASCII 字母数字／`.`／`+`／`-`／`(`／`)`／`_`；内部空白 ⇒
/// malformed）→ 只剥**一层** `(`…`)` 再 trim → 尾 `j`/`J` 走"实部＋虚部"拆分（最后一个不在
/// `e`/`E` 之后的 `+`/`-` 当分隔；空虚部 ⇒ 1、`+` ⇒ 1、`-` ⇒ -1）→ 各段按 CPython 的
/// float 字面量口径解析（合法下划线、`inf`／`infinity`／`nan` 不区分大小写）。
fn complex_parse_text(instance: &Instance, text: &str) -> Result<(f64, f64), ExecError> {
    let original = text;
    let mut body = text.trim();
    if body.is_empty() {
        return Err(complex_malformed(instance));
    }
    // 先剥**一层**括号再 trim（参照实测：`"( 1+2j)"`／`"(1+2j )"` 合法；空格只在**两端**
    // 允许——`"( 1 + 2j )"` 中间有空格 ⇒ malformed，由下面的白名单兜住）。
    let bytes = body.as_bytes();
    if bytes.first() == Some(&b'(') && bytes.last() == Some(&b')') {
        body = body[1..body.len() - 1].trim();
        if body.is_empty() {
            return Err(complex_malformed(instance));
        }
    }
    let allowed = |character: char| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '+' | '-' | '(' | ')' | '_')
    };
    if !body.chars().all(allowed) {
        return Err(complex_malformed(instance));
    }
    if body.ends_with('j') || body.ends_with('J') {
        let working = &body[..body.len() - 1];
        let working_bytes = working.as_bytes();
        let mut split = None;
        for index in (1..working_bytes.len()).rev() {
            if matches!(working_bytes[index], b'+' | b'-')
                && !matches!(working_bytes[index - 1], b'e' | b'E')
            {
                split = Some(index);
                break;
            }
        }
        let (real_text, imag_text) = match split {
            Some(index) => (&working[..index], &working[index..]),
            None => ("", working),
        };
        let real = if real_text.is_empty() {
            0.0
        } else {
            complex_component(instance, real_text, original)?
        };
        let imag = match imag_text {
            "" | "+" => 1.0,
            "-" => -1.0,
            other => complex_component(instance, other, original)?,
        };
        Ok((real, imag))
    } else {
        let real = complex_component(instance, body, original)?;
        Ok((real, 0.0))
    }
}

/// 解析一个**数值分量**（CPython 的 float 字面量口径；下划线位置非法 ⇒ 另一种文案）。
fn complex_component(
    instance: &Instance,
    text: &str,
    original: &str,
) -> Result<f64, ExecError> {
    if text.is_empty() {
        return Err(complex_malformed(instance));
    }
    let bytes = text.as_bytes();
    let mut cleaned = String::with_capacity(text.len());
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'_' {
            let before = index > 0 && bytes[index - 1].is_ascii_digit();
            let after = index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit();
            if !(before && after) {
                return Err(instance.raise_builtin_error(
                    "ValueError",
                    &format!("could not convert string to complex: '{original}'"),
                ));
            }
        } else {
            cleaned.push(*byte as char);
        }
    }
    match cleaned.parse::<f64>() {
        Ok(number) => Ok(number),
        Err(_) => Err(complex_malformed(instance)),
    }
}

fn complex_malformed(instance: &Instance) -> ExecError {
    instance.raise_builtin_error("ValueError", "complex() arg is a malformed string")
}

/// `abs(x)`：整数给整数、浮点给浮点；别的 `TypeError`。
///
/// 实测：`abs(True)` ⇒ `1`（**int**，不是 bool）；`abs("x")` ⇒
/// `bad operand type for abs(): 'str'`。
fn abs_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "abs", args, 1)?;
    let value = args[0];
    if let Some(integer) = instance.int_value(value) {
        return Ok(instance.new_int(integer.abs()));
    }
    if let Some(number) = instance.float_value(value) {
        return Ok(instance.new_float(number.abs()));
    }
    // **`abs(complex)` ⇒ 模（float）**（task-31；参照 `abs(1+2j)` ⇒ `2.23606797749979` ✓）。
    if let Some((real, imag)) = instance.complex_value(value) {
        return Ok(instance.new_float(real.hypot(imag)));
    }
    let message = format!("bad operand type for abs(): '{}'", type_name(instance, value));
    Err(instance.raise_builtin_error("TypeError", &message))
}

/// `len(x)`：`str`／`list`／`tuple`／`dict`／`set`；别的 `TypeError`。
///
/// 实测：`len(5)` ⇒ `object of type 'int' has no len()`。
fn len_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "len", args, 1)?;
    let value = args[0];
    match instance.length_with_protocol(value) {
        Ok(Some(length)) => Ok(instance.new_int(length as i64)),
        Err(error) => Err(error),
        Ok(None) => {
            let message = format!(
                "object of type '{}' has no len()",
                type_name(instance, value)
            );
            Err(instance.raise_builtin_error("TypeError", &message))
        }
    }
}

/// `ord(s)`：单字符字符串 → 码点。
///
/// 实测：`ord("ab")` ⇒ `ord() expected a character, but string of length 2 found`。
fn ord_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **实参口径照参照 3.14.4 实测**：关键字先拒、再查个数（`ord() takes exactly one argument`）。
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "ord() takes no keyword arguments"));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("ord() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let value = args[0];
    // **`str` 的码点视图走 WTF-8**（P3-38 S2a-2）：孤立代理各算 1 个码点 ✓ —— 用 `text_of`
    // （合法 UTF-8 视图）会漏掉 WTF-8 表示的串 ✗。
    if let Some(bytes) = instance.str_wtf8_bytes(value) {
        let mut count = 0usize;
        let mut first = 0u32;
        for point in pyawa_core::wtf8::code_points(bytes) {
            let Ok(code_point) = point else {
                // 已校验的 `str` 不该解不出来；真出现按"长度 0"如实报（不猜一个码点）。
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    "ord() expected a character, but string of length 0 found",
                ));
            };
            if count == 0 {
                first = code_point;
            }
            count += 1;
        }
        if count != 1 {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("ord() expected a character, but string of length {count} found"),
            ));
        }
        return Ok(instance.new_int(i64::from(first)));
    }
    // **`str` 子类**（载荷同布局；`wtf8_of` 只认精确类型）：合法 UTF-8 的走 `object_str` 视图
    // （参照对 `class S(str)` 的 `ord(S("a"))` 同样给 97 ✓）。
    if instance
        .type_named("str")
        .is_some_and(|base| instance.is_subtype(instance.type_of(value), base))
    {
        let text = instance.object_str(value)?;
        let count = text.chars().count();
        if count != 1 {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("ord() expected a character, but string of length {count} found"),
            ));
        }
        let code_point = text.chars().next().unwrap_or('\0');
        return Ok(instance.new_int(i64::from(code_point as u32)));
    }
    let message = format!(
        "ord() expected string of length 1, but {} found",
        type_name(instance, value)
    );
    Err(instance.raise_builtin_error("TypeError", &message))
}

fn chr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "chr() takes no keyword arguments"));
    }
    if args.len() != 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("chr() takes exactly one argument ({} given)", args.len()),
        ));
    }
    let value = args[0];
    let Some(code) = instance.int_value(value) else {
        let message = format!(
            "'{}' object cannot be interpreted as an integer",
            type_name(instance, value)
        );
        return Err(instance.raise_builtin_error("TypeError", &message));
    };
    let Some(code_point) = u32::try_from(code).ok().filter(|point| *point <= 0x10FFFF) else {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "chr() arg not in range(0x110000)",
        ));
    };
    // **单码点 WTF-8 构造**（P3-38 S2a-2）：参照允许**孤立代理** 0xD800–0xDFFF
    // （`chr(0xD800)` ⇒ `'\ud800'`、len 1、`ord` 回 55296 ✓）；合法标量的 WTF-8 字节就是
    // UTF-8 ⇒ `new_str_wtf8` 走 UTF-8 表示（**零行为改动** ✓）。
    let mut bytes = Vec::with_capacity(4);
    if pyawa_core::wtf8::encode_code_point(code_point, &mut bytes).is_err() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "chr() arg not in range(0x110000)",
        ));
    }
    instance.new_str_wtf8(&bytes).ok_or_else(|| {
        // 单码点一定合法 ⇒ 不该发生；真发生也如实报范围错误，不造有损串。
        instance.raise_builtin_error("ValueError", "chr() arg not in range(0x110000)")
    })
}

/// `bin`／`oct`／`hex` 的共同实现（`base` 是 2／8／16，前缀照参照实现）。
fn radix_native(
    instance: &Instance,
    name: &str,
    base: u32,
    args: &[NonNull<Header>],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, name, args, 1)?;
    let value = args[0];
    let integer = match instance.int_value(value) {
        Some(integer) => integer,
        None => {
            let message = format!(
                "'{}' object cannot be interpreted as an integer",
                type_name(instance, value)
            );
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    let sign = if integer < 0 { "-" } else { "" };
    let magnitude = integer.unsigned_abs();
    let digits = match base {
        2 => format!("{magnitude:b}"),
        8 => format!("{magnitude:o}"),
        _ => format!("{magnitude:x}"),
    };
    let prefix = match base {
        2 => "0b",
        8 => "0o",
        _ => "0x",
    };
    Ok(instance.new_str(&format!("{sign}{prefix}{digits}")))
}

/// `bin(x)`：`bin(-5)` ⇒ `'-0b101'`；`bin(True)` ⇒ `'0b1'`。
fn bin_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "bin", 2, args)
}

/// `oct(x)`：`oct(8)` ⇒ `'0o10'`。
fn oct_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "oct", 8, args)
}

/// `hex(x)`：`hex(1.5)` ⇒ `'float' object cannot be interpreted as an integer`。
fn hex_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    radix_native(instance, "hex", 16, args)
}

/// `callable(x)`：**有 `call` 槽就返回 `True`**（`OM-11`），否则 `False`。
fn callable_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "callable", args, 1)?;
    let value = args[0];
    Ok(instance.new_bool(instance.is_callable(value)))
}

/// `isinstance(x, T)`：`T` 可以是类型，也可以是类型的元组。
///
/// 实测：`isinstance(1, 5)` ⇒
/// `isinstance() arg 2 must be a type, a tuple of types, or a union`。
fn isinstance_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "isinstance", args, 2)?;
    let subject = args[0];
    let matches = type_matches(instance, subject, args[1], "isinstance")?;
    Ok(instance.new_bool(matches))
}

/// `issubclass(T, U)`：`bool` 是 `int` 的子类，反向不是。
///
/// 实测：`issubclass(1, int)` ⇒ `issubclass() arg 1 must be a class`。
fn issubclass_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "issubclass", args, 2)?;
    let subject = args[0];
    // 注意：`issubclass(T, U)` 的**主题类型是 `T` 自己**（不是 `type_of(T)`，那是 `type`）
    let subject_type = match instance.as_type(subject) {
        Some(ty) => ty,
        None => {
            return Err(instance.raise_builtin_error(
                "TypeError",
                "issubclass() arg 1 must be a class",
            ))
        }
    };
    let matches = type_matches_against(instance, subject, subject_type, args[1], "issubclass")?;
    Ok(instance.new_bool(matches))
}

/// `isinstance` 的判定：`T` 是类型 ⇒ 直接比；是元组 ⇒ 逐个比。
fn type_matches(
    instance: &Instance,
    subject: NonNull<Header>,
    expected: NonNull<Header>,
    caller: &str,
) -> Result<bool, ExecError> {
    let subject_type = instance.type_of(subject);
    type_matches_against(instance, subject, subject_type, expected, caller)
}

/// 判定"某个类型是不是 `expected` 描述的类型（或之一）"。
fn type_matches_against(
    instance: &Instance,
    subject: NonNull<Header>,
    subject_type: NonNull<pyawa_core::TypeObject>,
    expected: NonNull<Header>,
    caller: &str,
) -> Result<bool, ExecError> {
    if let Some(target) = instance.as_type(expected) {
        // **元类钩子优先** ✓（第 722 轮）：参照的 `isinstance`／`issubclass` 走
        // `type(U).__instancecheck__`／`__subclasscheck__` ✓ —— `Lib/_py_abc.py:92/108` 正是 ABC 的
        // **虚拟子类注册**那一套（`Sequence.register(list)` 之后 `isinstance([1], Sequence)` 要为真 ✓）。
        // 普通类没有这两个名字 ⇒ `None` ⇒ 照旧走下面的**结构判定** ✓。
        let hook = if caller == "isinstance" { "__instancecheck__" } else { "__subclasscheck__" };
        if let Some(verdict) = call_type_hook(instance, target.cast::<Header>(), hook, subject)? {
            return Ok(verdict);
        }
        return Ok(instance.is_subtype(subject_type, target));
    }
    if let Some(items) = instance.tuple_items(expected) {
        for item in items {
            if type_matches_against(instance, subject, subject_type, item, caller)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let message = format!("{caller}() arg 2 must be a type, a tuple of types, or a union");
    Err(instance.raise_builtin_error("TypeError", &message))
}

/// 问**元类钩子** ✓（`__instancecheck__`／`__subclasscheck__`）：有就调、`None` ＝ 没有这个钩子 ✓。
///
/// **口径照参照** ✓（task-25 ④ 顺带修 ✗）：CPython 的 `PyObject_IsInstance` 走的是
/// **`type(U).__instancecheck__`** ✓（`_PyObject_LookupSpecial(U, …)` ＝ 在 `type(U)` 上找、
/// 再 `descr.__get__(U, type(U))` 绑到 `U` ✓）—— 先前在 **`U` 自己的 MRO** 上找 ✗
/// ⇒ `isinstance(arg, _SpecialForm)`（`U` 是**类** ✓）把 `_SpecialForm` 自己命名的
/// `__instancecheck__(self, obj)` 取成**未绑定**函数 ✗ ⇒ 调用只带一个实参 ⇒
/// `TypeError: __instancecheck__() missing 1 required positional argument: 'obj'` ✓
/// （`Lib/typing.py:210` 的 `isinstance(arg, _SpecialForm)` 就卡这 ✓）。
///
/// 绑定形态：拿到的若是**函数／原生**（未绑定 ✓）⇒ 调用时把 `U` 作为第一个实参补上 ✓；
/// 拿到**方法**（已绑定 ✓）⇒ 只传 `subject` ✓。
fn call_type_hook(
    instance: &Instance,
    target: NonNull<Header>,
    hook: &str,
    subject: NonNull<Header>,
) -> Result<Option<bool>, ExecError> {
    // **元类**：`type(U)` ✓。
    let metaclass = instance.type_of(target).cast::<Header>();
    let Some(callable) = instance.attribute_optional_of(metaclass, hook)? else {
        return Ok(None);
    };
    let callable_type = instance.type_name(instance.type_of(callable));
    let result = if callable_type == "method" {
        pyawa_core::executor::call::call_value(instance, callable, &[subject], &[])?
    } else {
        // 未绑定：补 `U` 当第一个实参 ✓（借用 ⇒ 调用机制自己 retain ✓）。
        pyawa_core::executor::call::call_value(instance, callable, &[target, subject], &[])?
    };
    let verdict = instance.truth_of(result);
    instance.release(result);
    Ok(Some(verdict))
}

/// `repr(x)`：走 `OM-11` 的 `repr` 槽（`TS-44` 的口径）。
/// `id(object)`（第 581 轮）：对象的**身份**＝它的地址 ✓（参照语义：同一对象恒定 ✓、不同对象不同 ✓）。
///
/// **如实范围** ✗：参照把它当"生命周期内唯一且不复用"的编号 ✓，我们是**直接给地址** ✓ ⇒
/// 对象释放后地址可能被复用 ✗（参照也不保证跨生命周期稳定 ✓，故差别只在"复用后可能与旧值相同" ✗）。
fn id_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "id", args, 1)?;
    Ok(instance.new_int(args[0].as_ptr() as usize as i64))
}

/// `eval(源码, 命名空间)`（**最小实现** ✓，第 616 轮）：只支持**带命名空间**的形态 ✓
///（`eval` 拿不到调用者帧 ✗ —— 我们只接了 `sys._getframe(0)` ✓）；无命名空间时如实报错 ✓，不假装 ✓。
fn eval_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(first) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "eval() missing required argument 'source' (pos 1)",
        ));
    };
    let (globals, locals, locals_owned, globals_owned) = resolve_namespaces(
        instance,
        args.get(1).copied(),
        args.get(2).copied(),
        "eval",
    )?;
    let result = if pyawa_core::is_code_object(instance, first) {
        // **code 对象分支**（task-43）：`eval(compile(src, f, "eval"), globals)` ✓。
        pyawa_core::eval_code_public(instance, first, globals, locals)
    } else if let Some(source) = instance.text_of(first) {
        pyawa_core::eval_source_public(instance, source, globals, locals)
    } else {
        Err(instance.raise_builtin_error(
            "TypeError",
            "eval() arg 1 must be a string, bytes or code object",
        ))
    };
    if locals_owned {
        instance.release(locals);
    }
    if let (Some(globals), true) = (globals, globals_owned) {
        instance.release(globals);
    }
    result
}

/// **`exec(source, globals=None, locals=None)`** ✓（task-20 第 3 轮；task-43 补齐 code 对象与
/// 隐式命名空间）：`Lib/dataclasses.py::_create_fn` 一族靠它把**生成的源码**跑起来 ✓。
fn exec_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "exec", args, 1)?;
    let Some(first) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "exec() missing required argument 'source' (pos 1)",
        ));
    };
    let (globals, locals, locals_owned, globals_owned) = resolve_namespaces(
        instance,
        args.get(1).copied(),
        args.get(2).copied(),
        "exec",
    )?;
    let result = if pyawa_core::is_code_object(instance, first) {
        // **code 对象分支**（task-43）：`exec(compile(src, f, "exec"), globals, locals)` ✓。
        pyawa_core::exec_code_public(instance, first, globals, locals)
    } else if let Some(source) = instance.text_of(first) {
        instance.exec_source(source, globals, locals)
    } else {
        Err(instance.raise_builtin_error(
            "TypeError",
            "exec() arg 1 must be a string, bytes or code object",
        ))
    };
    if locals_owned {
        instance.release(locals);
    }
    if let (Some(globals), true) = (globals, globals_owned) {
        instance.release(globals);
    }
    result
}

/// **`eval`／`exec` 的 (globals, locals) 解析**（task-43）：`None` 单例与"没给"等价 ✓；
/// 只给 `globals` ⇒ `locals` 就是它 ✓；都没给 ⇒ 用**调用者帧**的 globals／`f_locals` 快照 ✓
/// （`f_back` 链接好后这条路才成立 ✓）。返回值里两个 bool 表示"这两份要不要本函数归还" ✓。
fn resolve_namespaces(
    instance: &Instance,
    globals_arg: Option<NonNull<Header>>,
    locals_arg: Option<NonNull<Header>>,
    what: &str,
) -> Result<(Option<NonNull<Header>>, NonNull<Header>, bool, bool), ExecError> {
    let none = instance.singletons().none();
    let explicit_globals = globals_arg.filter(|value| *value != none);
    let explicit_locals = locals_arg.filter(|value| *value != none);
    let check_dict = |value: NonNull<Header>| -> Result<(), ExecError> {
        if instance.dict_entries(value).is_none() {
            return Err(instance.raise_builtin_error(
                "TypeError",
                &format!("{what}: locals 目前只收 dict"),
            ));
        }
        Ok(())
    };
    match (explicit_globals, explicit_locals) {
        (Some(globals), Some(locals)) => {
            check_dict(locals)?;
            Ok((Some(globals), locals, false, false))
        }
        (Some(globals), None) => Ok((Some(globals), globals, false, false)),
        (None, Some(locals)) => {
            check_dict(locals)?;
            let (implicit_globals, implicit_locals) = pyawa_core::implicit_namespaces(instance);
            instance.release(implicit_locals);
            let globals_owned = implicit_globals.is_some();
            Ok((implicit_globals, locals, false, globals_owned))
        }
        (None, None) => {
            let (globals, locals) = pyawa_core::implicit_namespaces(instance);
            let globals_owned = globals.is_some();
            Ok((globals, locals, true, globals_owned))
        }
    }
}

/// **`ascii(obj)`** ✓（task-19 第 3 轮 ✓）：参照口径 ＝ **`repr(obj)` 再把非 ASCII 转义** ✓
/// （实测 `ascii('é')` ⇒ `"'\\xe9'"` ✓、`ascii('中')` ⇒ `"'\\u4e2d'"` ✓、`ascii(1234)` ⇒ `'1234'` ✓）。
/// 不可打印字符那半边已由 `repr` 转好 ✓（`repr('\u200b')` ⇒ `'\\u200b'` ✓，这里只补**可打印非 ASCII** ✓）。
fn ascii_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "ascii", args, 1)?;
    let repr = instance.object_repr(args[0])?;
    let mut out = String::with_capacity(repr.len());
    for character in repr.chars() {
        if character.is_ascii() {
            out.push(character);
            continue;
        }
        let code = character as u32;
        if code <= 0xFF {
            out.push_str(&format!("\\x{code:02x}"));
        } else if code <= 0xFFFF {
            out.push_str(&format!("\\u{code:04x}"));
        } else {
            out.push_str(&format!("\\U{code:08x}"));
        }
    }
    Ok(instance.new_str(&out))
}

fn repr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "repr", args, 1)?;
    // `OM-11` 扩之后 `repr` 槽能表达失败 ⇒ 如实上抛（如 `TS-45` ①的位数上限）
    let text = instance.object_repr(args[0])?;
    Ok(instance.new_str(&text))
}

/// `iter(object)`（第 142 轮）：走执行器**同一处** `iter_value` ✓（`iter(迭代器)` 返回它自己 ✓）。
fn iter_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "iter", args, 1)?;
    instance.iter_object(args[0])
}

/// `pow(base, exp)`（第 152 轮）：**整数面** ✓（`pow(2, 10)` ✓；三参数（模）随后补 ✗）。
fn pow_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "pow", args, 2)?;
    if args.len() > 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "pow expected at most 3 arguments",
        ));
    }
    let base = instance
        .int_of(args[0])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "pow() 目前只接整数（浮点面随后补）"))?;
    let exponent = instance
        .int_of(args[1])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "pow() 目前只接整数（浮点面随后补）"))?;
    let base = base.to_bigint();
    let exponent = exponent.to_bigint();
    // **两参**：任意精度快速幂 ✓（第 406 轮：先前是 `i64` 的 `wrapping_mul` 循环 ✗ ⇒
    // `pow(10, 10**6)` 直接溢出、`pow(10, 2**61)` 的循环**永不结束** ✓）。
    if args.len() == 2 {
        if exponent.signum() < 0 {
            return Err(instance.raise_builtin_error(
                "NotImplementedError",
                "pow() 的负指数（返回浮点）尚未接线",
            ));
        }
        let value = bigint_pow(&base, &exponent);
        return Ok(instance.new_int_value(pyawa_core::bigint::IntValue::from_big(value)));
    }
    // **三参（模幂）** ✓（第 406 轮）：`_pydecimal.py:6400` 的
    // `pow(10, _PyHASH_MODULUS - 2, _PyHASH_MODULUS)` 在导入期就要它 ✓。
    // 口径照参照实测：结果落 `[0, |m|)`（`m > 0`）／`(m, 0]`（`m < 0`）；
    // **负指数**＝先求模逆（不可逆 ⇒ `ValueError`）再快速幂 ✓；`m == 0` ⇒ `ValueError` ✓。
    let modulus = instance
        .int_of(args[2])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "pow() 的模数要整数"))?;
    let modulus = modulus.to_bigint();
    if modulus.is_zero() {
        return Err(instance.raise_builtin_error(
            "ValueError",
            "pow() 3rd argument cannot be 0",
        ));
    }
    let negative_modulus = modulus.signum() < 0;
    let magnitude = modulus.abs();
    let one = BigInt::from_i64(1);
    let mut result = if exponent.signum() < 0 {
        // 负指数：底数先规约到 [0, |m|)，再求模逆 ✓
        let reduced = bigint_mod(&base, &magnitude);
        let Some(inverse) = bigint_mod_inverse(&reduced, &magnitude) else {
            return Err(instance.raise_builtin_error(
                "ValueError",
                "base is not invertible for the given modulus",
            ));
        };
        let positive_exponent = exponent.abs();
        bigint_modpow(&inverse, &positive_exponent, &magnitude)
    } else {
        let sign_negative = base.signum() < 0 && !bigint_is_even(&exponent);
        let mut value = bigint_modpow(&bigint_mod(&base.abs(), &magnitude), &exponent, &magnitude);
        if sign_negative && !value.is_zero() {
            value = magnitude.sub(&value);
        }
        value
    };
    if negative_modulus && !result.is_zero() {
        result = result.sub(&magnitude);
    }
    let _ = one;
    Ok(instance.new_int_value(pyawa_core::bigint::IntValue::from_big(result)))
}

/// 任意精度快速幂（`base >= 0` 的指数；平方-乘）。
fn bigint_pow(base: &BigInt, exponent: &BigInt) -> BigInt {
    let mut result = BigInt::from_i64(1);
    let mut factor = base.clone();
    let mut remaining = exponent.clone();
    while !remaining.is_zero() {
        if !bigint_is_even(&remaining) {
            result = result.mul(&factor);
        }
        factor = factor.mul(&factor);
        remaining = remaining.shr(1);
    }
    result
}

/// 快速模幂：`base` 先规约到 `[0, modulus)`；`modulus > 0`。
fn bigint_modpow(base: &BigInt, exponent: &BigInt, modulus: &BigInt) -> BigInt {
    let one = BigInt::from_i64(1);
    if modulus.cmp(&one) == core::cmp::Ordering::Equal {
        return BigInt::from_i64(0);
    }
    let mut result = BigInt::from_i64(1);
    let mut factor = bigint_mod(base, modulus);
    let mut remaining = exponent.clone();
    while !remaining.is_zero() {
        if !bigint_is_even(&remaining) {
            result = bigint_mod(&result.mul(&factor), modulus);
        }
        factor = bigint_mod(&factor.mul(&factor), modulus);
        remaining = remaining.shr(1);
    }
    result
}

/// `value mod modulus`（`modulus > 0` ⇒ 结果落 `[0, modulus)` ✓）。
fn bigint_mod(value: &BigInt, modulus: &BigInt) -> BigInt {
    match value.divmod_floor(modulus) {
        Some((_, remainder)) => remainder,
        None => BigInt::from_i64(0),
    }
}

fn bigint_is_even(value: &BigInt) -> bool {
    bigint_mod(value, &BigInt::from_i64(2)).is_zero()
}

/// 模逆（`modulus > 0`；扩展欧几里得）。不可逆给 `None`（含 `value == 0`）。
fn bigint_mod_inverse(value: &BigInt, modulus: &BigInt) -> Option<BigInt> {
    let one = BigInt::from_i64(1);
    if modulus.cmp(&one) == core::cmp::Ordering::Equal {
        return Some(BigInt::from_i64(0));
    }
    let mut old_r = bigint_mod(value, modulus);
    let mut r = modulus.clone();
    let mut old_s = BigInt::from_i64(1);
    let mut s = BigInt::from_i64(0);
    while !r.is_zero() {
        let (quotient, remainder) = old_r.divmod_floor(&r)?;
        old_r = r;
        r = remainder;
        let next = old_s.sub(&quotient.mul(&s));
        old_s = s;
        s = next;
    }
    if old_r.cmp(&one) != core::cmp::Ordering::Equal {
        return None;
    }
    Some(bigint_mod(&old_s, modulus))
}

/// `divmod(a, b)`（第 152 轮）：**整数面** ✓ ⇒ `(a // b, a % b)` ✓（整除**向下取整**，与参照一致 ✓）。
fn divmod_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "divmod", args, 2)?;
    // **`divmod(complex, …)` ⇒ 参照文案**（task-31；实测
    // `divmod(1+2j, 2)` ⇒ `unsupported operand type(s) for divmod(): 'complex' and 'int'`）。
    if is_complex_instance(instance, args[0]) || is_complex_instance(instance, args[1]) {
        let left = type_name(instance, args[0]);
        let right = type_name(instance, args[1]);
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("unsupported operand type(s) for divmod(): '{left}' and '{right}'"),
        ));
    }
    let left = instance
        .int_value(args[0])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "divmod() 目前只接整数（浮点面随后补）"))?;
    let right = instance
        .int_value(args[1])
        .ok_or_else(|| instance.raise_builtin_error("TypeError", "divmod() 目前只接整数（浮点面随后补）"))?;
    if right == 0 {
        return Err(instance.raise_builtin_error("ZeroDivisionError", "integer division or modulo by zero"));
    }
    // **向下取整**（Python 口径 ✓）：Rust 的 `/` 是向零取整 ✗ ⇒ 自己算 ✓
    let mut quotient = left / right;
    let mut remainder = left % right;
    if remainder != 0 && (remainder < 0) != (right < 0) {
        quotient -= 1;
        remainder += right;
    }
    let pair = instance.new_tuple(vec![instance.new_int(quotient), instance.new_int(remainder)]);
    Ok(pair)
}

/// `round(number[, ndigits])`（第 152 轮）：**整数面** ✓（`round(7) == 7` ✓；浮点面随后补 ✗）。
/// `round(number[, ndigits])`（第 337 轮：**浮点面接上** ✓）。
///
/// 口径**逐条量过** ✓（3.14 实测）：
/// * **不给 `ndigits`** ⇒ 返回 **`int`** ✓（`round(2.5)` ⇒ `2` ✓、`round(3.5)` ⇒ `4` ✓、
///   `round(-0.5)` ⇒ `0` ✓ —— **半数取偶** ✓）；
/// * **给了 `ndigits`** ⇒ 返回 **`float`** ✓（`round(2.675, 2)` ⇒ `2.67` ✓、`round(1234.5678, -2)`
///   ⇒ `1200.0` ✓、`round(2.5, 0)` ⇒ `2.0` ✓）；`int` 给了 `ndigits` 仍返回 **`int`** ✓（`round(7, 2)` ⇒ `7` ✓）；
/// * `ndigits` 不是整数 ⇒ `TypeError: 'float' object cannot be interpreted as an integer` ✓；
/// * 实参没有 `__round__` ⇒ `TypeError: type str doesn't define __round__ method` ✓。
///
/// **半数取偶**用 [`f64::round_ties_even`] ✓（Rust 1.77 起有 ✓）——与参照一致 ✓（不是 `round()` 的
/// "远离零" ✗）。
fn round_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "round", args, 1)?;
    let digits = match args.get(1) {
        None => None,
        Some(value) => match instance.int_value(*value) {
            Some(digits) => Some(digits),
            None => {
                // 照参照：**`bool` 也算整数** ✓（`int_value` 的口径 ✓）；`float` ⇒ 这条消息 ✓
                let what = instance.type_name(instance.type_of(*value));
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("'{what}' object cannot be interpreted as an integer"),
                ));
            }
        },
    };
    // **`int` 一族**：有没有 `ndigits` 都返回 `int` ✓（本层没有大整数 ⇒ 原样返回 ✓）
    if let Some(number) = instance.int_value(args[0]) {
        return Ok(instance.new_int(number));
    }
    let Some(number) = instance.float_value(args[0]) else {
        let what = instance.type_name(instance.type_of(args[0]));
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("type {what} doesn't define __round__ method"),
        ));
    };
    match digits {
        None => Ok(instance.new_int(number.round_ties_even() as i64)),
        Some(digits) if digits >= 0 => {
            // **照参照：按"正确的十进制舍入"** ✓ —— 不是 `number * 10**n` 缩放 ✗。
            // 实测差异：`round(2.675, 2)` 参照给 `2.67` ✓（`2.675` 的二进制真值是
            // `2.67499999999999982…` ⇒ 十进制第 3 位就**不到半** ✓）；而"先乘 100"会得到
            // **正好 267.5** ✗（本层与参照都是 267.5 ✓ 实测过 ✓）⇒ 半数取偶给 268 ⇒ `2.68` ✗。
            // Rust 的 `{:.n}` 用**精确值**做十进制舍入 ✓ ⇒ 与参照同路 ✓。
            let precision = digits.min(17) as usize;
            let text = format!("{number:.precision$}");
            match text.parse::<f64>() {
                Ok(rounded) => Ok(instance.new_float(rounded)),
                Err(_) => Ok(instance.new_float(number)),
            }
        }
        Some(digits) => {
            // `ndigits < 0`：`format!` 做不了 ⇒ 照参照按 10 的幂缩放 ✓（`round(1234.5678, -2)`
            // ⇒ `1200.0` ✓，这一格缩放与参照一致 ✓ 实测过 ✓）。
            // `digits` 是**负数** ⇒ 步长是 `10^(-digits)` ✓（`round(1234.5678, -2)` ⇒ 步长 100 ✓
            // ⇒ `1200.0` ✓）。先前写成 `10^digits`（＝0.01 ✗）⇒ 除以 0.01 等于乘 100 ✗ ⇒
            // 得到 `1234.57` ✗（实测抓到的 ✓）。
            let scale = 10f64.powi((-digits).clamp(0, 308) as i32);
            if scale == 0.0 || !scale.is_finite() {
                return Ok(instance.new_float(number));
            }
            Ok(instance.new_float((number / scale).round_ties_even() * scale))
        }
    }
}

/// `next(iterator[, default])`（第 142 轮）：走执行器**同一处** `advance` ✓（内建迭代器 ＋
/// `__next__` 协议 ✓）；耗尽时有 `default` 给 `default` ✓，没有就抛 `StopIteration` ✓。
fn next_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "next", args, 1)?;
    match instance.advance_iterator(args[0])? {
        Some(item) => Ok(item),
        None => match args.get(1) {
            Some(default) => Ok(*default),
            None => Err(instance.raise_builtin_error("StopIteration", "")),
        },
    }
}

/// `list([iterable])`（第 130 轮）：空表或把可迭代项收进来 ✓。
fn list_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "list", args, 0)?;
    let items = match args.first() {
        Some(iterable) => instance
            .iterable_items(*iterable)
            .ok_or_else(|| instance.raise_builtin_error("TypeError", "object is not iterable"))?,
        None => Vec::new(),
    };
    Ok(instance.new_list(items))
}

/// `tuple([iterable])`。
fn tuple_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "tuple", args, 0)?;
    let items = match args.first() {
        Some(iterable) => instance
            .iterable_items(*iterable)
            .ok_or_else(|| instance.raise_builtin_error("TypeError", "object is not iterable"))?,
        None => Vec::new(),
    };
    Ok(instance.new_tuple(items))
}

/// `set([iterable])`。
fn set_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "set", args, 0)?;
    let items = match args.first() {
        Some(iterable) => instance
            .iterable_items(*iterable)
            .ok_or_else(|| instance.raise_builtin_error("TypeError", "object is not iterable"))?,
        None => Vec::new(),
    };
    Ok(instance.new_set(items))
}

/// `dict()`（**最小面**：只接无参 ✓；从映射／键值对建表随后补 ✗）。
fn dict_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "dict() 目前只接无参（从映射／键值对建表随后补）",
        ));
    }
    Ok(instance.new_dict())
}

/// `str([object])`：字符串原样返回 ✓，其余走 `repr`（最小面 ✓）。
fn str_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "str", args, 0)?;
    let Some(value) = args.first() else {
        return Ok(instance.new_str(""));
    };
    if instance.type_of(*value) == instance.singletons().str_type() {
        return Ok(instance.retain(*value));
    }
    let text = instance.object_repr(*value)?;
    Ok(instance.new_str(&text))
}

/// `int([value])`：整数原样 ✓、字符串按十进制解析 ✓（最小面 ✓）。
fn int_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "int", args, 0)?;
    let Some(value) = args.first() else {
        return Ok(instance.new_int(0));
    };
    if let Some(number) = instance.int_value(*value) {
        return Ok(instance.new_int(number));
    }
    if let Some(text) = instance.text_of(*value) {
        return match text.trim().parse::<i64>() {
            Ok(number) => Ok(instance.new_int(number)),
            Err(_) => Err(instance.raise_builtin_error(
                "ValueError",
                &format!("invalid literal for int(): {text}"),
            )),
        };
    }
    if let Some(number) = instance.float_value(*value) {
        return Ok(instance.new_int(number as i64));
    }
    // **`int(complex)` ⇒ 参照文案**（task-31；实测 `int(1+2j)` ⇒
    // `int() argument must be a string, a bytes-like object or a real number, not 'complex'` ✓）。
    if is_complex_instance(instance, *value) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "int() argument must be a string, a bytes-like object or a real number, not 'complex'",
        ));
    }
    Err(instance.raise_builtin_error("TypeError", "int() 只接整数／字符串／浮点（最小面）"))
}

/// `float([value])`：浮点原样 ✓、整数与字符串转换 ✓（最小面 ✓）。
fn float_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "float", args, 0)?;
    let Some(value) = args.first() else {
        return Ok(instance.new_float(0.0));
    };
    if let Some(number) = instance.float_value(*value) {
        return Ok(instance.new_float(number));
    }
    if let Some(number) = instance.int_value(*value) {
        return Ok(instance.new_float(number as f64));
    }
    if let Some(text) = instance.text_of(*value) {
        return match text.trim().parse::<f64>() {
            Ok(number) => Ok(instance.new_float(number)),
            Err(_) => Err(instance.raise_builtin_error(
                "ValueError",
                &format!("could not convert string to float: {text}"),
            )),
        };
    }
    // **`float(complex)` ⇒ 参照文案**（task-31；实测 ⇒
    // `float() argument must be a string or a real number, not 'complex'` ✓）。
    if is_complex_instance(instance, *value) {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "float() argument must be a string or a real number, not 'complex'",
        ));
    }
    Err(instance.raise_builtin_error("TypeError", "float() 只接浮点／整数／字符串（最小面）"))
}

/// `bool([value])`：真假表 ✓（`bool_value` 就是引擎的口径 ✓）。
fn bool_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "bool", args, 0)?;
    let value = match args.first() {
        // **走执行器的通用真假判定**（第 131 轮）：`bool_value` 只认 bool／None ✗
        Some(value) => instance.truthiness_of(*value)?,
        None => false,
    };
    Ok(instance.retain(instance.singletons().boolean(value)))
}

/// `type(object)`：返回它的类型对象 ✓。
fn type_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "type", args, 1)?;
    // **返回既有对象必须 `retain`**（第 131 轮：不 retain ⇒ 调用方释放后 double free ✗）
    Ok(instance.retain(instance.type_of(args[0]).cast()))
}

/// `getattr(object, name[, default])` ✓（对象字典查名 ✓）。
fn getattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    // **参照的实参口径**（3.14.4 实测）：关键字**先**拒（`getattr(name="x")` 也报"无关键字实参"），
    // 再查位置实参个数（2..=3）；文案照实测。
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "getattr() takes no keyword arguments"));
    }
    if args.len() < 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("getattr expected at least 2 arguments, got {}", args.len()),
        ));
    }
    if args.len() > 3 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("getattr expected at most 3 arguments, got {}", args.len()),
        ));
    }
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    // **走真正的属性通道** ✗（不是"把对象当字典查" ✗ —— 那对非字典是 UB ✓）
    // 这一段内部已经把 AttributeError 吞成 `None` ✓（core `attribute_optional` 的口径）。
    match instance.attribute_optional_of(args[0], name)? {
        Some(found) => return Ok(found),
        None => {}
    }
    // **协议口径** ✓（第 708 轮）：core 里那个公开入口负责 `__getattr__` 回退 ✓。
    // **三参形态要吞掉 `__getattr__` 抛的 AttributeError** ✓（task-34）：CPython 的
    // `getattr(obj, name, default)` 对属性查找链上的一切 AttributeError 都给 default；
    // 两参形态仍原样上抛（原先 `?` 让三参也冒出去 ⇒ `MagicMock()` 在 `NonCallableMock.__init__`
    // 前读 `_mock_methods` 就炸 ✓）。非 AttributeError（用户代码自己抛的）一律不吞 ✓。
    match instance.get_attribute_with_protocol(args[0], name) {
        Ok(Some(found)) => return Ok(found),
        Ok(None) => {}
        Err(error) => {
            let has_default = args.len() == 3;
            if !has_default || !swallow_attribute_error(instance, &error) {
                return Err(error);
            }
        }
    }
    if let Some(default) = args.get(2) {
        return Ok(instance.retain(*default));
    }
    // 参照的"属性缺失"文案带**接收者的类型名**（实测 `getattr(object(), "nope")` ⇒
    // `'object' object has no attribute 'nope'`）；旧文案少了类型名。
    let holder = type_name(instance, args[0]);
    Err(instance.raise_builtin_error(
        "AttributeError",
        &format!("'{holder}' object has no attribute '{name}'"),
    ))
}

/// 把 `ExecError::Raised` 里的 **AttributeError 完整吞掉**（参照的"探测"口径）。
///
/// 返回 `true` ＝ 确实是 AttributeError 且已吞；`false` ＝ 不是（调用方原样上抛）。
/// **两份所有权都要还**：`executor::runtime::raise` 给"实例状态"与 `Err(Raised)` 各一份引用 ⇒
/// 先 `set_pending_exception(None)` 取回状态那份并归还，再归还 `Err` 里的那份
/// （同 `executor/import.rs` 的 `ignore_missing_submodule` 先例 ✓）。
fn swallow_attribute_error(instance: &Instance, error: &ExecError) -> bool {
    let ExecError::Raised { exception } = error else {
        return false;
    };
    let Some(attribute_error) = instance.type_named("AttributeError") else {
        return false;
    };
    // SAFETY: `Raised` 里的异常对象由实例的 `pending_exception` 保活（存活）。
    if !instance.is_subtype(instance.type_of(*exception), attribute_error) {
        return false;
    }
    if let Some(previous) = instance.set_pending_exception(None) {
        instance.release(previous);
    }
    instance.release(*exception);
    true
}

/// `hasattr(object, name)` ✓。
///
/// 参照口径（3.14.4 实测）：`hasattr` 吞 AttributeError（含 `__getattr__` 抛的）⇒ False；
/// **非 AttributeError 原样上抛**（`ValueError: boom` 之类必须继续冒 ✓）。
fn hasattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    if !kwargs.is_empty() {
        return Err(instance.raise_builtin_error("TypeError", "hasattr() takes no keyword arguments"));
    }
    if args.len() != 2 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("hasattr expected 2 arguments, got {}", args.len()),
        ));
    }
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    let mut present = false;
    // **归还新引用** ✓（`attribute_optional_of` 给的是新引用；旧代码 `.is_some()` 直接丢了 ⇒ 泄漏）。
    match instance.attribute_optional_of(args[0], name)? {
        Some(value) => {
            instance.release(value);
            present = true;
        }
        None => {}
    }
    if !present {
        // 本调用顺序下来到这里时，`attribute_lookup` 必然已失败 ⇒ `Ok(Some)` 只可能来自
        // `__getattr__` 分支（`call_callable` 的新引用）⇒ 也要归还 ✓。
        match instance.get_attribute_with_protocol(args[0], name) {
            Ok(Some(value)) => {
                instance.release(value);
                present = true;
            }
            Ok(None) => {}
            Err(error) => {
                if !swallow_attribute_error(instance, &error) {
                    return Err(error);
                }
            }
        }
    }
    Ok(instance.retain(instance.singletons().boolean(present)))
}

/// **`vars(object)`** ✓（task-25 ① ✓）：就是 `object.__dict__` ✓（实例 ⇒ 那个**可变**字典 ✓ ——
/// `vars(self).update(name=…)` 要它 ✓；类／模块 ⇒ 命名空间映射 ✓）。
/// 没有 `__dict__` ⇒ 照参照报 `TypeError: vars() argument must have __dict__ attribute` ✓。
///
/// **如实范围** ✗：**零参**形态（＝ `locals()`）尚未接线 ✓ —— 本层的函数帧没有"局部名字 → 值"的
/// 映射（只有槽位／cell ✓）；报能定位的 `TypeError` ✓，不静默给错东西 ✓。
fn vars_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(object) = args.first().copied() else {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "vars() 的零参形态（＝ locals()）尚未接线",
        ));
    };
    if args.len() > 1 {
        return Err(instance.raise_builtin_error(
            "TypeError",
            &format!("vars() takes at most 1 argument ({} given)", args.len()),
        ));
    }
    // **与 `getattr` 同一条属性通道** ✓（`__getattr__` 回退也照走 ✓）。
    if let Some(found) = instance.attribute_optional_of(object, "__dict__")? {
        return Ok(found);
    }
    if let Some(found) = instance.get_attribute_with_protocol(object, "__dict__")? {
        return Ok(found);
    }
    Err(instance.raise_builtin_error(
        "TypeError",
        "vars() argument must have __dict__ attribute",
    ))
}

/// `globals()`（第 156 轮）：**正在执行的那一帧的全局映射** ✓（`Frame` 的 `globals` 那格 ✓ ——
/// `BC-57` 已保证函数帧取的是函数的 `__globals__` ✓ ⇒ 模块级与函数里都对 ✓）。
/// 没有正在执行的帧（不该发生 ✓）⇒ 如实报错 ✗，不用空字典冒充 ✓。
fn globals_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    _args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    match instance.current_globals() {
        Some(globals) => Ok(instance.retain(globals)),
        None => Err(instance.raise_builtin_error(
            "NotImplementedError",
            "globals() 需要正在执行的帧（执行器还没挂上当前帧）",
        )),
    }
}

/// `setattr(object, name, value)`（第 148 轮）：走 `STORE_ATTR` 同一条路 ✓（**一处真相** ✓）。
/// `delattr(obj, name)` —— 删实例属性（与 `DELETE_ATTR` 用同一条通路 ✓）。
fn delattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "delattr", args, 2)?;
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    pyawa_core::instance_attribute_delete(instance, args[0], name)?;
    Ok(instance.retain(instance.singletons().none()))
}

fn setattr_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    _kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    need_args(instance, "setattr", args, 3)?;
    let Some(name) = instance.text_of(args[1]) else {
        return Err(instance.raise_builtin_error("TypeError", "attribute name must be string"));
    };
    // **协议口径** ✓（第 706 轮）：core 里那个公开入口负责 `__setattr__` ✓（见它的注释 ✓）。
    // **引用纪律**（task-18 真 bug ✗）：原生函数的实参是 `call_callable` 的**借用视图** ✗
    // —— 而该入口要的是**新引用**（与 `STORE_ATTR` 从值栈弹出的那一份同契约 ✓）⇒ 先自己
    // `retain` 一份再交出 ✓。先前漏了这一份 ✗ ⇒ 属性字典持有一份不存在的引用 ✓；实参是
    // **运行时新建的 dict**（无别处持有 ✓）时当场被释放 ✓ ⇒ 读回该属性 SIGSEGV／
    // misaligned pointer（官方 `test_decorators` 的 `noteargs` 装饰器把 `(*args, **kwds)`
    // 存进函数属性时正撞它 ✓）。普通 tuple 常被 code object 的常量表另持一份 ⇒ 症状被遮住 ✓。
    instance.retain(args[2]);
    instance.set_attribute_with_protocol(args[0], name, args[2])?;
    Ok(instance.retain(instance.singletons().none()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_lists_what_it_implements() {
        let instance = Instance::new();
        let namespace = build(&instance);
        for name in IMPLEMENTED {
            assert!(
                instance.dict_get(namespace, name).is_some(),
                "{name} 应当在 builtins 命名空间里"
            );
        }
        // `__build_class__` 来自核心（OM-14）
        assert!(instance.dict_get(namespace, "__build_class__").is_some());
        // 需要输出通道的 `print` 不在这里（口径待裁，见 §5.2.2）
        assert!(instance.dict_get(namespace, "print").is_some());
    }
}

// ---- `min`／`max`／`sorted`（`§9.4` 第 4 条的纯计算面；`CM-4` 的合约见 `SPEC-c-modules.md` §5.2.2）----

/// 取一个关键字实参（`None` 与"没给"不同 ⇒ 返回 `Option<Option<...>>`）。
fn keyword(
    instance: &Instance,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    name: &str,
) -> Result<Option<NonNull<Header>>, ExecError> {
    for (key, value) in kwargs {
        if instance.text_value(*key).as_deref() == Some(name) {
            return Ok(Some(*value));
        }
    }
    Ok(None)
}

/// 关键字里有没有这个名字（不需要值）。
fn has_keyword(kwargs: &[(NonNull<Header>, NonNull<Header>)], instance: &Instance, name: &str) -> bool {
    kwargs
        .iter()
        .any(|(key, _)| instance.text_value(*key).as_deref() == Some(name))
}

/// 拒绝不认识的关键字（实测消息：`min() got an unexpected keyword argument 'reverse'`）。
fn reject_unknown_keywords(
    instance: &Instance,
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    allowed: &[&str],
    function: &str,
) -> Result<(), ExecError> {
    for (key, _) in kwargs {
        let name = instance.text_value(*key).unwrap_or_default();
        if !allowed.contains(&name.as_str()) {
            let message = format!("{function}() got an unexpected keyword argument '{name}'");
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    }
    Ok(())
}

/// `min`／`max` 的取值列表：**单实参** ⇒ 迭代它（实测 `'int' object is not iterable`）；
/// **多实参** ⇒ 实参本身就是候选。
fn candidate_items(
    instance: &Instance,
    args: &[NonNull<Header>],
    function: &str,
) -> Result<Vec<NonNull<Header>>, ExecError> {
    if args.is_empty() {
        let message = format!("{function} expected at least 1 argument, got 0");
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    if args.len() == 1 {
        return match instance.iterable_items(args[0]) {
            Some(items) => Ok(items),
            None => {
                let message = format!("'{}' object is not iterable", type_name(instance, args[0]));
                Err(instance.raise_builtin_error("TypeError", &message))
            }
        };
    }
    Ok(args.iter().map(|item| instance.retain(*item)).collect())
}

/// 求 `key(值)`；没给 `key` 时就是值本身（都返回**新引用**，由调用方归还）。
fn decorated_key(
    instance: &Instance,
    value: NonNull<Header>,
    key: Option<NonNull<Header>>,
) -> Result<NonNull<Header>, ExecError> {
    match key {
        // `call_value` **接手**实参表 ⇒ 先为它新增一份（`retain`），调用方那份仍归自己
        Some(callable) => pyawa_core::call_value(instance, callable, &[instance.retain(value)], &[]),
        None => Ok(instance.retain(value)),
    }
}

/// `min`／`max` 的公共实现：`want` 是"这个序才算更优"。
fn extremum(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
    want: core::cmp::Ordering,
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &["key", "default"], function)?;
    let key = keyword(instance, kwargs, "key")?;
    // `key=None` 与"没给"同义（实测 `sorted([1], key=None)` 正常）
    let key = match key {
        Some(value) if instance.type_of(value) == instance.singletons().none_type() => None,
        other => other,
    };
    let default_given = has_keyword(kwargs, instance, "default");
    let default = keyword(instance, kwargs, "default")?;
    let items = candidate_items(instance, args, function)?;
    if items.is_empty() {
        if default_given {
            for item in &items {
                instance.release(*item);
            }
            return Ok(instance.retain(default.expect("上面确认过给了")));
        }
        let message = format!("{function}() iterable argument is empty");
        return Err(instance.raise_builtin_error("ValueError", &message));
    }
    let mut best = items[0];
    let mut best_key = decorated_key(instance, best, key)?;
    for item in items.iter().skip(1) {
        let item_key = decorated_key(instance, *item, key)?;
        match instance.order_of(item_key, best_key) {
            Some(ordering) if ordering == want => {
                instance.release(best_key);
                best = *item;
                best_key = item_key;
            }
            Some(_) => instance.release(item_key),
            None => {
                // 实测消息把**正在比的那个**放前面：`min(1, 'a')` ⇒ `'str' and 'int'`
                let message = format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    type_name(instance, item_key),
                    type_name(instance, best_key)
                );
                instance.release(item_key);
                instance.release(best_key);
                for item in &items {
                    instance.release(*item);
                }
                return Err(instance.raise_builtin_error("TypeError", &message));
            }
        }
    }
    let result = instance.retain(best);
    instance.release(best_key);
    for item in &items {
        instance.release(*item);
    }
    Ok(result)
}

/// `min(*args, key=None, default=…)`。
fn min_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    extremum(instance, args, kwargs, "min", core::cmp::Ordering::Less)
}

/// `max(*args, key=None, default=…)`。
fn max_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    extremum(instance, args, kwargs, "max", core::cmp::Ordering::Greater)
}

/// `sorted(iterable, /, *, key=None, reverse=False)`：稳定排序，结果是一个**新列表**。
fn sorted_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &["key", "reverse"], "sort")?;
    if args.len() != 1 {
        let message = format!("sorted expected 1 argument, got {}", args.len());
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    let key = keyword(instance, kwargs, "key")?;
    let key = match key {
        Some(value) if instance.type_of(value) == instance.singletons().none_type() => None,
        other => other,
    };
    let reverse = match keyword(instance, kwargs, "reverse")? {
        Some(value) => instance.bool_value(value).unwrap_or(false),
        None => false,
    };
    let items = match instance.iterable_items(args[0]) {
        Some(items) => items,
        None => {
            let message = format!("'{}' object is not iterable", type_name(instance, args[0]));
            return Err(instance.raise_builtin_error("TypeError", &message));
        }
    };
    // 装饰：把每个元素与它的 key 配对（key 只算**一次**，与参照实现一致）
    let mut decorated: Vec<(NonNull<Header>, NonNull<Header>)> = Vec::with_capacity(items.len());
    for item in &items {
        let item_key = decorated_key(instance, *item, key)?;
        decorated.push((*item, item_key));
    }
    let mut failure: Option<String> = None;
    decorated.sort_by(|left, right| match instance.order_of(left.1, right.1) {
        Some(ordering) => ordering,
        None => {
            failure.get_or_insert_with(|| {
                format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    type_name(instance, left.1),
                    type_name(instance, right.1)
                )
            });
            core::cmp::Ordering::Equal
        }
    });
    if let Some(message) = failure {
        for (_, item_key) in &decorated {
            instance.release(*item_key);
        }
        for item in &items {
            instance.release(*item);
        }
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    // `reverse=True`：参照实现是**排完再反转**（等值元素保持原序）
    if reverse {
        decorated.reverse();
    }
    let result = instance.new_list(
        decorated.iter().map(|(item, _)| instance.retain(*item)).collect(),
    );
    for (_, item_key) in &decorated {
        instance.release(*item_key);
    }
    for item in &items {
        instance.release(*item);
    }
    Ok(result)
}

// ---- `sum`／`all`／`any`（上一条把迭代入口铺好之后就能做；合约见 `SPEC-c-modules.md` §5.2.2）----

/// 取一个可迭代对象摊成一批**新引用**；不是可迭代的就报实测那条 `TypeError`。
fn items_of(instance: &Instance, object: NonNull<Header>) -> Result<Vec<NonNull<Header>>, ExecError> {
    match instance.iterable_items(object) {
        Some(items) => Ok(items),
        None => {
            let message = format!("'{}' object is not iterable", type_name(instance, object));
            Err(instance.raise_builtin_error("TypeError", &message))
        }
    }
}

/// `sum(iterable, /, start=0)`：从 `start` 起累加（数值塔内），比不了就报参照实现那条。
fn sum_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &[], "sum")?;
    if args.is_empty() {
        return Err(instance.raise_builtin_error(
            "TypeError",
            "sum() takes at least 1 positional argument (0 given)",
        ));
    }
    let mut total = match args.get(1) {
        Some(start) => instance.retain(*start),
        None => instance.new_int(0),
    };
    for item in items_of(instance, args[0])? {
        let next = instance.add_values(total, item);
        instance.release(item);
        match next {
            Some(value) => {
                instance.release(total);
                total = value;
            }
            None => {
                let message = format!(
                    "unsupported operand type(s) for +: '{}' and '{}'",
                    type_name(instance, total),
                    type_name(instance, item)
                );
                instance.release(total);
                return Err(instance.raise_builtin_error("TypeError", &message));
            }
        }
    }
    Ok(total)
}

/// `all(iterable)`：空 ⇒ `True`。
fn all_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    truth_reducer(instance, args, kwargs, "all", true)
}

/// `any(iterable)`：空 ⇒ `False`。
fn any_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    truth_reducer(instance, args, kwargs, "any", false)
}

/// `all`／`any` 的公共实现：`empty` 是空可迭代时的结果。
fn truth_reducer(
    instance: &Instance,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
    function: &str,
    empty: bool,
) -> Result<NonNull<Header>, ExecError> {
    reject_unknown_keywords(instance, kwargs, &[], function)?;
    if args.len() != 1 {
        // 实测：`all() takes exactly one argument (0 given)`
        let message = format!("{function}() takes exactly one argument ({} given)", args.len());
        return Err(instance.raise_builtin_error("TypeError", &message));
    }
    let mut result = empty;
    for item in items_of(instance, args[0])? {
        let truth = instance.truth_of(item);
        instance.release(item);
        // `any` 见到真就定；`all` 见到假就定
        if truth != empty {
            result = truth;
            break;
        }
    }
    Ok(instance.new_bool(result))
}

/// `__import__(name, globals=None, locals=None, fromlist=(), level=0)` ✓（第 726 轮）：
/// `Lib/encodings/__init__.py:100` 的 `__import__('encodings.' + modname, fromlist=…)` 要它 ✗
/// （缺它 ⇒ `codecs.lookup('cp437')` 当场 `NameError: name '__import__' is not defined` ✓）。
///
/// **如实口径** ✗：只接**绝对**名字（`level != 0` 与相对导入未接 ✓，如实报错 ✓）；
/// **返回顶层模块** ✓（照参照：`fromlist` 为空 ⇒ 顶层 ✓；非空 ⇒ 叶子 ✓ —— 与 `IMPORT_NAME` 同口径 ✓）。
fn import_native(
    instance: &Instance,
    _bound: Option<NonNull<Header>>,
    args: &[NonNull<Header>],
    kwargs: &[(NonNull<Header>, NonNull<Header>)],
) -> Result<NonNull<Header>, ExecError> {
    let Some(name) = args.first().and_then(|value| instance.text_of(*value)).map(|text| text.to_owned())
    else {
        return Err(instance.raise_builtin_error("TypeError", "__import__() 的第一个实参要是 str"));
    };
    // **kwargs 面**（task-6 修 ✗）：先前的实现**整个忽略关键字** ⇒
    // `__import__(name, fromlist=['*'], level=0)`（`Lib/encodings/__init__.py:100` 的写法 ✓）
    // 的 `fromlist` 被当成空 ⇒ 返回顶层包 ⇒ `encodings.search_function` 取 `mod.getregentry`
    // 拿到包 ⇒ `AttributeError` ⇒ `codecs.lookup('cp932')` 一族**全部**查不到 ✗（同一条因也挡
    // 住 charmap 族）。这里照签名把 `fromlist`／`level` 两格从关键字补上；`globals`／`locals`
    // 按参照忽略 ✓（本层不用它们），其余名字照参照报 `TypeError` ✓。
    let mut fromlist_value = args.get(3).copied();
    let mut level_value = args.get(4).copied();
    for (key, value) in kwargs {
        let Some(keyword) = instance.text_of(*key) else {
            return Err(instance.raise_builtin_error("TypeError", "keywords must be strings"));
        };
        match keyword {
            "globals" | "locals" => {}
            "fromlist" => {
                if fromlist_value.is_none() {
                    fromlist_value = Some(*value);
                }
            }
            "level" => {
                if level_value.is_none() {
                    level_value = Some(*value);
                }
            }
            other => {
                return Err(instance.raise_builtin_error(
                    "TypeError",
                    &format!("__import__() got an unexpected keyword argument '{other}'"),
                ))
            }
        }
    }
    let level = level_value
        .and_then(|value| instance.int_of(value))
        .and_then(|value| value.to_i64())
        .unwrap_or(0);
    if level != 0 {
        return Err(ExecError::Unsupported {
            opcode: 0,
            what: "__import__：相对导入（level != 0）尚未接线",
        });
    }
    let fromlist_empty = match fromlist_value {
        None => true,
        Some(value) => match instance.tuple_items(value) {
            Some(items) => items.is_empty(),
            None => !instance.truth_of(value),
        },
    };
    let top = name.split('.').next().unwrap_or(&name).to_owned();
    let leaf = pyawa_core::executor::import::import_through_bridge(instance, &name)?;
    let Some(leaf) = leaf else {
        return Err(instance.raise_builtin_error(
            "ModuleNotFoundError",
            &format!("No module named '{name}'"),
        ));
    };
    if fromlist_empty && top != name {
        if let Some(top_module) = pyawa_core::executor::import::import_through_bridge(instance, &top)? {
            return Ok(instance.retain(top_module));
        }
    }
    Ok(instance.retain(leaf))
}
