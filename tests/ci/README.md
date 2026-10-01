# tests/ci — 不变量与 CI 强制项的检查实现

约束的**定义**（编号、出处、现在能否落地）唯一出处是 `docs/CONSTRAINTS.md`（`CX-`）；
本目录只负责**实现**检查，不重述约束内容。

## 怎么跑

```sh
python3 tests/ci/check.py           # 全部检查；有失败则退出码 1
python3 tests/ci/check.py --list    # 列出检查项与对应的 CX 编号
python3 tests/ci/selftest.py        # 自检：逐条注入违规，证明每项检查都会红（CX-15）
```

范围：`T-CX-3`／`T-CX-4`／`T-CX-5` 只扫各 crate 的 `src/`——它们约束的是随包发布的实现，
测试代码不算。**该范围的规范定义在 `docs/CONSTRAINTS.md` §3.1**；范围是约束定义的一部分，
不是实现细节，因此**不另领编号**。

## 状态账本

`T-CX-6` 会核对本表与 `CONSTRAINTS.md` §3 的"现在能否落地"列：标**不能**的条目**禁止**写成
"已实现"；标**能**的条目必须要么已实现、要么在此写明状态与理由（不许沉默）。

| 约束 | 状态 | 备注 |
|---|---|---|
| `CX-1` | 已实现 | `T-CX-1`：全库引用的编号扫描（未写规格的引用须标临时假设） |
| `CX-2` | 已实现 | `T-CX-2`：`SPEC-INDEX.md` §1 ↔ `README.md` 完成度 |
| `CX-3` | 已实现 | `T-CX-3`：`static mut`／`thread_local` 扫描 |
| `CX-4` | 已实现 | `T-CX-4`：`std::fs`／`std::net`／libc／`#[cfg(target_os)]` 扫描 |
| `CX-5` | 未实现 | 等 `pyawa-capabilities` 接线；该 crate 目前只有骨架 |
| `CX-6` | 已实现 | `T-CX-5`：`Rc`／`Arc` 作对象引用的扫描 |
| `CX-7` | 未实现 | 运行时有断言（`OM-7`），静态检查未做 |
| `CX-8` | 未实现 | `Lib/` 在 M3 引入 |
| `CX-9` | 未实现 | 依赖编译管线（M2） |
| `CX-10` | 未实现 | 依赖能力接口接线 |
| `CX-11` | 未实现 | 依赖 `pyawa-abi` 落地 |
| `CX-12` | 未实现 | 行为已由 `crates/pyawa-core/tests/` 覆盖，静态检查未做 |
| `CX-13` | 未实现 | 需要 VM 初始化后才能断言 |

对应验收编号：`T-CX-1`…`T-CX-6` 的定义见 `CONSTRAINTS.md` §5；`T-OM-7`／`T-OM-8`／`T-CP-6`
与本目录同源，实现处也是本目录。
