#!/usr/bin/env bash
# **整轮验收（并行版）**：慢档十项闸门 ＋ 官方全量 survey **同时**跑（2026-11-09 用户批准提速）。
#
# 用法：tools/roundcheck.sh                # release；survey 默认 --jobs 4（避免和闸门抢核）
#       tools/roundcheck.sh --jobs 8       # 给 survey 更多并发（机器空闲时）
#       tools/roundcheck.sh --gate-only    # 只跑闸门（survey 另跑）
#
# 判定：两者**各自退出码**都必须 0；任一非 0 ⇒ 本脚本非 0（分别打印尾部日志）。
#
# **默认串行（先闸门、后 survey）**：并行会让 `test_userstring` 这类"临界超时"文件在 120 s 预算上
# 被负载推过线（实测：并行 survey 68/78/247/4 vs 空闲 69/78/247/3 ⇒ 计数不可比 ✗）。
# 需要赶时间时可 `--parallel`（两类测量互相干扰，**超时/耗时类计数只作参考**）。
# 串行时把文档/侦察放在后台跑，实际墙钟 ≈ max(闸门 13m, 其他) + survey 10m。
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="$PWD/target/cargo-home"
mkdir -p "$CARGO_HOME"

JOBS=2
GATE_ONLY=0
PARALLEL=0
for arg in "$@"; do
  case "$arg" in
    --jobs) JOBS="" ;;  # 兼容：下一参数单独处理见下
    --gate-only) GATE_ONLY=1 ;;
    --parallel) PARALLEL=1 ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *) [ -z "$JOBS" ] && JOBS="$arg" ;;
  esac
done
[ -z "$JOBS" ] && JOBS=2

LOGDIR="target/roundcheck"
mkdir -p "$LOGDIR"
START=$(date +%s)
GATE_RC=0
SURVEY_RC=0
SURVEY_PID=""

if [ "$PARALLEL" = "1" ]; then
  echo "roundcheck: 并行模式（闸门＋survey 同时；超时类计数只作参考）"
  ( tools/slowcheck.sh --release >"$LOGDIR/gate.log" 2>&1 ) &
  GATE_PID=$!
  if [ "$GATE_ONLY" = "0" ]; then
    ( python3 tools/libtests_survey.py --release --jobs "$JOBS" >"$LOGDIR/survey.log" 2>&1 ) &
    SURVEY_PID=$!
  fi
  wait "$GATE_PID"; GATE_RC=$?
  if [ -n "$SURVEY_PID" ]; then wait "$SURVEY_PID"; SURVEY_RC=$?; fi
else
  echo "roundcheck: 串行模式（先闸门、后 survey；计数可比）"
  tools/slowcheck.sh --release >"$LOGDIR/gate.log" 2>&1
  GATE_RC=$?
  if [ "$GATE_ONLY" = "0" ] && [ "$GATE_RC" = "0" ]; then
    echo "roundcheck: 闸门绿 ✓，继续 survey --jobs $JOBS"
    python3 tools/libtests_survey.py --release --jobs "$JOBS" >"$LOGDIR/survey.log" 2>&1
    SURVEY_RC=$?
  fi
fi
END=$(date +%s)

echo "--- 闸门（$(wc -l <"$LOGDIR/gate.log") 行日志）---"
tail -3 "$LOGDIR/gate.log"
[ "$GATE_RC" = "0" ] || tail -20 "$LOGDIR/gate.log"
if [ "$GATE_ONLY" = "0" ] && [ -f "$LOGDIR/survey.log" ]; then
  echo "--- survey ---"
  tail -4 "$LOGDIR/survey.log"
  [ "$SURVEY_RC" = "0" ] || tail -20 "$LOGDIR/survey.log"
fi
echo "roundcheck: $([ "$GATE_RC" = "0" ] && [ "$SURVEY_RC" = "0" ] && echo 'OK ✓' || echo '红 ✗')  墙钟 $((END - START))s（闸门 rc=$GATE_RC，survey rc=$SURVEY_RC）"
[ "$GATE_RC" = "0" ] && [ "$SURVEY_RC" = "0" ]
