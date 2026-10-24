//! `executor` 的 `step_match` 域：`match` 的模式匹配指令。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_match<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "MATCH_SEQUENCE" => {
                    // 净 +1：压"是不是序列"，被测对象留着。实测 `str`／`dict` **不算**序列
                    let subject = frame.get().peek()?;
                    // SAFETY: subject 在帧值栈上，存活。
                    let ty = unsafe { subject.as_ref() }.ty();
                    // 参照实测：`list`／`tuple` 的**子类**也算序列 ✓（`str`／`dict`／`set` 不算 ✓）。
                    let truth = instance.is_subtype(ty, builtin_type(instance, "list"))
                        || instance.is_subtype(ty, builtin_type(instance, "tuple"));
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "MATCH_MAPPING" => {
                    // 净 +1：本层只有 `dict` 算映射
                    let subject = frame.get().peek()?;
                    // SAFETY: subject 在帧值栈上，存活。
                    // 参照实测：`dict` 的**子类**也算映射 ✓（`collections.UserDict` 那种 ABC 面本层未接 ✓）。
                    let truth = instance
                        .is_subtype(unsafe { subject.as_ref() }.ty(), builtin_type(instance, "dict"));
                    let raw = instance.singletons().boolean(truth);
                    push(instance, frame.get(), raw)?;
                }
                "MATCH_KEYS" => {
                    // 净 +1：栈是 `[被测映射, 键的 tuple]`——**两者都留着**，再压"值的 tuple"；
                    // 任一键缺失就压 `None`（实测：缺键 ⇒ 这个 case 不匹配）
                    let keys = frame.get().peek()?;
                    let subject = frame.get().peek_from_top(2)?;
                    // SAFETY: 两个都在帧值栈上，存活。
                    let keys_type = unsafe { keys.as_ref() }.ty();
                    if keys_type != builtin_type(instance, "tuple") {
                        return Err(ExecError::Unsupported {
                            opcode: opcode_number,
                            what: "MATCH_KEYS 的键必须是 tuple（编译器保证）",
                        });
                    }
                    // SAFETY: 类型身份已确认。
                    let key_items = unsafe { &*keys.as_ptr().cast::<TupleObject>() };
                    // SAFETY: subject 是存活对象。
                    let subject_type = unsafe { subject.as_ref() }.ty();
                    if subject_type != builtin_type(instance, "dict") {
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                        return Ok(Step::Continue);
                    }
                    // SAFETY: 类型身份已确认。
                    let mapping = unsafe { &*subject.as_ptr().cast::<DictObject>() };
                    let mut values: Vec<NonNull<Header>> = Vec::with_capacity(key_items.len());
                    let mut missing = false;
                    for index in 0..key_items.len() {
                        let key = key_items.item(index).expect("下标在范围内");
                        match mapping
                            .entries()
                            .iter()
                            .position(|(existing, _)| values_equal(instance, *existing, key))
                        {
                            Some(position) => {
                                let (_, value) = mapping.entry(position).expect("刚查到的位置");
                                // SAFETY: value 由字典持有，存活。
                                unsafe { instance.incref_object(value.as_ptr()) };
                                values.push(value);
                            }
                            None => {
                                missing = true;
                                break;
                            }
                        }
                    }
                    if missing {
                        for value in values {
                            release(instance, value);
                        }
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                    } else {
                        let tuple = instance.new_tuple(values);
                        frame.get().push(tuple)?;
                    }
                }
                "MATCH_CLASS" => {
                    // 净 −2：栈是 `[被测对象, 类, 关键字名 tuple]`——**被测对象也被吃掉**，
                    // 命中就压"取出的属性 tuple"，不命中就压 `None`（实测形状：
                    // `MATCH_CLASS n; COPY 1; POP_JUMP_IF_NONE L; UNPACK_SEQUENCE …`）。
                    //
                    // 位置形参（task-37 补 ✓）照参照 3.14.4：
                    // - 类有 `__match_args__` ⇒ 必须是 tuple，前 `n` 个名字依次取属性 ✓；
                    // - 没有 `__match_args__` 时，`_Py_TPFLAGS_MATCH_SELF` 一族（int/str/float/bytes/
                    //   bytearray/list/tuple/set/frozenset/dict 及其子类 ✓）允许**一个**位置子模式，
                    //   直接匹配 subject 自己 ✓（实测 `case str(s)`／`case int(i)` ✓）；
                    // - 其余类型：0 个位置子模式，否则 TypeError（文案照实测 ✓）。
                    // 属性**缺失** ⇒ 该模式不命中（压 None ✓，参照同款）；其它属性错误照抛 ✓。
                    // 关键字与位置属性重名 ⇒ `TypeError: … got multiple sub-patterns for attribute 'x'` ✓。
                    let names = frame.get().pop()?;
                    let class_object = frame.get().pop()?;
                    let subject = frame.get().pop()?;
                    // SAFETY: class_object 是刚出栈的存活对象。
                    let class_type = unsafe { class_object.as_ref() }.ty();
                    if class_type != builtin_type(instance, "type") {
                        release(instance, names);
                        release(instance, class_object);
                        release(instance, subject);
                        return Err(raise_builtin(
                            instance,
                            "TypeError",
                            "called match pattern must be a type",
                        ));
                    }
                    let class = class_object.cast::<TypeObject>();
                    // SAFETY: subject 是存活对象。
                    let subject_type = unsafe { subject.as_ref() }.ty();
                    let matched = instance.is_subtype(subject_type, class);
                    release(instance, class_object);
                    if !matched {
                        release(instance, names);
                        release(instance, subject);
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                        return Ok(Step::Continue);
                    }
                    let class_name = instance.type_name(class);
                    // 关键字名必须是 str（编译器保证 ✓）。
                    // SAFETY: names 是 tuple（编译器保证）。
                    let name_items = unsafe { &*names.as_ptr().cast::<TupleObject>() };
                    let mut keywords: Vec<String> = Vec::with_capacity(name_items.len());
                    for index in 0..name_items.len() {
                        let name = name_items.item(index).expect("下标在范围内");
                        // SAFETY: name 由 tuple 持有，存活。
                        let name_type = unsafe { name.as_ref() }.ty();
                        if name_type != instance.singletons().str_type() {
                            release(instance, names);
                            release(instance, subject);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "MATCH_CLASS 的关键字名必须是 str",
                            });
                        }
                        // SAFETY: 类型身份已确认。
                        keywords.push(
                            unsafe { &*name.as_ptr().cast::<StrObject>() }
                                .value()
                                .to_owned(),
                        );
                    }
                    // 位置子模式的名字（`__match_args__` 面）。
                    let mut positional_names: Vec<String> = Vec::with_capacity(oparg);
                    let mut values: Vec<NonNull<Header>> = Vec::with_capacity(oparg + keywords.len());
                    let mut self_matched = false;
                    if oparg > 0 {
                        match instance.type_lookup(class, "__match_args__") {
                            Some(attributes) => {
                                // SAFETY: attributes 由类型字典持有，存活。
                                let attributes_type = unsafe { attributes.as_ref() }.ty();
                                if attributes_type != builtin_type(instance, "tuple") {
                                    let got = instance.type_name(attributes_type);
                                    release(instance, names);
                                    release(instance, subject);
                                    return Err(raise_builtin(
                                        instance,
                                        "TypeError",
                                        &format!(
                                            "{class_name}.__match_args__ must be a tuple (got {got})"
                                        ),
                                    ));
                                }
                                // SAFETY: 类型身份已确认。
                                let match_args =
                                    unsafe { &*attributes.as_ptr().cast::<TupleObject>() };
                                if match_args.len() < oparg {
                                    let allowed = match_args.len();
                                    let plural = if allowed == 1 { "" } else { "s" };
                                    release(instance, names);
                                    release(instance, subject);
                                    return Err(raise_builtin(
                                        instance,
                                        "TypeError",
                                        &format!(
                                            "{class_name}() accepts {allowed} positional sub-pattern{plural} ({oparg} given)"
                                        ),
                                    ));
                                }
                                for index in 0..oparg {
                                    let name = match_args.item(index).expect("下标在范围内");
                                    // SAFETY: name 由 tuple 持有，存活。
                                    let name_type = unsafe { name.as_ref() }.ty();
                                    if name_type != instance.singletons().str_type() {
                                        let got = instance.type_name(name_type);
                                        release(instance, names);
                                        release(instance, subject);
                                        return Err(raise_builtin(
                                            instance,
                                            "TypeError",
                                            &format!(
                                                "__match_args__ elements must be strings (got {got})"
                                            ),
                                        ));
                                    }
                                    // SAFETY: 类型身份已确认。
                                    positional_names.push(
                                        unsafe { &*name.as_ptr().cast::<StrObject>() }
                                            .value()
                                            .to_owned(),
                                    );
                                }
                            }
                            None => {
                                if !match_self_class(instance, class) {
                                    release(instance, names);
                                    release(instance, subject);
                                    return Err(raise_builtin(
                                        instance,
                                        "TypeError",
                                        &format!(
                                            "{class_name}() accepts 0 positional sub-patterns ({oparg} given)"
                                        ),
                                    ));
                                }
                                if oparg > 1 {
                                    release(instance, names);
                                    release(instance, subject);
                                    return Err(raise_builtin(
                                        instance,
                                        "TypeError",
                                        &format!(
                                            "{class_name}() accepts 1 positional sub-pattern ({oparg} given)"
                                        ),
                                    ));
                                }
                                // MATCH_SELF：单个位置子模式＝subject 自己 ✓
                                values.push(instance.retain(subject));
                                self_matched = true;
                            }
                        }
                    }
                    // 取属性：位置在前、关键字在后（参照的 tuple 顺序 ✓）。
                    let mut missing = false;
                    for name in &positional_names {
                        match crate::executor::attribute_optional(instance, subject, name) {
                            Ok(Some(raw)) => values.push(raw),
                            Ok(None) => {
                                missing = true;
                                break;
                            }
                            Err(error) => {
                                for value in values {
                                    release(instance, value);
                                }
                                release(instance, names);
                                release(instance, subject);
                                return Err(error);
                            }
                        }
                    }
                    if !missing {
                        for name in &keywords {
                            if !self_matched && positional_names.iter().any(|used| used == name) {
                                for value in values {
                                    release(instance, value);
                                }
                                release(instance, names);
                                release(instance, subject);
                                return Err(raise_builtin(
                                    instance,
                                    "TypeError",
                                    &format!(
                                        "{class_name}() got multiple sub-patterns for attribute '{name}'"
                                    ),
                                ));
                            }
                            match crate::executor::attribute_optional(instance, subject, name) {
                                Ok(Some(raw)) => values.push(raw),
                                Ok(None) => {
                                    missing = true;
                                    break;
                                }
                                Err(error) => {
                                    for value in values {
                                        release(instance, value);
                                    }
                                    release(instance, names);
                                    release(instance, subject);
                                    return Err(error);
                                }
                            }
                        }
                    }
                    release(instance, names);
                    release(instance, subject);
                    if missing {
                        for value in values {
                            release(instance, value);
                        }
                        let none = instance.singletons().none();
                        push(instance, frame.get(), none)?;
                        return Ok(Step::Continue);
                    }
                    let tuple = instance.new_tuple(values);
                    frame.get().push(tuple)?;
                }
        _ => unreachable!("step_match 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}

/// **`_Py_TPFLAGS_MATCH_SELF` 的等价面**（CPython 3.14 实测）：这些内建类型（含 Python 子类）
/// 允许**一个**位置子模式，且直接匹配 subject **自己** ✓。
///
/// 来源：参照行为实测 —— `case str(s)`／`case int(i)`／`case list(v)` 绑定整个 subject ✓；
/// `complex`／`range`／`memoryview`／`type`／`object`／异常族**不**允许（报 0 positional ✓）。
fn match_self_class(instance: &Instance, class: NonNull<TypeObject>) -> bool {
    [
        "int", "str", "float", "bytes", "bytearray", "list", "tuple", "set", "frozenset", "dict",
    ]
    .iter()
    .any(|name| {
        instance
            .type_named(name)
            .is_some_and(|base| instance.is_subtype(class, base))
    })
}
