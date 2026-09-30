# Pyawa

用 Rust 从零实现的、语义对标 **CPython 3.14** 的 Python 方言。
核心是**像 Lua 一样易嵌入**：宿主直接实现沙箱接口、脚本不可信且不可逃逸、纯 Python 库逐字可用。
另提供**程序级可选的安全谱**（渐进类型，非强制）。**目标不是更快的 Python。**

当前状态：**M0 设计文档进行中，未完成**——`DESIGN.md` 已成文，但文档集 12 份中只写了 5 份、7 份空白（见下表状态列）。
**M1 未开始，且不具备开工条件**：`SPEC-object-model.md`／`SPEC-bytecode.md` 自标为"M1 前置规格"，7 份待写文档补齐前，M1 的前置条件不成立。

## 文档（`docs/`，全部平铺）

**冲突时的优先级**：`REQUIREMENTS.md`（决策）> `SPEC-*.md`（执行细则）> `DESIGN.md`（架构论证）。
跨文档引用**只写编号、禁止重述内容**，规则见 `SPEC-INDEX.md` §2。

文档集共 **12 份**（定义见 `SPEC-INDEX.md` §1）：**已写 5 份 / 待写 7 份**。

| 文件 | 范围 | ID 前缀 | 状态 |
|---|---|---|---|
| `docs/REQUIREMENTS.md` | 决策记录（唯一决策源） | — | v1.1 |
| `docs/DESIGN.md` | 架构论证与里程碑；未决项汇总在 §13 | — | v1 |
| `docs/SPEC-INDEX.md` | 文档集工作约定（不是规格） | — | v0 |
| `docs/SPEC-object-model.md` | 实例级内存、对象头、类型对象、引用计数、循环回收、弱引用、宿主对象 | `OM-` | v0 |
| `docs/SPEC-bytecode.md` | code object、指令集契约、编译管线、帧与执行、异常表、monitoring 事件点 | `BC-` | v0 |
| `docs/SPEC-imports-and-modes.md` | 模式开关、import 钩子、`.pyac` 格式与失效、重名检查 | `IM-` | 待写 |
| `docs/SPEC-capabilities.md` | 能力域接口契约、句柄生命周期、可否异步化 | `CP-` | 待写 |
| `docs/SPEC-c-abi.md` | 函数清单与预算、栈规则、错误码、宿主类型注册、签名元数据、版本策略 | `AB-` | 待写 |
| `docs/SPEC-type-system.md` | 相容关系、边界检查、归责、检查算法 | `TS-` | 待写 |
| `docs/SPEC-c-modules.md` | 113 个 C 模块的实现顺序与逐模块合约 | `CM-` | 待写 |
| `docs/PLAN-milestones.md` | 里程碑、验收、对拍 harness 定义 | `MS-` | 待写 |
| `docs/CONSTRAINTS.md` | 不变量与 CI 强制项清单 | `CX-` | 待写 |

## 目录

```
AGENTS.md      给 AI agent 的提交规则与协作纪律（主题行格式、提交前检查、分支与历史、完成度）
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

## 四条架构不变量

违反其中任何一条都需要推翻重来（`docs/DESIGN.md` §3）：

1. **不存在未注入的 I/O**——所有外部世界访问经能力层
2. **不存在全局可变状态**——所有状态挂在实例上（`lua_State` 模型）
3. **panic 不跨 FFI**——每个 C ABI 入口 `catch_unwind`
4. **跨线程的能力调用不碰 VM 对象**——只传裸数据与 OS 句柄

## 当前未决

**只有一条卡开工**：`docs/DESIGN.md` §13-10 兼容性验收标准——不定它，「语义兼容」不可证伪。
其余未决项分档见 `DESIGN.md` §13。
