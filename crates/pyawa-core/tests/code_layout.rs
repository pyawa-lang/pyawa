//! **编译产物逐条反汇编**（第 294 轮加的工具）：把一段源码（环境变量 `PYAWA_LAYOUT_SOURCE`）
//! 编出来逐条打印，含**嵌套代码对象**与**跳转落点**。
//!
//! 为什么要有它：`ROUNDS.md` 第 293 轮定位 `P3-20`（嵌套 `try` 截断外层块）时，光看源码和
//! 运行结果都推不出病灶 —— 把产物摊开才看见"处理块那条路把**作用域收尾**也发了一份"。
//! 本仓库原先没有这个口子。
//!
//! 用法（不设环境变量时**什么也不做**，所以它是干净的常驻工具）：
//! ```text
//! PYAWA_LAYOUT_SOURCE=$'try:\n    x = 1\nexcept ValueError:\n    x = 2\nprint(x)' \
//!     cargo test -p pyawa-core --test code_layout -- --nocapture
//! ```
use pyawa_core::compile::{compile, CheckTier, CompiledUnit, Constant, Mode};
use pyawa_core::decode::Decoder;
use pyawa_core::opcode;

fn dump(unit: &CompiledUnit, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{indent}== {} (qualname={})", unit.name, unit.qualname);
    let mut decoder = Decoder::new(&unit.code);
    while let Ok(Some(instruction)) = decoder.next_instruction() {
        let name = opcode::opname(u16::from(instruction.opcode)).unwrap_or("?");
        let target = instruction
            .jump_target()
            .map(|target| format!("→ {target}"))
            .unwrap_or_default();
        println!(
            "{indent}{:4}  {name:26} arg={:<5} {target}",
            instruction.offset, instruction.oparg
        );
    }
    println!("{indent}-- 常量表（{} 条）", unit.constants.len());
    for (index, constant) in unit.constants.iter().enumerate() {
        let text = match constant {
            Constant::Code(nested) => format!("Code({})", nested.qualname),
            other => format!("{other:?}"),
        };
        let text = if text.len() > 70 { format!("{}…", &text[..70]) } else { text };
        println!("{indent}   [{index}] {text}");
    }
    for constant in &unit.constants {
        if let Constant::Code(nested) = constant {
            dump(nested, depth + 1);
        }
    }
}

#[test]
fn dump_code_layout() {
    let source = std::env::var("PYAWA_LAYOUT_SOURCE").unwrap_or_default();
    if source.is_empty() {
        return;
    }
    let unit = compile(&source, "<s>", Mode::PurePython, CheckTier::Shallow, 0).expect("编译");
    dump(&unit, 0);
}
