//! **协议清单（唯一真相）**：`PLAN`「对象语义唯一入口＝属性查找」不变量（§12.12 机检并入 §12.13）。
//!
//! 每个 VM **支持**的协议在此登记：协议名、**该查的 dunder 名**、oracle 用例名。
//! - **核心派发** 必须问到这里登记的 dunder（宿主对象经 `getattr_fn`、普通对象经属性通道）；
//! - **ABI oracle**（`crates/pyawa-abi/tests/protocol_oracle.rs`）逐条跑用例、断言对应 dunder
//!   出现在记录式 `getattr_fn` 里 ⇒ 任何"只查类型槽/MRO 预判"的旁路都会被抓红；
//! - 不在表里的名字 = **VM 当前不支持该协议**（不是"已实现但没列"）——新增协议必须同时加用例，
//!   否则 oracle 的覆盖检查会红（禁止"只覆盖已实现协议"让机检变绿）。
//!
//! **属性钩子**（`__getattr__`／`__getattribute__`／`__setattr__`／`__delattr__`／`__dir__`）
//! 不在此表：它们**就是**属性查找本身（宿主 `getattr_fn` 是这条通道的入口），不能再用
//! "查 dunder"递归定义；由 `AB-73`／`AB-75` 的通道测试覆盖。
//! **描述符**（`__get__`／`__set__`／`__delete__`）是类属性机制、不是实例协议，暂不在宿主 oracle 面。

/// 一条协议用例：协议名 → 规范 dunder → oracle 用例键（ABI 测试按它给可执行片段）。
#[derive(Clone, Copy)]
pub struct ProtocolCase {
    /// 人类可读协议名（文档／报告用）。
    pub protocol: &'static str,
    /// VM 协议派发**必须询问**的 dunder 名（规范名；回退/别名写在注释里）。
    pub dunder: &'static str,
    /// oracle 用例键（`crates/pyawa-abi/tests/protocol_oracle.rs` 的片段表键）。
    pub oracle: &'static str,
}

