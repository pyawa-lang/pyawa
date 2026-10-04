//! **TS-41**：内建类型的集合与基类关系——**由 `tools/gen_builtin_types.py` 从参照实现探测生成**。
//!
//! **禁止手改本文件**；也**禁止**在别处手写内建类型枚举（`PLAN-milestones.md` §9.4 的复核清单）。
//! `TS-42` 的阶梯只标先后：`ladder` 字段说明这个类型被哪一批指令族逼出来。
//!
//! 载荷布局**不**在这张表里：`TS-43` 把它留给实现。

/// `TS-42` 的阶梯。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ladder {
    /// 常量、名、跳转、运算符、`co_names`／`co_varnames` 逼出来的那一批。
    Step1,
    /// 容器与解包、调用与返回、迭代、异常四族逼出来的那一批。
    M2,
    /// 其余（随 `CM-` 分批）。
    Later,
}

/// 一个内建类型的层次信息（名字、直接基类、MRO）。
#[derive(Debug)]
pub struct BuiltinType {
    /// `__name__`。
    pub name: &'static str,
    /// `__bases__` 的名字（升序保持参照实现的顺序）。
    pub bases: &'static [&'static str],
    /// `__mro__` 的名字（含自身）。
    pub mro: &'static [&'static str],
    /// `TS-42` 的阶梯。
    pub ladder: Ladder,
}

