//! **`import` 族** —— 从 `builtin_objects.rs` **原样搬来** ✓（纯移动 ✓、零逻辑改动 ✓）。
//!
//! * 派发表 `import_method_native` **不搬** ✓（族之间的粘合层 ✓）；
//! * 仍在原处的被引用项：`execute`、`mounted_instance_dict`、`raise_builtin`、`read_file_through_fs`、`release`、`str_matches_public` ✓；
//! * `use` 块照抄原文件 ✓（`cargo fix` 随后删多余的 ✓），缺失项由自愈循环补 ✓。

use crate::executor::{mounted_instance_dict, read_file_through_fs, release, str_matches_public};
use crate::executor::Attribute;
use crate::builtin_objects::DictObject;
use crate::executor::ExecError;
use crate::frame::Frame;
use crate::header::Header;
use crate::instance::Instance;
use core::ptr::NonNull;
use crate::builtin_objects::TupleObject;
use crate::executor::attribute::attribute_lookup;



/// **取 `sys.path`**（一处真相 ✓，第 222 轮抽出）：`None` ＝ 取不到／不是列表 ✓。
fn module_search_path(instance: &Instance, sys_module: NonNull<Header>) -> Option<Vec<String>> {
    match attribute_lookup(instance, sys_module, "path") {
        Ok(Attribute::Owned(path)) | Ok(Attribute::Value(path)) => {
            let list_type = instance.type_named("list").expect("list 在引导期已登记");
            if instance.type_of(path) != list_type {
                return None;
            }
            // SAFETY: 类型身份刚确认是 list。
            Some(
                unsafe { &*path.as_ptr().cast::<crate::builtin_objects::ListObject>() }
                    .items()
                    .iter()
                    .filter_map(|item| instance.text_of(*item).map(|text| text.to_owned()))
                    .collect(),
            )
        }
        _ => None,
    }
}


/// 按 `sys.path` 找模块文件／包（`None` ＝ 没有 ✓）。
fn locate_source(instance: &Instance, name: &str) -> Option<(String, Option<String>)> {
    let modules = instance.modules()?;
    let sys_module = instance.dict_get(modules, "sys")?;
    for directory in module_search_path(instance, sys_module)? {
        let file = format!("{directory}/{name}.py");
        if let Some(source) = read_file_through_fs(instance, file.as_bytes()) {
            return Some((source, None));
        }
        let package_directory = format!("{directory}/{name}");
        let init = format!("{package_directory}/__init__.py");
        if let Some(source) = read_file_through_fs(instance, init.as_bytes()) {
            return Some((source, Some(package_directory)));
        }
    }
    None
}

/// **把某个模块装进"给定名字空间"**（第 235 轮；`IM-31` 的 **loader** 那半 ✓）：`_bootstrap` 先
/// `create_module` 拿到模块对象（名字空间就是装载目标 ✓），再 `exec_module` 调这里 ✓。
/// 找不到 ⇒ `false` ✓；**不**登记模块表（那是 `_bootstrap` 的活 ✓）。
///
/// **为什么单独成函数**：定位（`can_locate_through_bridge` ✓）与装载必须分开 ✓ ——
/// finder 的 `find_spec` 只许"找" ✗（把"装"放进去会让嵌套导入**重入**导入机制 ✗，第 217／222 轮 ✓）。
pub fn exec_module_into_namespace(
    instance: &Instance,
    name: &str,
    namespace: NonNull<Header>,
) -> Result<bool, ExecError> {
    let Some((source, package_directory)) = locate_source(instance, name) else {
        return Ok(false);
    };
    if let Some(directory) = package_directory {
        let path_list = instance.new_list(vec![instance.new_str(&directory)]);
        instance.dict_set(namespace, "__path__", path_list);
    }
    let chunk = format!("{name}.py");
    let unit = match crate::compile::compile(
        &source,
        &chunk,
        crate::compile::Mode::PurePython,
        crate::compile::CheckTier::Shallow,
        0,
    ) {
        Ok(unit) => unit,
        Err(crate::compile::CompileError::Syntax(message)) => {
            return Err(crate::executor::raise_builtin(instance, "SyntaxError", &message))
        }
        Err(crate::compile::CompileError::Unsupported(what)) => {
            return Err(crate::executor::raise_builtin(instance, "NotImplementedError", &what))
        }
    };
    let code = crate::compile::instantiate(instance, &unit);
    let frame_type = instance.type_named("frame").ok_or(ExecError::Unsupported {
        opcode: 0,
        what: "引导期没有登记 frame 类型",
    })?;
    instance.retain(namespace);
    let frame = instance.alloc(Frame::for_code_with_namespace(frame_type, &code, namespace));
    let outcome = crate::execute(instance, &frame);
    drop(frame);
    drop(code);
    outcome?;
    Ok(true)
}

