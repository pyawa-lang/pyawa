# Pyawa

用 Rust 从零实现的、语义对标 **CPython 3.14** 的 Python 方言。
核心是**像 Lua 一样易嵌入**：宿主直接实现沙箱接口、脚本不可信且不可逃逸、纯 Python 库逐字可用。
另提供**程序级可选的安全谱**（渐进类型，非强制）。**目标不是更快的 Python。**

当前状态：**M0 已成立**——文档集 12 份全部写出（状态列见 `docs/SPEC-INDEX.md` §1），M0 的两条判据
（`CX-1` 编号零悬空＋重复定义、`CX-2` 文档集状态一致）由 `tests/ci/check.py` 承载，当前 **7/7 绿**。
规格内仍有 **9 处已声明的「尚未写出」小节**（见各规格末节），这是 `v0` 的既定含义，不阻塞 M0。
**M1 已开始，但按 `docs/PLAN-milestones.md` §6 的判据尚未成立**：对象模型骨架与循环回收已落地
（清单见 `crates/pyawa-core/src/lib.rs`），而 M1 本体（实例生命周期、字节码 VM、C ABI）尚未接线。
先动骨架是**有意的例外**，**不代表 M1 的前置条件已满足**。

## 文档（`docs/`，全部平铺）

**冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）。
跨文档引用**只写编号、禁止重述内容**，规则见 `SPEC-INDEX.md` §2。

**文档清单（文件、范围、ID 前缀、状态）唯一出处为 `docs/SPEC-INDEX.md` §1**，本文件不重述。
本文件只报完成度：**共 12 份，已写 12 份 / 待写 0 份**。

## 目录

```
AGENTS.md      给 AI agent 的边界（每次任务都守）＋ 细则索引
agents-rules/  不常发生的细则：commit-rule.md / branch-rule.md / docs-rule.md
docs/          文档集（12 份，全部平铺：引用以文件名 + 编号为准）
crates/        Rust 工作区：pyawa-core / capabilities / abi / stdlib / runtime
tests/         conformance/（与 CPython 对拍）· ci/（不变量静态检查）
tools/         LSP、调试器、profiler（M6 后）
Lib/           CPython 3.14 纯 Python 标准库，逐字同步、禁止本地修改（M3 起引入）
```

## 分支与合并约定

- **`main` 是主干，只接受合并**：改动先提交在工作分支 `dev`（需要并行时再从 `dev` 开 `dev/<主题>`）
- **合并一律 `--no-ff`，必须留下合并提交**——合并提交是"这批改动是什么、何时进来"的唯一痕迹；
  快进合并会让工作分支白建，也让 `main` 的历史失去阶段边界。仓库已设 `merge.ff = false`，
  因此 `git merge dev` 默认即留痕
- 合并提交的说明写清这批改动的**范围与落点**（工作分支上的提交号），便于日后定位

```sh
git checkout main && git merge --no-ff dev -m "Merge branch 'dev': <这批改动是什么>"
git checkout dev
```

## 架构不变量

四条架构不变量**唯一出处为 `docs/DESIGN.md` §3**（不变量 1–4）：违反其中任何一条都需要推翻重来。
本文件不重述其内容。

## 当前未决

未决项按"什么时候必须定"分档，**唯一出处为 `docs/DESIGN.md` §13**。
原先唯一卡开工的一条（`§13-10` 兼容性验收标准）**已落地为 `PLAN-milestones.md`**，
因此**目前没有卡开工的未决项**；剩余为 B 档（公开第一个 ABI 之前拍板）与 C 档（可边做边定）。
