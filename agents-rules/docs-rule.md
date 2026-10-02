# 文档与规格

> 写文档、写规格时才查阅；每次任务都要守的边界见 `AGENTS.md`。

## 1. 一个事实只有一个出处

| 内容 | 去处 |
|---|---|
| 提交纪律 | `agents-rules/commit-rule.md` |
| 分支与历史 | `agents-rules/branch-rule.md`、`README.md`「分支与合并约定」 |
| 技术决策（选了什么） | `docs/REQUIREMENTS.md` |
| 架构论证与未决项汇总 | `docs/DESIGN.md` §13 |
| 执行细则与硬约束编号 | `docs/SPEC-*.md`；跨文件引用只写编号（`docs/SPEC-INDEX.md` §2） |
| 某个 crate 的职责与约束靶子 | 该 crate 的 `README.md` |
| 项目结构 | `README.md` |
| 文档清单、完成度 | `docs/SPEC-INDEX.md` §1 |
| 这次改动是什么 | commit 说明 |

## 2. 完成度口径

- 文档集清单与完成度以 `docs/SPEC-INDEX.md` §1 为准（共 **12 份**），如实写"已写 N 份 / 待写 M 份"
- 未实现的东西标"占位"或"未引入"，不要写成已就位
- 阶段完成的判据见 `docs/DESIGN.md` §12

## 3. 写规格时

- 固定小节、ID 前缀与引用规则见 `docs/SPEC-INDEX.md` §5 与 §2——照它执行，**不要另立格式**
- 每份规格的"未决"一节只引用 `DESIGN.md` §13 的编号，**禁止**私自决定
- **不追求一次性写全**：长文档一次一份，写完交人类过一遍再继续
- 例：`pyawa-core` 是否依赖 `pyawa-capabilities`，取决于 `docs/SPEC-capabilities.md`（`CP-`）
  里 vtable 的形状——在它写出之前接线，等于替那份规格做主
