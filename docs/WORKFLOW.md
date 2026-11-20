# 工作流（2026-11-21 起）

## 当前模型：main + 短命分支 + spike

```
main                 唯一长期线：受保护、PR-only、squash-only、tag 在这里
feat/<topic>         从 main 切；小 PR；合并后删除
spike/<topic>        从 main 切；**永不合并**；报告进 docs/rounds/spikes；分支可删
archive/pre-alpha-dev   alpha 前的 dev 尖端（cea7e306）只读参考
```

- 任何改动必须走 `feat/*` → PR → 必需检查全绿 → squash 进 `main`；
- `main` 禁止直接 push、禁止 force-push；PR 合并后删分支；
- 发布 = `main` 上 annotated tag + Release notes；
- `spike` 只产出报告，**任何情况下不合并**；报告模板见 `docs/rounds/spikes/`。

## PR 门槛（与规则集一致）

必需检查：`快速闸门`、`Web（WASM）`、`Windows 最小构建`、`完整闸门`、
`脚本可移植性（ubuntu/windows）`、`脚本可移植性（Linux×Windows 比对）`。
代码改动本地须 `check.py` 20/20 ＋ `slowcheck --release` 十二项全绿；纯文档 `check.py` 即可。

## 决策门槛

- 方向性变更先写 `docs/DECISIONS.md` 的 ADR（proposed → accepted / rejected / superseded）；
- 规范文档只在 ADR accepted 后修改；探索只进 `docs/rounds/spikes/`；
- 未定方向不写进承诺、不合并代码。

## 以后需要时再增量加（不重写历史）

| 触发 | 增量 |
|---|---|
| 需要“下版本集成分支” | 从 `main` 建 `develop`；`feat/*` PR 目标改为 `develop` |
| 发布稳定化 | 从 `main`/`develop` 切 `release/<version>`，只收 bugfix，合 `main`＋回 `develop` |
| 维护已发布版本 | 从 tag 切 `hotfix/<version>`，修完合 `main`＋回 `develop` |
| 多版本并行/LTS | 多个 `release/*` 与维护分支 |

分支模型按“发布需要”扩展，而不是按代码行数或单人现状固化。
