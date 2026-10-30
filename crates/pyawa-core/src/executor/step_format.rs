//! `executor` 的 `step_format` 域：f-string 的格式化三步与栈操作。
//!
//! 从 `execute` 的分派臂**原样搬来**；由 `executor.rs` 的分派按 opcode 组调用。

use super::*;

pub(super) fn step_format<'a>(instance: &'a Instance, frame: &Owned<'a, Frame>, oparg: usize, opcode_number: u8, name: &str) -> Result<Step<'a>, ExecError> {
    match name {
                "FORMAT_SIMPLE" => {
                    // 净 0：TOS 换成它的 `str()`（3.14 把旧的 `FORMAT_VALUE` 拆成了三条）
                    let value = frame.get().pop()?;
                    // P3-38 S2a-2 批 2 最小直通：**恰 `str` 的 WTF-8 表示**（含孤立代理）
                    // `str(x) is x`（参照实测 ✓）⇒ 原样压回去，避免经 `object_str`（String 装不下代理）。
                    if instance.is_str_object(value) && instance.text_of(value).is_none() {
                        push(instance, frame.get(), value)?;
                        return Ok(Step::Continue);
                    }
                    // **直接走 `object_str`** ✓（第 210 轮修正 ✗）：它自己就是"**槽位优先** ＋ 覆写通道 ＋
                    // 兜底" ✓ —— 与参照的默认 `object.__format__`（= `str(self)` ✓）同序 ✓。
                    // 先前这里"**先查 `__str__` 覆写**" ✗ ⇒ 一旦 `object.__str__` 存在（本轮起 ✓），
                    // 每个 MRO 都会命中它 ⇒ 把 `str`／`int` 自带的 `str` 槽带跑 ✗（实测 `f"{x}"` 给 `'1'` ✗）。
                    let text = instance.object_str(value)?;
                    release(instance, value);
                    push(instance, frame.get(), instance.new_str(&text))?;
                }
                "CONVERT_VALUE" => {
                    // 净 0：`!s`／`!r`／`!a`（实测 oparg 1／2／3）
                    let value = frame.get().pop()?;
                    // P3-38 S2a-2 批 2 最小直通：`!s` 对**恰 str 的 WTF-8 表示**同样是身份转换
                    //（参照 `f"{s!s}" is s` ✓）⇒ 原样压回，避免 `object_str` 的 String 装不下代理。
                    if oparg == 1 && instance.is_str_object(value) && instance.text_of(value).is_none() {
                        push(instance, frame.get(), value)?;
                        return Ok(Step::Continue);
                    }
                    // `!s`／`!r`／`!a`（实测 oparg 1／2／3），都走 `OM-11` 的槽位
                    let text = match oparg {
                        // **同 `FORMAT_SIMPLE`** ✓（第 210 轮修正 ✗）。
                        1 => instance.object_str(value)?,
                        2 => match dunder_text(instance, value, "__repr__", opcode_number)? {
                            Some(text) => text,
                            None => instance.object_repr(value)?,
                        },
                        3 => {
                            let base = match dunder_text(instance, value, "__repr__", opcode_number)? {
                                Some(text) => text,
                                None => instance.object_repr(value)?,
                            };
                            escape_non_ascii(&base)
                        }
                        _ => {
                            release(instance, value);
                            return Err(ExecError::Unsupported {
                                opcode: opcode_number,
                                what: "CONVERT_VALUE 的 oparg 只能是 1／2／3",
                            });
                        }
                    };
                    release(instance, value);
                    push(instance, frame.get(), instance.new_str(&text))?;
                }
                "FORMAT_WITH_SPEC" => {
                    // 实测净 −1：栈是 `[值, 规格]`（规格在 TOS）。
                    // 路线：① 类型字典里的 `__format__`（Python 级覆写优先）② 类型的 `format` 槽
                    // ③ 都不认 ⇒ 报错（消息**实测**：`unsupported format string passed to X.__format__`）。
                    let spec_object = frame.get().pop()?;
                    // SAFETY: spec_object 是刚出栈的存活对象。
                    let spec_type = unsafe { spec_object.as_ref() }.ty();
                    if spec_type != instance.singletons().str_type() {
                        release(instance, spec_object);
                        return Err(raise_builtin(
                            instance,
                            "TypeError",
                            "format spec must be a str",
                        ));
                    }
                    // SAFETY: 类型身份已确认。
                    let spec_text =
                        unsafe { &*spec_object.as_ptr().cast::<StrObject>() }.value().to_owned();
                    release(instance, spec_object);

                    let value = frame.get().pop()?;
                    // P3-38 S2a-2 批 2 最小直通：**空规格 + 恰 str 的 WTF-8 表示** ⇒ 身份返回
                    //（参照 `format(s) is s` ✓；带非空规格的完整语义留下批）。
                    if spec_text.is_empty()
                        && instance.is_str_object(value)
                        && instance.text_of(value).is_none()
                    {
                        frame.get().push(value)?;
                        return Ok(Step::Continue);
                    }
                    // 渲染整段**抽到 `format_value_with_spec`** ✓（第 722 轮：`str.format` 要调**同一处** ✓
                    // —— 先前这段内联逻辑只有 `FORMAT_WITH_SPEC` 能走 ✗）。
                    let formatted = format_value_with_spec(instance, value, &spec_text, opcode_number);
                    release(instance, value);
                    frame.get().push(formatted?)?;
                    return Ok(Step::Continue);
                }
                "SWAP" => {
                    // 参照实现：SWAP(i) 交换 TOS 与 TOS[-i]（净 0）
                    frame.get().swap_from_top(oparg)?;
                }
                "COPY" => {
                    // 参照实现：COPY(i) 把 TOS[-i] 复制一份压栈（+1）
                    let raw = frame.get().peek_from_top(oparg)?;
                    push(instance, frame.get(), raw)?;
                }
        _ => unreachable!("step_format 只接收本族的 opcode"),
    }
    Ok(Step::Continue)
}