/// **只定位、不执行**（第 222 轮；`IM-31`：finder 的 `find_spec` 只能"找" ✗ 不许"装" ✓）：
/// 报告"这座桥**找得到**这个名字吗" ✓ —— 先查模块表 ✓，再按 `sys.path` 的每个目录试
/// `<dir>/<名字>.py` 与 `<dir>/<名字>/__init__.py` ✓（**只读**，经 `fs` 域 ✓，**绝不执行** ✓）。
///
/// 为什么需要它（本轮的诊断 ✓）：把"装"放进 `find_spec` ⇒ 执行模块时的**嵌套导入**会再次进
/// `find_spec` ✗ ⇒ 重入导入机制（`_bootstrap` 的 finder 循环还在栈上 ✓）⇒ 实测
/// `target/recon/repro-217-segv.py` **SIGSEGV** ✗。定位与装载必须分开 ✓。
pub fn can_locate_through_bridge(instance: &Instance, name: &str) -> bool {
    let Some(modules) = instance.modules() else {
        return false;
    };
    if instance.dict_get(modules, name).is_some() {
        return true;
    }
    let Some(sys_module) = instance.dict_get(modules, "sys") else {
        return false;
    };
    let Some(entries) = module_search_path(instance, sys_module) else {
        return false;
    };
    entries.iter().any(|directory| {
        let module_file = format!("{directory}/{name}.py");
        let package_file = format!("{directory}/{name}/__init__.py");
        read_file_through_fs(instance, module_file.as_bytes()).is_some()
            || read_file_through_fs(instance, package_file.as_bytes()).is_some()
    })
}

/// **把过渡桥的加载能力交出去**（第 216 轮；`IM-30`…`IM-32` 的 ①a「Python 层 finder」的**前置** ✓）：
/// 给 `sys.meta_path` 上的 finder 用 ✓ —— `Some(模块)` ＝ 已装好并**登记进模块表** ✓；
/// `None` ＝ "这座桥找不到"（finder **必须**如实 `None` ✓，不许编假模块、也不许把"找不到"当异常抛 ✗）。
///
/// **为什么要有这一层**（第 215 轮实测 ✗→✓）：`load_module` 在找不到时**抛 `ModuleNotFoundError`** ✗，
/// 而 finder 的 `find_spec` 契约是**返回 `None`** ✗ ⇒ 这里**只**把"找不到"那一类映射成 `None` ✓；
/// **模块体自己抛的异常照旧上抛** ✓（那是真错误，不许吞 ✗）。
///
/// **交出约定**：`Some` 是**借用**（模块表持着它 ✓，与 [`load_module`] 同款 ✓）。
pub fn import_through_bridge(
    instance: &Instance,
    name: &str,
) -> Result<Option<NonNull<Header>>, ExecError> {
    let Some(modules) = instance.modules() else {
        return Ok(None);
    };
    match load_module(instance, modules, name, 0) {
        Ok(module) => Ok(Some(module)),
        // 桥用 `Unsupported` 表示"这形态还没接线" ⇒ 对 finder 也是"我没找到" ✓。
        Err(ExecError::Unsupported { .. }) => Ok(None),
        Err(ExecError::Raised { exception }) => {
            let type_name = instance.type_name(instance.type_of(exception));
            if type_name == "ModuleNotFoundError" || type_name == "ImportError" {
                Ok(None)
            } else {
                Err(ExecError::Raised { exception })
            }
        }
        Err(error) => Err(error),
    }
}

