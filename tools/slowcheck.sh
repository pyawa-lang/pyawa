#!/usr/bin/env bash
# **慢档闸门**（`round-rule.md` §3）：十项 `&&` 链收成一条命令，并**报墙钟** ✓。
#
# 用法：tools/slowcheck.sh          # 全跑；每项报耗时；**按退出码**判红绿（不 grep 日志 ✓）
#       tools/slowcheck.sh --quiet  # 只报最后一行汇总
#
# 为什么要有它 ✓：这十项是每笔提交前的必跑项 ✗，先前每次手打（易漏项、易把 `&&` 写成并列 ✗）。
# 判据只认**退出码** ✓：任一项非 0 ⇒ 立刻停并打印该项日志尾部 ✓（`run-rule.md` §2「开工先定验收」✓）。
set -uo pipefail
cd "$(dirname "$0")/.."

QUIET=0
[ "${1:-}" = "--quiet" ] && QUIET=1
LOGDIR="target/slowcheck"
mkdir -p "$LOGDIR"
START=$(date +%s)
FAILED=0
STEP_NO=0

step() {
  STEP_NO=$((STEP_NO + 1))
  local name="$1"; shift
  local begin end
  begin=$(date +%s)
  if "$@" >"$LOGDIR/$STEP_NO.log" 2>&1; then
    end=$(date +%s)
    [ "$QUIET" = "0" ] && printf '%2d/10 ✓ %-58s %3ds\n' "$STEP_NO" "$name" "$((end - begin))"
    return 0
  fi
  end=$(date +%s)
  printf '%2d/10 ✗ %-58s %3ds  （日志 %s）\n' "$STEP_NO" "$name" "$((end - begin))" "$LOGDIR/$STEP_NO.log"
  tail -20 "$LOGDIR/$STEP_NO.log"
  FAILED=1
  return 1
}

# ① 编译（0 警告） ✗ ② 逐字节 ✗ ③ 全量测试（含关键护栏） ✗ ④～⑥ CI 三件 ✗ ⑦⑧ 语料下限与夹具 ✗ ⑨⑩ 对拍（普通 ＋ 悬垂哨兵 ✓）
step "cargo check --workspace --all-targets（0 警告）" \
  bash -c 'out=$(cargo check --workspace --all-targets 2>&1); n=$(printf "%s" "$out" | grep -cE "^warning"); printf "%s\n" "$out"; [ "$n" = "0" ] || { echo "警告数=$n ✗"; exit 1; }' || true
if [ "$FAILED" = "1" ]; then
  echo "slowcheck: 红 ✗（第 $STEP_NO 项：见上）  墙钟 $(( $(date +%s) - START ))s"
  exit "$STEP_NO"
fi
step "cargo test -p pyawa-core --test compile（逐字节）" cargo test -p pyawa-core --test compile || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "cargo test --workspace" cargo test --workspace || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "python3 tests/ci/check.py" python3 tests/ci/check.py || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "python3 tests/ci/stability.py" python3 tests/ci/stability.py || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "python3 tests/ci/heap_and_concurrency.py" python3 tests/ci/heap_and_concurrency.py || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "python3 tools/check_fixture_cases.py" python3 tools/check_fixture_cases.py || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "python3 tools/check_corpus_floor.py" python3 tools/check_corpus_floor.py || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "cargo test -p pyawa-abi --test conformance" cargo test -p pyawa-abi --test conformance || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi
step "PYAWA_DANGLING=1 cargo test -p pyawa-abi --test conformance" \
  env PYAWA_DANGLING=1 cargo test -p pyawa-abi --test conformance || true
if [ "$FAILED" = "1" ]; then echo "slowcheck: 红 ✗（第 $STEP_NO 项）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP_NO"; fi

echo "slowcheck: OK ✓（十项全绿）  墙钟 $(( $(date +%s) - START ))s"
