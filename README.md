# Pyawa

用 Rust 从零实现的、语义对标 **CPython 3.14** 的 Python 方言。
核心是**像 Lua 一样易嵌入**：宿主直接实现沙箱接口、脚本不可信且不可逃逸、纯 Python 库逐字可用。
另提供**程序级可选的安全谱**（渐进类型，非强制）。**目标不是更快的 Python。**

当前状态：**M0 设计文档进行中，未完成**——`DESIGN.md` 已成文，文档集 12 份中只写了 5 份、7 份空白
（状态列见 `docs/SPEC-INDEX.md` §1）。
**M1 未开始，且不具备开工条件**：`SPEC-object-model.md`／`SPEC-bytecode.md` 自标为"M1 前置规格"，7 份待写文档补齐前，M1 的前置条件不成立。

## 文档（`docs/`，全部平铺）

**冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）。
跨文档引用**只写编号、禁止重述内容**，规则见 `SPEC-INDEX.md` §2。

**文档清单（文件、范围、ID 前缀、状态）唯一出处为 `docs/SPEC-INDEX.md` §1**，本文件不重述。
本文件只报完成度：**共 12 份，已写 5 份 / 待写 7 份**。

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

**只有一条卡开工**：`docs/DESIGN.md` §13-10 兼容性验收标准——不定它，「语义兼容」不可证伪。
其余未决项分档见 `DESIGN.md` §13。