/// **最小的模块加载器**（`IM-`／`P3-12` 的第一片）：按 `sys.path` 找 `<dir>/<名字>.py`，
/// 用 `fs` 域读进来 ⇒ 编译 ⇒ 在**新名字空间**里执行 ⇒ 登记进模块表（与 `sys.modules` 同一份 ✓）。
///
/// 返回模块对象（**新引用** ✓）。这一层就是"VM 侧的 importlib 等价物"；**未接**：包
/// （`__init__.py`／`__path__`）、相对导入、`.pyc`、`sys.meta_path`、`site.py`（`IM-24`）✓。
/// **Rust 侧那座过渡桥**（`PLAN-milestones.md` 的 A1；回填条件见那条）✓。
///
/// **交出约定（第 281 轮统一 ✓）**：返回的是**借用**（`sys.modules` 持着它 ✓），与 `dict_get` 同款 ——
/// 先前"命中表就借用、真载入还多 `incref` 一份"**两套** ✗ ⇒ 那是**泄漏**（`MS-25` 那一族最忌讳
/// 记账不齐 ✓）⇒ 现在只有一套 ✓，调用方要留就自己 `retain` ✓。
pub(crate) fn load_module(
    instance: &Instance,
    modules: NonNull<Header>,
    name: &str,
    opcode_number: u8,
) -> Result<NonNull<Header>, ExecError> {
    let unsupported = |what: &'static str| ExecError::Unsupported { opcode: opcode_number, what };
    // **`sys.modules` 先赢** ✓（第 197 轮真 bug 修复 ✗）：CPython 的规矩是"已经在 `sys.modules` 里就直接用" ✓
    // —— `Lib/os.py:103` 自己就写 `sys.modules['os.path'] = path` ✓；少了这一查，
    // `from os.path import …`（`os.py:104` ✓）会去找"**父包的 `__path__`**" ✗ 并报
    // `父包没有 __path__（包才有 ✓）` ✗（实测 ✓）⇒ 于是 `os.py` 只剩半截 ✗（`name`／`path` 有 ✓，
    // `sep`／`curdir`／`environ` 全没有 ✗）—— 这正是 `site.py` 卡住的那一环 ✓。
    if let Some(existing) = instance.dict_get(modules, name) {
        return Ok(existing);
    }
    // `sys.path` 从模块表里的 `sys` 模块对象上取（模块属性 ✓）
    let sys_module = instance
        .dict_get(modules, "sys")
        .ok_or(unsupported("模块表里没有 `sys`（加载器要 `sys.path`）"))?;
    let entries: Vec<String> = module_search_path(instance, sys_module)
        .ok_or(unsupported("`sys.path` 取不到或不是列表（加载器需要它）"))?;
    // **带点的名字**（第 135 轮）：`import a.b` ⇒ 在**父包 `a` 的 `__path__`** 里找 `b` ✓，
    // 文件名用**最后一段** ✓，并把子模块挂成父包的属性 ✓（参照语义 ✓：`a.b` 之后 `a.b` 可见 ✓）。
    let split = name.rsplit_once('.');
    let (parent, base) = match split {
        Some((parent, base)) => (Some(parent), base),
        None => (None, name),
    };
    let entries: Vec<String> = match parent {
        Some(parent) => {
            // **父包先载入（可递归）** ✓（第 281 轮修 ✗）：CPython 的 `_find_and_load` 会把
            // `a.b.c` 的**每一级父包**都先装好 ✓；先前只查表 ✗ ⇒ 像 `xml.etree.ElementInclude`
            // 这种"顶层包不自己 import 子包"的导入当场报"父包不在表里" ✗（实测 112 个模块 ✓）。
            if instance.dict_get(modules, parent).is_none() {
                load_module(instance, modules, parent, opcode_number)?;
            }
            let parent_module = instance.dict_get(modules, parent).ok_or(unsupported(
                "带点的导入要先有父包在模块表里（相对导入／按需加载随后补）",
            ))?;
            match attribute_lookup(instance, parent_module, "__path__") {
                Ok(Attribute::Owned(path)) | Ok(Attribute::Value(path)) => {
                    let list_type = instance.type_named("list").expect("list 在引导期已登记");
                    if instance.type_of(path) != list_type {
                        return Err(unsupported("父包的 `__path__` 不是列表"));
                    }
                    // SAFETY: 类型身份刚确认是 list。
                    let list = unsafe { &*path.as_ptr().cast::<crate::builtin_objects::ListObject>() };
                    list.items()
                        .iter()
                        .filter_map(|item| instance.text_of(*item).map(|text| text.to_owned()))
                        .collect()
                }
                _ => return Err(unsupported("父包没有 `__path__`（包才有 ✓）")),
            }
        }
        None => entries,
    };
    let mut last_syntax: Option<String> = None;
    // 编译时"尚未接线"的那条（**如实上抛**，不要伪装成"模块不存在" ✗）
    let mut last_unsupported: Option<String> = None;
    for entry in &entries {
        // **候选**（第 135 轮）：先 `<dir>/<名字>.py` ✓，再 `<dir>/<名字>/__init__.py` ✓（包 ✓，
        // 还要给它 `__path__` ✓）。顺序与参照的 `FileFinder` 一致：**文件先、包后** ✓。
        let candidates: [(String, Option<String>); 2] = [
            (format!("{entry}/{base}.py"), None),
            (
                format!("{entry}/{base}/__init__.py"),
                Some(format!("{entry}/{base}")),
            ),
        ];
        for (file, package_directory) in candidates {
        if crate::diag::flag("PYAWA_TRACE_IMPORT") {
            eprintln!("[载入] 试 {file}（模块 {name}）");
        }
        let Some(source) = read_file_through_fs(instance, file.as_bytes()) else {
            continue; // 读不到就试下一个候选／下一个入口（`CP-2`／机器错误都当"这里没有" ✓）
        };
        let unit = match crate::compile::compile(
            &source,
            &file,
            crate::compile::Mode::PurePython,
            crate::compile::CheckTier::Shallow,
            0,
        ) {
            Ok(unit) => unit,
            Err(crate::compile::CompileError::Syntax(message)) => {
                last_syntax = Some(message);
                continue;
            }
            Err(crate::compile::CompileError::Unsupported(what)) => {
                // **如实记下**（第 139 轮）：此前静默 continue ✗ ⇒ 最后只报"找不到模块" ✗
                last_unsupported = Some(what);
                continue;
            }
        };
        // **新名字空间** ＋ `__name__`（照参照实现的模块语义 ✓）
        let namespace = instance.new_dict();
        let module_name = instance.new_str(name);
        instance.dict_set(namespace, "__name__", module_name);
        // **包**：`__path__` ＝ 该包目录（参照语义 ✓；子模块导入要靠它 ✓）
        if let Some(package_directory) = &package_directory {
            let path_text = instance.new_str(package_directory);
            let path_list = instance.new_list(vec![path_text]);
            instance.dict_set(namespace, "__path__", path_list);
        }
        // 模块对象：`AttributeObject` ＋ 名字空间（`module` 类型缺就建 ✓）
        let module_type = instance
            .type_named("module")
            .unwrap_or_else(|| instance.new_attribute_type("module"));
        let module = instance
            .alloc(crate::builtin_objects::AttributeObject::new(
                module_type,
                core::cell::RefCell::new(Some(namespace)),
            ))
            .into_raw()
            .cast::<Header>();
        // **先登记再执行**（环状导入要能看到半成品 ✓，与参照一致）
        instance.dict_set(modules, name, module);
        let code = crate::compile::instantiate(instance, &unit);
        let frame_type = instance.type_named("frame").ok_or(unsupported("引导期没有 `Frame` 类型"))?;
        // SAFETY: `namespace` 由模块对象持有，存活。
        unsafe { instance.incref_object(namespace.as_ptr()) };
        let frame = instance
            .alloc(crate::Frame::for_code_with_namespace(frame_type, &code, namespace));
        let outcome = crate::execute(instance, &frame);
        // **诊断** ✓（第 197 轮）：模块**先登记**后执行 ✓ ⇒ 一旦执行出错，`sys.modules` 里会**留下半截模块** ✗。
        // 这里在**开着 `PYAWA_TRACE_IMPORT`** 时把那个错**如实打出来** ✓（默认零输出 ✓）。
        if crate::diag::flag("PYAWA_TRACE_IMPORT") {
            if let Err(error) = &outcome {
                eprintln!("[载入] 模块 {name} 执行出错：{error:?}");
            }
        }
        if crate::diag::flag("PYAWA_TRACE_IMPORT") {
            // **诊断** ✓（第 200 轮）：执行完**命名空间里有多少个名字** ✓（好判断"写入是否落地" ✗）。
            let size = crate::mounted_instance_dict(instance, module)
                .map(|dict| {
                    // SAFETY: dict 是存活对象。
                    unsafe { &*dict.as_ptr().cast::<crate::builtin_objects::DictObject>() }
                        .entries()
                        .len()
                })
                .unwrap_or(0);
            eprintln!("[载入] 模块 {name} 执行完：命名空间 {size} 个名字");
        }
        drop(frame);
        drop(code);
        outcome?;
        // **挂成父包的属性**（第 135 轮）：`import a.b` 之后 `a.b` 要能取到 ✓（参照语义 ✓）
        if let (Some(parent), Some(base)) = (parent, split.map(|_| base)) {
            if let Some(parent_module) = instance.dict_get(modules, parent) {
                if let Some(parent_namespace) = crate::mounted_instance_dict(instance, parent_module) {
                    instance.dict_set(parent_namespace, base, module);
                }
            }
        }
        // **借用**交出 ✓（约定见本函数文档：只有一套 ✓）
        return Ok(module);
    }
    }
    // **按实报错**（第 139 轮）：文件找到了、但编译不过 ⇒ 说清是哪个模块、哪句话 ✓；
    // 编译时撞到"尚未接线" ⇒ 原样上抛 ✓；两者都没有 ⇒ 才是真的"找不到" ✓。
    if let Some(message) = last_syntax {
        let text = format!("加载模块 '{name}'：{message}");
        return Err(instance.raise_builtin_error("SyntaxError", &text));
    }
    if let Some(what) = last_unsupported {
        // 编译撞到"尚未接线" ⇒ **原样如实上抛**（点明模块名 ✓，不再伪装成"模块不存在" ✗）
        let text = format!("加载模块 '{name}'：{what}");
        return Err(instance.raise_builtin_error("NotImplementedError", &text));
    }
    // **找不到就报 `ModuleNotFoundError`** ✓（第 162 轮）：参照里 `import 不存在的名字` 抛的就是它 ✓，
    //   而**不是**一个"未接线"的硬错误 ✗ —— 后者会让 `try: from _abc import … except ImportError:` 这类
    //   **合法回退**接不住 ✓（`Lib/abc.py:85` 正是这样 ✓：`_abc` 是 C 内建模块 ✗，参照回退到 `_py_abc` ✓）。
    Err(instance.raise_builtin_error(
        "ModuleNotFoundError",
        &format!("No module named '{name}'"),
    ))
}

