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
| `CX-2` | 已实现 | `T-CX-2`：§1 ↔ 根 `README.md` 计数 ② 任一 `README.md` 里的规格状态标注 |
| `CX-3` | 已实现 | `T-CX-3`：`static mut`／`thread_local` 扫描 |
| `CX-4` | 已实现 | `T-CX-4`：`std::fs`／`std::net`／libc／`#[cfg(target_os)]` 扫描 |
| `CX-5` | 未实现 | 等 `pyawa-capabilities` 接线；该 crate 目前只有骨架 |
| `CX-6` | 已实现 | `T-CX-5`：`Rc`／`Arc` 作对象引用的扫描 |
| `CX-7` | 已实现 | `T-CX-9`：位常量可求值且不与 `RESERVED_MASK` 相交；`RESERVED_MASK` 引用白名单 |
| `CX-8` | 未实现 | `Lib/` 在 M3 引入 |
| `CX-9` | 未实现 | 依赖编译管线（M2） |
| `CX-10` | 未实现 | 依赖能力接口接线 |
| `CX-11` | 未实现 | 依赖 `pyawa-abi` 落地 |
| `CX-12` | 已实现 | **不设 `check.py` 扫描**：由 Rust 侧 `T-OM-9` 承担（`cargo test --workspace` 即 CI，断言"除 `clear` 外无释放路径"） |
| `CX-13` | 未实现 | 需要 VM 初始化后才能断言 |
| `CX-17` | 已实现 | `T-CX-7`：每份已写规格都有「尚未写出」节 |
| `CX-18` | 已实现 | `T-CX-8`：`docs/*.md`、各 `README.md`、根 `Cargo.toml` 注释里反引号包裹的 `crates/….rs|json|toml` 路径存在性 |
| `CX-19` | 已实现 | `T-CX-10`：各前缀**已定义**编号从 1 连续到最大值（墓碑算定义），无未解释缺号；族含 `AB`／`BC`／…／`CX`、`T-` 变体与 `DESIGN.md` §13 的未决项编号 |
| `CX-20` | 已实现 | `T-CX-11`：各规格「未决」「尚未写出」节不得把**已决**的 `§13-N` 列为待定（该行或前 3 行内有"已决／原先／关闭／不再是"即放行） |
| `CX-21` | 已实现 | `T-CX-12`：**持引用字段必须被 `traverse`／`clear` 覆盖**（`OM-12`）；**Rust 侧承担**，含"新增字段漏项必须红"的注入用例（不设 `check.py` 扫描，同 `CX-12` 的先例） |

对应验收编号：`T-CX-1`…`T-CX-12` 的定义见 `CONSTRAINTS.md` §5；`T-OM-7`／`T-OM-8`／`T-CP-6`
与本目录同源，实现处也是本目录。

## C ABI 的 M1 验收（`T-AB-1`／`MS-21`）

`python3 tests/ci/t_ab_1.py [--release]`：数示例的行数（判据说 ≤50）→ `cargo build -p pyawa-abi`
→ `cc` **真编译真链接**（链 `staticlib`）→ **真运行**，断言退出码 0 且打印 `42`。
**缺 `cc` 即红**：不跳过、不弱化——判据要么被证明、要么不被证明。

| 验收 | 状态 | 落点 |
|---|---|---|
| `T-AB-1` | 已实现 | 示例 `examples/m1.c`（45 行 ≤ 50）＋ 本节的脚本；其中"执行一段脚本"由 `SPEC-c-abi.md` §15.3 的 `pa_exec_string`（`AB-60`）承担，"注入宿主函数"由 `pa_register`（`AB-24`／`AB-25`）承担 |

## 稳定性（`MS-25`）

`python3 tests/ci/stability.py [--runs N]`（默认 3，少于 3 直接拒绝）：连跑 `cargo test
--workspace --no-fail-fast`，比对**每个测试二进制**的计数与总通过数是否完全一致。

- 只比总数会漏掉"此消彼长"；只比"有没有 FAILED"会漏掉**信号中止**（cargo 对 `SIGABRT`
  不打 `FAILED`，表现为总数变少）
- 实现注意：cargo 把 `Running …` 写 **stderr**、把 `test result:` 写 **stdout** ⇒
  必须把 stderr 重定向进 stdout 才能保住"Running → 该二进制结果"的配对；
  `Doc-tests` 单独成组，否则它会覆盖前一个二进制的计数
- 当前基线：**70** 个测试二进制、**466** 项通过（第 211 轮实测）
