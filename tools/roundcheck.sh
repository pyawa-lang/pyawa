#!/usr/bin/env bash
# **整轮验收（并行版）**：慢档十项闸门 ＋ 官方全量 survey **同时**跑（2026-11-09 用户批准提速）。
#
# 用法：tools/roundcheck.sh                # release；survey 默认 --jobs 4（避免和闸门抢核）
#       tools/roundcheck.sh --jobs 8       # 给 survey 更多并发（机器空闲时）
#       tools/roundcheck.sh --gate-only    # 只跑闸门（survey 另跑）
#
# 判定：两者**各自退出码**都必须 0；任一非 0 ⇒ 本脚本非 0（分别打印尾部日志）。
# 注意：闸门里有时间敏感项（stability/heap）；若并行时出现抖动，用 --jobs 2 或退回串行
#       （`tools/slowcheck.sh --release && python3 tools/libtests_survey.py --release`）。
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="$PWD/target/cargo-home"
mkdir -p "$CARGO_HOME"

JOBS=4
GATE_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --jobs) JOBS="" ;;  # 兼容：下一参数单独处理见下
    --gate-only) GATE_ONLY=1 ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) [ -z "$JOBS" ] && JOBS="$arg" ;;
  esac
done
[ -z "$JOBS" ] && JOBS=4

LOGDIR="target/roundcheck"
mkdir -p "$LOGDIR"
START=$(date +%s)

echo "roundcheck: 启动慢档闸门（后台）$( [ "$GATE_ONLY" = "1" ] && echo '' || echo "＋ survey --jobs $JOBS（后台）")"
( tools/slowcheck.sh --release >"$LOGDIR/gate.log" 2>&1 ) &
GATE_PID=$!
SURVEY_PID=""
if [ "$GATE_ONLY" = "0" ]; then
  ( python3 tools/libtests_survey.py --release --jobs "$JOBS" >"$LOGDIR/survey.log" 2>&1 ) &
  SURVEY_PID=$!
fi

wait "$GATE_PID"; GATE_RC=$?
SURVEY_RC=0
if [ -n "$SURVEY_PID" ]; then wait "$SURVEY_PID"; SURVEY_RC=$?; fi
END=$(date +%s)

echo "--- 闸门（$(wc -l <"$LOGDIR/gate.log") 行日志）---"
tail -3 "$LOGDIR/gate.log"
[ "$GATE_RC" = "0" ] || tail -20 "$LOGDIR/gate.log"
if [ -n "$SURVEY_PID" ]; then
  echo "--- survey ---"
  tail -4 "$LOGDIR/survey.log"
  [ "$SURVEY_RC" = "0" ] || tail -20 "$LOGDIR/survey.log"
fi
echo "roundcheck: $([ "$GATE_RC" = "0" ] && [ "$SURVEY_RC" = "0" ] && echo 'OK ✓' || echo '红 ✗')  墙钟 $((END - START))s（闸门 rc=$GATE_RC，survey rc=$SURVEY_RC）"
[ "$GATE_RC" = "0" ] && [ "$SURVEY_RC" = "0" ]