/// **相对导入的名字解析** ✓（`IMPORT_NAME` 的 `level > 0`；第 278 轮接线）。
///
/// 参照口径：`__package__` 优先 ✓；空则看 `__path__` 在不在（在 ⇒ 当前就是包 ⇒ 用 `__name__` ✓），
/// 否则取 `__name__` 去掉最后一段 ✓；再按 `level` 往上走（`level == 1` ⇒ 当前包本身 ✓）。
/// `level` 越过顶层 ⇒ 照参照报 `ImportError: attempted relative import beyond top-level package` ✓。
pub(crate) fn resolve_relative_import(
    instance: &Instance,
    frame: &Frame,
    raw: &str,
    level: usize,
    opcode: u8,
) -> Result<String, ExecError> {
    // **模块级帧用 `namespace`、函数帧用 `globals`** ✓（实测：`importlib/__init__.py` 的相对导入
    // 在模块级帧上跑 ⇒ `globals` 是 `None` ✗）⇒ 两个都看 ✓。
    let globals = frame
        .globals()
        .or_else(|| frame.namespace())
        .ok_or(ExecError::Unsupported {
            opcode,
            what: "相对导入需要当前模块的名字空间（`globals`／`namespace` 都是空）",
        })?;
    let text = |key: &str| {
        instance
            .dict_get(globals, key)
            .and_then(|value| instance.text_of(value))
            .map(str::to_owned)
    };
    let name = text("__name__").unwrap_or_default();
    let package = match text("__package__") {
        Some(package) if !package.is_empty() => package,
        _ => {
            if instance.dict_get(globals, "__path__").is_some() {
                name.clone()
            } else {
                match name.rfind('.') {
                    Some(index) => name[..index].to_owned(),
                    None => String::new(),
                }
            }
        }
    };
    // 与参照的 `package.rsplit('.', level - 1)` 同义 ✓
    let bits: Vec<&str> = package.rsplitn(level, '.').collect();
    if bits.len() < level {
        return Err(instance.raise_builtin_error(
            "ImportError",
            "attempted relative import beyond top-level package",
        ));
    }
    let base = bits[bits.len() - 1];
    if raw.is_empty() {
        Ok(base.to_owned())
    } else {
        Ok(format!("{base}.{raw}"))
    }
}

