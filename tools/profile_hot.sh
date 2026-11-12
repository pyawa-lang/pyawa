#!/usr/bin/env bash
# **热点剖析仪器**（优先 `perf`；无 `perf` 权限时退回 callgrind —— **退回会明说**，不静默降级 ✗）
#
# 背景：`tools/bench_hot.py` 实测 pyawa 比 CPython 3.14.4 慢 **135–226×** ✗；
#   本工具给出函数级热点，用于定位"每条指令都付的病态成本"。
#
# 2026-11-12 首次运行（callgrind，baseline 10 万次）已定位：
#   `Instance::type_named` 6.56% ＋ `str::traits` 5.92% ＋ `cmp` 5.71% ＋ `TypeObject::name` 1.75%
#   ＋ `memcmp/strncmp` ~1.4% ＋ `sip` hash 0.54% ≈ **20%（按名字查类型）**
#   ＋ **libc `getenv` 2.18%（未缓存的 diag flag）**
#
# 用法：
#   tools/profile_hot.sh                       # perf 采样，四档全跑（**release 二进制**，带符号）
#   tools/profile_hot.sh --iters 50000         # 更小更快（形状不变）
#   tools/profile_hot.sh --tier baseline       # 只跑 baseline 档（最干净）
#   tools/profile_hot.sh --callgrind           # 强制 callgrind（指令级、慢 ~50×）
#   tools/profile_hot.sh --bin <路径>          # 指定别的二进制（仍受"陈旧检查"约束）
#   tools/profile_hot.sh --allow-stale         # 明确允许拿陈旧二进制出数（**不推荐** ✗）
#
# 产出：`target/hotprofile.txt`（排序表）＋ `target/hot.data`（perf）或 `target/callgrind.out.*`
#
# **为什么默认 release（round 248 修 ✗）**：`tools/bench_hot.py` 量的是 release；debug 二进制的
# 成本形状不同（内联／检查全不消），更糟的是 `target/debug/pyawa` **不随 `cargo build` 自动更新**
# ⇒ 曾经拿 2026-11-08 的陈旧 debug 二进制"复剖析"，把 A2 之前的旧热点当成现状 ✗。现在默认
# release，并在出数前检查**源码是否比二进制新** ⇒ 陈旧就报错退出，不再静默出错误证据 ✗。
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ITERS=200000; TIER=""; TOP=25; BIN="$ROOT/target/release/pyawa"; MODE=auto; ALLOW_STALE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --iters) ITERS="$2"; shift 2 ;;
    --tier) TIER="$2"; shift 2 ;;
    --top) TOP="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    --callgrind) MODE=callgrind; shift ;;
    --allow-stale) ALLOW_STALE=1; shift ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done
[ -x "$BIN" ] || { echo "找不到 CLI：$BIN" >&2; exit 2; }
# **陈旧检查**（round 248）：源码比二进制新 ⇒ 拒绝对陈旧二进制出数（防止把旧热点当现状 ✗）。
if [ "$ALLOW_STALE" = "0" ]; then
  NEWER="$(find "$ROOT/crates" -name '*.rs' -newer "$BIN" -print -quit 2>/dev/null)"
  if [ -n "$NEWER" ]; then
    echo "  ✗ 二进制比源码旧（例：$NEWER）" >&2
    echo "    ⇒ 先 cargo build --release -p pyawa-runtime --bin pyawa；确要看陈旧数就加 --allow-stale ✗" >&2
    exit 2
  fi
fi

PROBE=$(mktemp /tmp/hotprobe.XXXX.py)
python3 "$ROOT/tools/bench_hot.py" --emit | sed "s/2000000/$ITERS/g" > "$PROBE"
if [ -n "$TIER" ]; then
  python3 - "$PROBE" "$TIER" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); tier = sys.argv[2]
lines = []
for line in p.read_text().splitlines():
    lines.append(("# " + line) if (line.startswith("measure(") and tier not in line) else line)
p.write_text("\n".join(lines) + "\n")
PY
fi
echo "  探针：$ITERS 次／档 ${TIER:+（只跑 $TIER）}"

OUT="$ROOT/target/hotprofile.txt"
PERF_DATA="$ROOT/target/hot.data"
CG_DATA="$ROOT/target/callgrind.out"

# ① 首选 perf（开销≈0）；无权限则明说并退回 callgrind
if [ "$MODE" != "callgrind" ] && command -v perf >/dev/null 2>&1 \
   && perf record -o "$PERF_DATA" -- true >/dev/null 2>&1; then
  echo "  [perf] 采样中（不需要 --callgrind）"
  perf record -g -o "$PERF_DATA" -- "$BIN" "$PROBE" >/dev/null 2>&1
  {
    echo "# hotprofile —— perf（$ITERS 次/档 ${TIER:+；$TIER}）$("$BIN" --version 2>/dev/null | head -1)"
    echo "# 读法：Overhead 最高的符号＝病态成本候选"
    perf report --stdio -i "$PERF_DATA" --no-children --sort dso,symbol --percent-limit 0.5 2>/dev/null \
      | grep -E '^ *[0-9]+\.[0-9]+%' | head -"$TOP"
  } > "$OUT"
  echo "  报告 → $OUT"; sed -n '1,18p' "$OUT" | cut -c1-120
  rm -f "$PROBE"; exit 0
fi

# ② 退回 callgrind —— 明确告知这是降级 ✗
VG="$(command -v valgrind || echo "$ROOT/target/valgrind/extract/usr/bin/valgrind")"
ANNOTATE="$(command -v callgrind_annotate || echo "$ROOT/target/valgrind/extract/usr/bin/callgrind_annotate")"
[ -x "$VG" ] || { echo "既没有 perf 权限也没有 valgrind ✗ —— 先跑 tools/memcheck.sh --setup 或请用户装包" >&2; exit 2; }
echo "  ⚠ 降级提示：perf 不可用 ⇒ 改用 callgrind（指令级、慢 ~50×）✗"
"$VG" --tool=callgrind --callgrind-out-file="$CG_DATA" --cache-sim=no --branch-sim=no \
      "$BIN" "$PROBE" >/dev/null 2>"$CG_DATA.log"
{
  echo "# hotprofile —— callgrind（降级；$ITERS 次/档 ${TIER:+；$TIER}）"
  echo "# 读法：Ir 占比最高的函数＝病态成本候选"
  "$ANNOTATE" --threshold=99 "$CG_DATA" 2>/dev/null | grep -E '^\s*[0-9,]+ \(' | head -"$TOP"
} > "$OUT"
echo "  报告 → $OUT"; sed -n '1,18p' "$OUT" | cut -c1-120
rm -f "$PROBE"
