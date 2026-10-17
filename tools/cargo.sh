#!/usr/bin/env bash
# **带可写 `CARGO_HOME` 的 cargo**（第 561 轮实测 ✓）。
#
# 为什么必须有它 ✓：默认 `CARGO_HOME=~/.cargo` 在**工作区之外** ✗ ⇒ DSH 文件沙箱（`workspace-write` ✓）
# 只允许写工作区内 ⇒ 拉取/解包 crate 时报 **`Read-only file system (os error 30)`** ✗
# （`cargo add --dry-run` 只读元数据 ⇒ 会**假装**通路是通的 ✗ —— 我先前就被它误导过 ✓）。
# 把 `CARGO_HOME` 指到 `target/cargo-home`（工作区内 ✓）后：`regex v1.13.1` ＋ 两个依赖
# **拉取 ＋ 编译 15 s** ✓✓ ⇒ **依赖优先**（用户 2026-10-07 口径 ✓）这条路是通的 ✓。
#
# 用法：tools/cargo.sh build -p pyawa-core ／ tools/cargo.sh add regex -p pyawa-core ／ …
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="$PWD/target/cargo-home"
mkdir -p "$CARGO_HOME"
exec cargo "$@"