/// **`IMPORT_NAME` 的 fromlist 那一步**（第 279 轮接线；参照 `importlib._bootstrap._handle_fromlist`）。
///
/// 口径（照参照 ✓）：
/// - **只有包**（模块命名空间里有 `__path__` ✓）才做这一步 —— 普通模块没有子模块 ✓；
/// - 逐个名字：**已经是模块属性** ⇒ 跳过 ✓（随后 `IMPORT_FROM` 会取到它 ✓）；否则把
///   `<模块名>.<名字>` 当**子模块**导入 ✓（`load_module` 会把它挂成父包的属性 ✓）；
/// - 子模块**真不存在** ⇒ **忽略** ✓（参照的向下兼容：交给随后的 `IMPORT_FROM` 去报
///   `AttributeError` ✓）；子模块**自己执行出错**等 ⇒ **原样上抛** ✓，**不得**吞 ✗（吞了会把
///   `Lib/` 里的真 bug 伪装成"这个名字没有" ✗）。
pub(crate) fn handle_fromlist(
    instance: &Instance,
    modules: NonNull<Header>,
    module: NonNull<Header>,
    fromlist: NonNull<Header>,
    module_name: &str,
    opcode_number: u8,
) -> Result<(), ExecError> {
    // `fromlist` 是编译器发的**元组常量**（`import a` ⇒ 空元组 ✓）；不是元组就当作没有 ✓。
    if instance.type_name(instance.type_of(fromlist)) != "tuple" {
        return Ok(());
    }
    // **包才有子模块** ✓（参照：`hasattr(module, '__path__')` ✓）
    if !module_has_name(instance, module, "__path__") {
        return Ok(());
    }
    // SAFETY: 上面刚确认 fromlist 的类型是 `tuple`。
    let names: Vec<String> = unsafe { &*fromlist.as_ptr().cast::<TupleObject>() }
        .items()
        .iter()
        .filter_map(|item| instance.text_of(*item).map(str::to_owned))
        .collect();
    for name in names {
        if module_has_name(instance, module, &name) {
            continue;
        }
        let from = format!("{module_name}.{name}");
        match load_module(instance, modules, &from, opcode_number) {
            // **交的是借用** ✓（`load_module` 的约定，第 281 轮统一 ✓）⇒ 这里**不还** ✓
            Ok(_loaded) => {}
            Err(error) => {
                if !ignore_missing_submodule(instance, modules, &from, &error) {
                    return Err(error);
                }
                // **吞掉它** ✓（照参照的 `continue` ✓）：异常状态一份、`Err` 一份，**两份都要还** ✓
                if let ExecError::Raised { exception } = error {
                    if let Some(previous) = instance.set_pending_exception(None) {
                        release(instance, previous);
                    }
                    release(instance, exception);
                }
            }
        }
    }
    Ok(())
}