/// **TS-41** 的表，按名字升序（可用 [`builtin_type`] 二分查找）。
pub static BUILTIN_TYPES: &[BuiltinType] = &[
    BuiltinType {
        name: "ArithmeticError",
        bases: &["Exception"],
        mro: &["ArithmeticError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "AssertionError",
        bases: &["Exception"],
        mro: &["AssertionError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "AttributeError",
        bases: &["Exception"],
        mro: &["AttributeError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "BaseException",
        bases: &["object"],
        mro: &["BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "BaseExceptionGroup",
        bases: &["BaseException"],
        mro: &["BaseExceptionGroup", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "BlockingIOError",
        bases: &["OSError"],
        mro: &["BlockingIOError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "BrokenPipeError",
        bases: &["ConnectionError"],
        mro: &["BrokenPipeError", "ConnectionError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "BufferError",
        bases: &["Exception"],
        mro: &["BufferError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "BuiltinImporter",
        bases: &["object"],
        mro: &["BuiltinImporter", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "BytesWarning",
        bases: &["Warning"],
        mro: &["BytesWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ChildProcessError",
        bases: &["OSError"],
        mro: &["ChildProcessError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ConnectionAbortedError",
        bases: &["ConnectionError"],
        mro: &["ConnectionAbortedError", "ConnectionError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ConnectionError",
        bases: &["OSError"],
        mro: &["ConnectionError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ConnectionRefusedError",
        bases: &["ConnectionError"],
        mro: &["ConnectionRefusedError", "ConnectionError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ConnectionResetError",
        bases: &["ConnectionError"],
        mro: &["ConnectionResetError", "ConnectionError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "DeprecationWarning",
        bases: &["Warning"],
        mro: &["DeprecationWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "DynamicClassAttribute",
        bases: &["object"],
        mro: &["DynamicClassAttribute", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "EOFError",
        bases: &["Exception"],
        mro: &["EOFError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "EncodingWarning",
        bases: &["Warning"],
        mro: &["EncodingWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "Exception",
        bases: &["BaseException"],
        mro: &["Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ExceptionGroup",
        bases: &["BaseExceptionGroup", "Exception"],
        mro: &["ExceptionGroup", "BaseExceptionGroup", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "FileExistsError",
        bases: &["OSError"],
        mro: &["FileExistsError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "FileNotFoundError",
        bases: &["OSError"],
        mro: &["FileNotFoundError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "FloatingPointError",
        bases: &["ArithmeticError"],
        mro: &["FloatingPointError", "ArithmeticError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "FutureWarning",
        bases: &["Warning"],
        mro: &["FutureWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "GeneratorExit",
        bases: &["BaseException"],
        mro: &["GeneratorExit", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "GenericAlias",
        bases: &["object"],
        mro: &["GenericAlias", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "ImportError",
        bases: &["Exception"],
        mro: &["ImportError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ImportWarning",
        bases: &["Warning"],
        mro: &["ImportWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "IndentationError",
        bases: &["SyntaxError"],
        mro: &["IndentationError", "SyntaxError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "IndexError",
        bases: &["LookupError"],
        mro: &["IndexError", "LookupError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "InterruptedError",
        bases: &["OSError"],
        mro: &["InterruptedError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "IsADirectoryError",
        bases: &["OSError"],
        mro: &["IsADirectoryError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "KeyError",
        bases: &["LookupError"],
        mro: &["KeyError", "LookupError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "KeyboardInterrupt",
        bases: &["BaseException"],
        mro: &["KeyboardInterrupt", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "LookupError",
        bases: &["Exception"],
        mro: &["LookupError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "MemoryError",
        bases: &["Exception"],
        mro: &["MemoryError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ModuleNotFoundError",
        bases: &["ImportError"],
        mro: &["ModuleNotFoundError", "ImportError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "NameError",
        bases: &["Exception"],
        mro: &["NameError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "NoneType",
        bases: &["object"],
        mro: &["NoneType", "object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "NotADirectoryError",
        bases: &["OSError"],
        mro: &["NotADirectoryError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "NotImplementedError",
        bases: &["RuntimeError"],
        mro: &["NotImplementedError", "RuntimeError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "NotImplementedType",
        bases: &["object"],
        mro: &["NotImplementedType", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "OSError",
        bases: &["Exception"],
        mro: &["OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "OverflowError",
        bases: &["ArithmeticError"],
        mro: &["OverflowError", "ArithmeticError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "PendingDeprecationWarning",
        bases: &["Warning"],
        mro: &["PendingDeprecationWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "PermissionError",
        bases: &["OSError"],
        mro: &["PermissionError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ProcessLookupError",
        bases: &["OSError"],
        mro: &["ProcessLookupError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "PyCapsule",
        bases: &["object"],
        mro: &["PyCapsule", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "PythonFinalizationError",
        bases: &["RuntimeError"],
        mro: &["PythonFinalizationError", "RuntimeError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "RecursionError",
        bases: &["RuntimeError"],
        mro: &["RecursionError", "RuntimeError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ReferenceError",
        bases: &["Exception"],
        mro: &["ReferenceError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ResourceWarning",
        bases: &["Warning"],
        mro: &["ResourceWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "RuntimeError",
        bases: &["Exception"],
        mro: &["RuntimeError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "RuntimeWarning",
        bases: &["Warning"],
        mro: &["RuntimeWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "SimpleNamespace",
        bases: &["object"],
        mro: &["SimpleNamespace", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "StopAsyncIteration",
        bases: &["Exception"],
        mro: &["StopAsyncIteration", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "StopIteration",
        bases: &["Exception"],
        mro: &["StopIteration", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "SyntaxError",
        bases: &["Exception"],
        mro: &["SyntaxError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "SyntaxWarning",
        bases: &["Warning"],
        mro: &["SyntaxWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "SystemError",
        bases: &["Exception"],
        mro: &["SystemError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "SystemExit",
        bases: &["BaseException"],
        mro: &["SystemExit", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "TabError",
        bases: &["IndentationError"],
        mro: &["TabError", "IndentationError", "SyntaxError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "TimeoutError",
        bases: &["OSError"],
        mro: &["TimeoutError", "OSError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "TypeError",
        bases: &["Exception"],
        mro: &["TypeError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "UnboundLocalError",
        bases: &["NameError"],
        mro: &["UnboundLocalError", "NameError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "UnicodeDecodeError",
        bases: &["UnicodeError"],
        mro: &["UnicodeDecodeError", "UnicodeError", "ValueError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "UnicodeEncodeError",
        bases: &["UnicodeError"],
        mro: &["UnicodeEncodeError", "UnicodeError", "ValueError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "UnicodeError",
        bases: &["ValueError"],
        mro: &["UnicodeError", "ValueError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "UnicodeTranslateError",
        bases: &["UnicodeError"],
        mro: &["UnicodeTranslateError", "UnicodeError", "ValueError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "UnicodeWarning",
        bases: &["Warning"],
        mro: &["UnicodeWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "Union",
        bases: &["object"],
        mro: &["Union", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "UserWarning",
        bases: &["Warning"],
        mro: &["UserWarning", "Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ValueError",
        bases: &["Exception"],
        mro: &["ValueError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "Warning",
        bases: &["Exception"],
        mro: &["Warning", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "ZeroDivisionError",
        bases: &["ArithmeticError"],
        mro: &["ZeroDivisionError", "ArithmeticError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "_GeneratorWrapper",
        bases: &["object"],
        mro: &["_GeneratorWrapper", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "_IncompleteInputError",
        bases: &["SyntaxError"],
        mro: &["_IncompleteInputError", "SyntaxError", "Exception", "BaseException", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "async_generator",
        bases: &["object"],
        mro: &["async_generator", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "bool",
        bases: &["int"],
        mro: &["bool", "int", "object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "builtin_function_or_method",
        bases: &["object"],
        mro: &["builtin_function_or_method", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "bytearray",
        bases: &["object"],
        mro: &["bytearray", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "bytearray_iterator",
        bases: &["object"],
        mro: &["bytearray_iterator", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "bytes",
        bases: &["object"],
        mro: &["bytes", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "bytes_iterator",
        bases: &["object"],
        mro: &["bytes_iterator", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "cell",
        bases: &["object"],
        mro: &["cell", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "classmethod",
        bases: &["object"],
        mro: &["classmethod", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "classmethod_descriptor",
        bases: &["object"],
        mro: &["classmethod_descriptor", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "code",
        bases: &["object"],
        mro: &["code", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "complex",
        bases: &["object"],
        mro: &["complex", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "coroutine",
        bases: &["object"],
        mro: &["coroutine", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "dict",
        bases: &["object"],
        mro: &["dict", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "dict_keyiterator",
        bases: &["object"],
        mro: &["dict_keyiterator", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "ellipsis",
        bases: &["object"],
        mro: &["ellipsis", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "enumerate",
        bases: &["object"],
        mro: &["enumerate", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "filter",
        bases: &["object"],
        mro: &["filter", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "float",
        bases: &["object"],
        mro: &["float", "object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "frame",
        bases: &["object"],
        mro: &["frame", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "frozenset",
        bases: &["object"],
        mro: &["frozenset", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "function",
        bases: &["object"],
        mro: &["function", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "generator",
        bases: &["object"],
        mro: &["generator", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "getset_descriptor",
        bases: &["object"],
        mro: &["getset_descriptor", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "int",
        bases: &["object"],
        mro: &["int", "object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "list",
        bases: &["object"],
        mro: &["list", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "list_iterator",
        bases: &["object"],
        mro: &["list_iterator", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "list_reverseiterator",
        bases: &["object"],
        mro: &["list_reverseiterator", "object"],
        // **`reversed(list)` 的迭代器** ✓（第 227 轮）：`_collections_abc.py:75` 要它 ✓。
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "longrange_iterator",
        bases: &["object"],
        mro: &["longrange_iterator", "object"],
        // **`range(<超出 i64 的上限>)` 的迭代器** ✓（第 228 轮）：`_collections_abc.py:77` 要它 ✓。
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "map",
        bases: &["object"],
        mro: &["map", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "mappingproxy",
        bases: &["object"],
        mro: &["mappingproxy", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "member_descriptor",
        bases: &["object"],
        mro: &["member_descriptor", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "memoryview",
        bases: &["object"],
        mro: &["memoryview", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "method",
        bases: &["object"],
        mro: &["method", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "method-wrapper",
        bases: &["object"],
        mro: &["method-wrapper", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "method_descriptor",
        bases: &["object"],
        mro: &["method_descriptor", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "module",
        bases: &["object"],
        mro: &["module", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "object",
        bases: &[],
        mro: &["object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "property",
        bases: &["object"],
        mro: &["property", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "range",
        bases: &["object"],
        mro: &["range", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "range_iterator",
        bases: &["object"],
        mro: &["range_iterator", "object"],
        // **`range()` 的迭代器** ✓（第 228 轮）：参照给小范围这个**名字** ✓（我们先前一律给 `islice` ✗ ⇒ 旧偏差 ✓）。
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "reversed",
        bases: &["object"],
        mro: &["reversed", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "set",
        bases: &["object"],
        mro: &["set", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "set_iterator",
        bases: &["object"],
        mro: &["set_iterator", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "slice",
        bases: &["object"],
        mro: &["slice", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "staticmethod",
        bases: &["object"],
        mro: &["staticmethod", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "str",
        bases: &["object"],
        mro: &["str", "object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "str_ascii_iterator",
        bases: &["object"],
        mro: &["str_ascii_iterator", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "super",
        bases: &["object"],
        mro: &["super", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "traceback",
        bases: &["object"],
        mro: &["traceback", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "tuple",
        bases: &["object"],
        mro: &["tuple", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "tuple_iterator",
        bases: &["object"],
        mro: &["tuple_iterator", "object"],
        ladder: Ladder::M2,
    },
    BuiltinType {
        name: "type",
        bases: &["object"],
        mro: &["type", "object"],
        ladder: Ladder::Step1,
    },
    BuiltinType {
        name: "wrapper_descriptor",
        bases: &["object"],
        mro: &["wrapper_descriptor", "object"],
        ladder: Ladder::Later,
    },
    BuiltinType {
        name: "zip",
        bases: &["object"],
        mro: &["zip", "object"],
        ladder: Ladder::Later,
    },
];

/// 按名字取一个内建类型（表按名字升序，故二分）。
pub fn builtin_type(name: &str) -> Option<&'static BuiltinType> {
    BUILTIN_TYPES
        .binary_search_by(|entry| entry.name.cmp(name))
        .ok()
        .map(|index| &BUILTIN_TYPES[index])
}