/// **VM 支持的协议清单**（2026-11-14 起；新增/删除必须同步 oracle 片段表）。
pub const PROTOCOL_CASES: &[ProtocolCase] = &[
    // ---- 文本与格式化（§12.13 #1 首个必修项）----
    ProtocolCase { protocol: "str", dunder: "__str__", oracle: "str" },
    ProtocolCase { protocol: "repr", dunder: "__repr__", oracle: "repr" },
    ProtocolCase { protocol: "format", dunder: "__format__", oracle: "format" },
    ProtocolCase { protocol: "format-fstring", dunder: "__format__", oracle: "format_fstring" },
    // ---- 真值与哈希 ----
    ProtocolCase { protocol: "bool", dunder: "__bool__", oracle: "bool" },
    ProtocolCase { protocol: "hash", dunder: "__hash__", oracle: "hash" },
    // ---- 富比较（`!=` 规范 dunder 是 `__ne__`；`object.__ne__` 会委托 `__eq__`）----
    ProtocolCase { protocol: "eq", dunder: "__eq__", oracle: "eq" },
    ProtocolCase { protocol: "ne", dunder: "__ne__", oracle: "ne" },
    ProtocolCase { protocol: "lt", dunder: "__lt__", oracle: "lt" },
    ProtocolCase { protocol: "le", dunder: "__le__", oracle: "le" },
    ProtocolCase { protocol: "gt", dunder: "__gt__", oracle: "gt" },
    ProtocolCase { protocol: "ge", dunder: "__ge__", oracle: "ge" },
    // ---- 容器/迭代 ----
    ProtocolCase { protocol: "len", dunder: "__len__", oracle: "len" },
    ProtocolCase { protocol: "contains", dunder: "__contains__", oracle: "contains" },
    ProtocolCase { protocol: "getitem", dunder: "__getitem__", oracle: "getitem" },
    ProtocolCase { protocol: "setitem", dunder: "__setitem__", oracle: "setitem" },
    ProtocolCase { protocol: "delitem", dunder: "__delitem__", oracle: "delitem" },
    ProtocolCase { protocol: "iter", dunder: "__iter__", oracle: "iter" },
    ProtocolCase { protocol: "next", dunder: "__next__", oracle: "next" },
    ProtocolCase { protocol: "reversed", dunder: "__reversed__", oracle: "reversed" },
    // ---- 调用/上下文 ----
    ProtocolCase { protocol: "call", dunder: "__call__", oracle: "call" },
    ProtocolCase { protocol: "enter", dunder: "__enter__", oracle: "enter" },
    ProtocolCase { protocol: "exit", dunder: "__exit__", oracle: "exit" },
    // ---- 异步协议（await／async for／async with；`getattr_fn` 同样应可提供）----
    ProtocolCase { protocol: "await", dunder: "__await__", oracle: "await" },
    ProtocolCase { protocol: "aiter", dunder: "__aiter__", oracle: "aiter" },
    ProtocolCase { protocol: "anext", dunder: "__anext__", oracle: "anext" },
    ProtocolCase { protocol: "aenter", dunder: "__aenter__", oracle: "aenter" },
    ProtocolCase { protocol: "aexit", dunder: "__aexit__", oracle: "aexit" },
    // ---- 算术（正向）----
    ProtocolCase { protocol: "add", dunder: "__add__", oracle: "add" },
    ProtocolCase { protocol: "sub", dunder: "__sub__", oracle: "sub" },
    ProtocolCase { protocol: "mul", dunder: "__mul__", oracle: "mul" },
    ProtocolCase { protocol: "truediv", dunder: "__truediv__", oracle: "truediv" },
    ProtocolCase { protocol: "floordiv", dunder: "__floordiv__", oracle: "floordiv" },
    ProtocolCase { protocol: "mod", dunder: "__mod__", oracle: "mod" },
    ProtocolCase { protocol: "pow", dunder: "__pow__", oracle: "pow" },
    ProtocolCase { protocol: "matmul", dunder: "__matmul__", oracle: "matmul" },
    ProtocolCase { protocol: "lshift", dunder: "__lshift__", oracle: "lshift" },
    ProtocolCase { protocol: "rshift", dunder: "__rshift__", oracle: "rshift" },
    ProtocolCase { protocol: "and", dunder: "__and__", oracle: "and" },
    ProtocolCase { protocol: "or", dunder: "__or__", oracle: "or" },
    ProtocolCase { protocol: "xor", dunder: "__xor__", oracle: "xor" },
    // ---- 算术（反射）----
    ProtocolCase { protocol: "radd", dunder: "__radd__", oracle: "radd" },
    ProtocolCase { protocol: "rsub", dunder: "__rsub__", oracle: "rsub" },
    ProtocolCase { protocol: "rmul", dunder: "__rmul__", oracle: "rmul" },
    ProtocolCase { protocol: "rtruediv", dunder: "__rtruediv__", oracle: "rtruediv" },
    ProtocolCase { protocol: "rfloordiv", dunder: "__rfloordiv__", oracle: "rfloordiv" },
    ProtocolCase { protocol: "rmod", dunder: "__rmod__", oracle: "rmod" },
    ProtocolCase { protocol: "rpow", dunder: "__rpow__", oracle: "rpow" },
    ProtocolCase { protocol: "rmatmul", dunder: "__rmatmul__", oracle: "rmatmul" },
    ProtocolCase { protocol: "rlshift", dunder: "__rlshift__", oracle: "rlshift" },
    ProtocolCase { protocol: "rrshift", dunder: "__rrshift__", oracle: "rrshift" },
    ProtocolCase { protocol: "rand", dunder: "__rand__", oracle: "rand" },
    ProtocolCase { protocol: "ror", dunder: "__ror__", oracle: "ror" },
    ProtocolCase { protocol: "rxor", dunder: "__rxor__", oracle: "rxor" },
    // ---- 原位算术 ----
    ProtocolCase { protocol: "iadd", dunder: "__iadd__", oracle: "iadd" },
    ProtocolCase { protocol: "isub", dunder: "__isub__", oracle: "isub" },
    ProtocolCase { protocol: "imul", dunder: "__imul__", oracle: "imul" },
    ProtocolCase { protocol: "itruediv", dunder: "__itruediv__", oracle: "itruediv" },
    ProtocolCase { protocol: "ifloordiv", dunder: "__ifloordiv__", oracle: "ifloordiv" },
    ProtocolCase { protocol: "imod", dunder: "__imod__", oracle: "imod" },
    ProtocolCase { protocol: "ipow", dunder: "__ipow__", oracle: "ipow" },
    ProtocolCase { protocol: "imatmul", dunder: "__imatmul__", oracle: "imatmul" },
    ProtocolCase { protocol: "ilshift", dunder: "__ilshift__", oracle: "ilshift" },
    ProtocolCase { protocol: "irshift", dunder: "__irshift__", oracle: "irshift" },
    ProtocolCase { protocol: "iand", dunder: "__iand__", oracle: "iand" },
    ProtocolCase { protocol: "ior", dunder: "__ior__", oracle: "ior" },
    ProtocolCase { protocol: "ixor", dunder: "__ixor__", oracle: "ixor" },
    // ---- 一元 ----
    ProtocolCase { protocol: "neg", dunder: "__neg__", oracle: "neg" },
    ProtocolCase { protocol: "pos", dunder: "__pos__", oracle: "pos" },
    ProtocolCase { protocol: "abs", dunder: "__abs__", oracle: "abs" },
    ProtocolCase { protocol: "invert", dunder: "__invert__", oracle: "invert" },
    // ---- 转换 ----
    ProtocolCase { protocol: "int", dunder: "__int__", oracle: "int" },
    ProtocolCase { protocol: "float", dunder: "__float__", oracle: "float" },
    ProtocolCase { protocol: "complex", dunder: "__complex__", oracle: "complex" },
    ProtocolCase { protocol: "index", dunder: "__index__", oracle: "index" },
];