/// 探测"这个名字在不在"（`hasattr` 的等价物）—— **只查模块命名空间** ✓。
///
/// **为什么不用 `attribute_lookup`**：它以 `Err(Raised)` 报"没有"，而 `raise_builtin` 会**写
/// `pending_exception`** ✗ ⇒ 拿它当探针会**污染异常状态**（`hasattr` 是只读的 ✓）。
pub(crate) fn module_has_name(instance: &Instance, module: NonNull<Header>, name: &str) -> bool {
    module_value(instance, module, name).is_some()
}

/// 从模块命名空间里读一个属性（读不到 ⇒ `None`）—— **只看"在不在"，不看类型** ✓
/// （`__path__` 是**列表** ✗ ⇒ 不能拿 [`module_text`] 当存在性判据 ✗）。
pub(crate) fn module_value(
    instance: &Instance,
    module: NonNull<Header>,
    name: &str,
) -> Option<NonNull<Header>> {
    let namespace = mounted_instance_dict(instance, module)?;
    // **只有真的是 `dict` 才能按 `DictObject` 取项** ✓（第 185 轮的教训同款 ✗）
    if instance.type_name(instance.type_of(namespace)) != "dict" {
        return None;
    }
    // SAFETY: 上面刚确认 namespace 的类型是 `dict`。
    let entries = unsafe { &*namespace.as_ptr().cast::<DictObject>() }.entries();
    entries
        .into_iter()
        .find(|(key, _)| str_matches_public(instance, *key, name))
        .map(|(_, value)| value)
}

