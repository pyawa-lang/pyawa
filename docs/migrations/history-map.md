# dev 历史去向图（alpha 迁移）

本文回答“alpha 之前的 dev 去哪了”与“alpha 之后的 dev 怎么处理”。它是 `docs/migrations/alpha-dev-adoption.md` 的前置说明。

## 结论

- **alpha 之前的 dev 已在 `main` 的历史中**：m1–m6 与 alpha 的合并提交都把当时的 dev 尖端作为第二父提交；无需重放、无需新分支。
- **alpha 之后的 dev**（当前 `dev` 的 88 个提交）是**未合并的来源库**：按台账逐组“选择性采用”进 `main`；采用完成后删除 `dev`，**不建 `archive/alpha-dev`**。
- **显式入口**：`archive/pre-alpha-dev` 指向 `cea7e306`（alpha 时刻的 dev 尖端）；`dev` 在迁移完成后删除。

## 结构

```
dev:  Initial … ─ 8c0a34a ─(m1 合并)… ─ cea7e306 ─(alpha 合并)─ [88 个 alpha 后提交] ─ fe23ed78
                       ↑                     ↑
main: Initial 6dcf5156 ─ m1 ─ m2 ─ … ─ m6 ─ alpha(a907cc96, tag v0.1.0-alpha.1)
                                                        └─ feat/*（采用结果）──→ 后续 tag
```

## 已核验事实（2026-11-20）

- `v0.1.0-alpha.1` = `a907cc96`，第二父提交 = `cea7e306`（alpha 时刻 dev 尖端）。
- `git rev-list dev ^main | wc -l` = **88**（恰为 alpha 后提交数）⇒ alpha 之前全部 dev 提交都是 `main` 的祖先。
- `git merge-base main dev` = `cea7e306`。
- `dev..main` 的 7 条是 `main` 的阶段合并提交（m1–m6、alpha），只属于 `main`。
- 安全网：迁移前导出 `target/archive/dev-full.bundle`（`git bundle create … --all`，20 MB，不进版本库）。

## 引用/保留

| 名称 | 指向 | 说明 |
|---|---|---|
| `archive/pre-alpha-dev` | `cea7e306` | alpha 前的 dev 尖端（保留；只读参考） |
| `dev` | `fe23ed78` | alpha 后 88 个提交；迁移期来源；采用完成后删除 |
| `target/archive/dev-full.bundle` | 全历史 | 删除前的本地安全网（非分支、非发布物） |

## 规则

1. 不重写 `main` 历史、不移动既有 tag（m1–m6、`v0.1.0-alpha.1`）；
2. 采用一律从 `main` 切 `feat/*`，逐组 PR + CI + squash；
3. 未采用代码随 `dev` 删除而失效；只保留报告与台账；
4. 台账是所有“采用/改造/仅报告/丢弃”裁决的唯一依据。
