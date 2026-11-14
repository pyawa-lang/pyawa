#!/usr/bin/env bash
# **分块闸门**（round 132）：本会话单条命令约 600 s 上限 ⇒ `tools/slowcheck.sh` 整体（800+ s）跑不完。
# 用法：`tools/gate_chunk.sh small|stability|heap|conformance`
#   small        步骤 0/1/2/4/7/8(+12)：release 构建、cargo check（0 警告）、compile 逐字节、check.py、夹具、语料下限、失败路径 GC 探针、宿主可用性探针
#   stability    步骤 5：`python3 tests/ci/stability.py --release`（cargo test --workspace 连跑 3 次比对计数）
#   heap         步骤 6：`python3 tests/ci/heap_and_concurrency.py --release`
#   conformance  步骤 9/10：conformance ×2（普通 + PYAWA_DANGLING=1）
# 全部输出同时写 `target/roundcheck/chunk-<mode>.log`；退出码非 0 表示该块红 ✗。
set -u
export CARGO_HOME="$PWD/target/cargo-home"
MODE="${1:?用法: tools/gate_chunk.sh small|stability|heap|conformance}"
LOGDIR="target/roundcheck"; mkdir -p "$LOGDIR"
LOG="$LOGDIR/chunk-$MODE.log"
START=$(date +%s)
run() { echo "== $*"; "$@"; }
case "$MODE" in
  small)
    { run cargo build --release -p pyawa-runtime --lib --bin pyawa
      out=$(cargo check --workspace --all-targets 2>&1); n=$(printf "%s" "$out" | grep -cE "^warning"); printf "%s\n" "$out"; [ "$n" = "0" ] || { echo "警告数=$n ✗"; exit 1; }
      run cargo test --release -p pyawa-core --test compile
      run python3 tests/ci/check.py
      run python3 tools/check_fixture_cases.py
      run python3 tools/check_corpus_floor.py
      run python3 tests/ci/failure_path_leaks.py --binary target/release/pyawa
      run python3 tests/ci/host_usability_probe.py --release
      run cargo test --release -p pyawa-abi --test protocol_oracle --test protocol_host_dispatch
    } >"$LOG" 2>&1
    RC=$?
    ;;
  stability) { python3 tests/ci/stability.py --release; } >"$LOG" 2>&1; RC=$? ;;
  heap) { python3 tests/ci/heap_and_concurrency.py --release; } >"$LOG" 2>&1; RC=$? ;;
  conformance)
    { cargo test --release -p pyawa-abi --test conformance || exit 1
      PYAWA_DANGLING=1 cargo test --release -p pyawa-abi --test conformance || exit 1
    } >"$LOG" 2>&1
    RC=$?
    ;;
  *) echo "未知块：$MODE ✗"; exit 2 ;;
esac
ELAPSED=$(( $(date +%s) - START ))
if [ "$RC" = "0" ]; then
  echo "gate-chunk[$MODE]: OK ✓  ${ELAPSED}s  （日志 $LOG）"
else
  echo "gate-chunk[$MODE]: 红 ✗  ${ELAPSED}s  （日志 $LOG）"
  tail -20 "$LOG"
fi
exit "$RC"
