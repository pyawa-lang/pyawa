//! `.pyi` 导出（`AB-53`／`AB-54`）：**单向投影，不在依赖链上**。
//!
//! - **`AB-31`**：签名的**权威在运行时注册**（注册时内联提供），**运行期不读 fs**；
//!   `.pyi` 是**导出产物**，不是运行期输入。
//! - **`AB-53`**：`.pyi` 由**注册信息**遍历导出；导出是单向的——运行期与进程内检查器
//!   **禁止**依赖 `.pyi` 文件存在（沙箱下可能没有 fs），也**禁止**把导出物当第二个签名源。
//! - **`AB-54`**：进程内检查器读注册信息的**内存投影**（`compile_with_host_functions`）、
//!   运行期 `inspect` 读注册信息本身（`__text_signature__` 由 `pa_sig` 投影）；本模块的
//!   `.pyi` 是给**进程外工具**（LSP／独立 check／宿主 stub）的同源投影，各投影禁止不一致。
//!
//! 本模块是 **Rust 侧** API：它不是 C 导出（`AB-6` 管的是 C ABI 那条清单），
//! 由 Pyawa 自己的工具（CLI／REPL 一侧）调用，把宿主注册的东西导出成 `.pyi`。

use crate::host::param_flags;
use crate::pa_state;

/// 一条注册记录（函数或类型）。
#[derive(Clone, Debug)]
pub struct Registration {
    /// 注册名。
    pub name: String,
    /// 是类型（`pa_newtype`）还是函数（`pa_register`）。
    pub is_type: bool,
    /// 签名（注册时拷贝下来的那一份，`AB-31`）。
    pub signature: crate::host::Signature,
    /// 类型专有：是否为不可继承（`AB-37` 的 `PA_TYPE_FINAL` 反向位）。
    pub is_final: bool,
    /// 类型专有：是否另行挂载实例字典（`AB-37`／`OM-14`）。
    pub has_instance_dict: bool,
}

/// 把注册账本导出成 `.pyi` 片段（`AB-53`：遍历而成；`AB-54`：与运行期同一份数据）。
pub fn pyi(state: *mut pa_state) -> String {
    // SAFETY: 调用方保证 state 是 pa_create 交回且尚未销毁的指针。
    let Some(state) = (unsafe { state.as_ref() }) else {
        return String::new();
    };
    let mut out = String::from("# 由 Pyawa 的注册信息导出（AB-53／AB-54）；禁止手改。\n\n");
    for entry in &state.registrations {
        out.push_str(&render(entry));
        out.push('\n');
    }
    out
}

/// 把一条注册记录渲染成 `.pyi` 里的声明。
fn render(entry: &Registration) -> String {
    let mut lines = String::new();
    if entry.is_type {
        let base = if entry.is_final {
            "  # PA_TYPE_FINAL：不可继承（AB-37）"
        } else {
            "  # 默认可被继承（AB-37）"
        };
        lines.push_str(&format!("class {}(object):{}\n", entry.name, base));
        if entry.has_instance_dict {
            lines.push_str("    __dict__: dict[str, object]\n");
        }
        lines.push_str("    def __init__(self");
        for parameter in &entry.signature.params {
            lines.push_str(", ");
            lines.push_str(&render_parameter(parameter));
        }
        lines.push_str(") -> None: ...\n");
        return lines;
    }
    lines.push_str(&format!("def {}(", entry.name));
    let mut first = true;
    let mut keyword_only = false;
    for parameter in &entry.signature.params {
        let flags = parameter.flags;
        if flags & param_flags::PA_PARAM_KEYWORD_ONLY != 0 && !keyword_only {
            keyword_only = true;
            if !first {
                lines.push_str(", ");
            }
            lines.push('*');
            first = false;
        }
        if flags & param_flags::PA_PARAM_VARARGS != 0 {
            if !first {
                lines.push_str(", ");
            }
            lines.push('*');
            lines.push_str(parameter.name.as_deref().unwrap_or("args"));
            first = false;
            continue;
        }
        if flags & param_flags::PA_PARAM_VARKW != 0 {
            if !first {
                lines.push_str(", ");
            }
            lines.push_str("**");
            lines.push_str(parameter.name.as_deref().unwrap_or("kwargs"));
            first = false;
            continue;
        }
        if !first {
            lines.push_str(", ");
        }
        lines.push_str(&render_parameter(parameter));
        first = false;
    }
    lines.push_str(")");
    lines.push_str(&format!(
        " -> {}: ...\n",
        entry.signature.ret_expr.as_deref().unwrap_or("None")
    ));
    lines
}

/// 单个参数的渲染（名字、注解、默认值）。
fn render_parameter(parameter: &crate::host::HostParam) -> String {
    let name = parameter.name.as_deref().unwrap_or("arg");
    let annotation = parameter.type_expr.as_deref();
    let mut text = name.to_owned();
    if let Some(annotation) = annotation {
        text.push_str(": ");
        text.push_str(annotation);
    }
    if parameter.flags & param_flags::PA_PARAM_HAS_DEFAULT != 0 {
        text.push_str(" = ...");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(is_type: bool) -> Registration {
        Registration {
            name: if is_type { "Widget".to_owned() } else { "add".to_owned() },
            is_type,
            is_final: is_type,
            has_instance_dict: true,
            signature: crate::host::Signature {
                flags: 0,
                ret_expr: Some("int".to_owned()),
                params: vec![
                    crate::host::HostParam {
                        name: Some("left".to_owned()),
                        type_expr: Some("int".to_owned()),
                        flags: param_flags::PA_PARAM_POSITIONAL,
                        default: None,
                    },
                    crate::host::HostParam {
                        name: Some("right".to_owned()),
                        type_expr: None,
                        flags: param_flags::PA_PARAM_POSITIONAL | param_flags::PA_PARAM_HAS_DEFAULT,
                        default: None,
                    },
                ],
            },
        }
    }

    #[test]
    fn functions_render_with_annotations_and_defaults() {
        let text = render(&sample(false));
        assert_eq!(text, "def add(left: int, right = ...) -> int: ...\n");
    }

    #[test]
    fn types_render_with_final_and_instance_dict() {
        let text = render(&sample(true));
        assert!(text.starts_with("class Widget(object):  # PA_TYPE_FINAL"), "实际：{text}");
        assert!(text.contains("__dict__: dict[str, object]"), "实际：{text}");
        assert!(
            text.contains("def __init__(self, left: int, right = ...) -> None: ..."),
            "实际：{text}"
        );
    }
}