/// 从模块命名空间里读一个**字符串**属性（读不到 ⇒ `None`）。
pub fn module_text(instance: &Instance, module: NonNull<Header>, name: &str) -> Option<String> {
    module_value(instance, module, name)
        .and_then(|value| instance.text_of(value).map(str::to_owned))
}

/// 失败的子模块导入该不该**忽略** ✓（参照 `_handle_fromlist` 的向下兼容）。
///
/// 三条同时成立才忽略 ✓：① 是 `ModuleNotFoundError`；② 消息里的名字**就是** `from`；
/// ③ `sys.modules` 里**没留下**它（参照的 `sys.modules.get(from, _ERR_MSG) is _ERR_MSG` ——
/// 半截模块留在表里 ⇒ 那是"它存在但跑挂了" ✗，**必须**上抛 ✓）。
pub(crate) fn ignore_missing_submodule(
    instance: &Instance,
    modules: NonNull<Header>,
    from: &str,
    error: &ExecError,
) -> bool {
    let ExecError::Raised { exception } = error else {
        return false;
    };
    let exception = *exception;
    if instance.type_name(instance.type_of(exception)) != "ModuleNotFoundError" {
        return false;
    }
    let expected = format!("No module named '{from}'");
    if instance.exception_message_of(exception).as_deref() != Some(expected.as_str()) {
        return false;
    }
    instance.dict_get(modules, from).is_none()
}
