#!/usr/bin/env bash
# **内存安全检查仪器**（本地前缀 valgrind；无需 root）
#
# 为什么用本地前缀：本机 `uid=1000`，`sudo` 需要交互认证 ⇒ 装不了包 ✗；
#   `apt-get download valgrind` ＋ `dpkg-deb -x` 解到 `target/valgrind/` 即可直接运行 ✓
#   （`--setup` 自举；`target/` 已 gitignore ⇒ 仪器不进仓库 ✓）。
#
# 用法：
#   tools/memcheck.sh --setup            # 自举：下 .deb 并解包
#   tools/memcheck.sh                    # 默认跑 40 个语料用例
#   tools/memcheck.sh --full             # 全量语料
#   tools/memcheck.sh --limit 100        # 指定前 N 个用例
#   tools/memcheck.sh --bin target/release/pyawa
#
# 口径：只关心 **invalid read/write、double free、uninitialised**（`--leak-check=no`）；
#   泄漏由 `PYAWA_DANGLING=1`／`memdangle_survey.py` 负责，两者互补 ✓。
#   慢约 20–50× ⇒ 默认只跑子集；`--full` 才全量。
# 结果：`target/memcheck.tsv`（逐用例）＋ 一行汇总；**有错 exit 99**（可直接进闸门 ✓）。
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VG_DIR="$ROOT/target/valgrind"
# 优先系统 valgrind；没有再退回本地前缀（**退回会打印来源** ✓，不静默 ✓）
VG="$(command -v valgrind 2>/dev/null || echo "$VG_DIR/extract/usr/bin/valgrind")"
OUT="$ROOT/target/memcheck.tsv"

setup() {
  mkdir -p "$VG_DIR"; cd "$VG_DIR" || exit 2
  if [ ! -x "$VG" ]; then
    apt-get download valgrind >/dev/null 2>&1 || { echo "  下载失败 ✗（无网／apt 源不可用）" >&2; exit 2; }
    for d in ./*.deb; do dpkg-deb -x "$d" extract; done
  fi
  [ -x "$VG" ] || { echo "  解包后仍找不到 valgrind ✗" >&2; exit 2; }
  echo "  valgrind 就绪 ✓ $("$VG" --version 2>/dev/null | head -1)"
}

LIMIT=40; BIN="$ROOT/target/debug/pyawa"; DO_SETUP=0
while [ $# -gt 0 ]; do
  case "$1" in
    --setup) DO_SETUP=1; shift ;;
    --full) LIMIT=0; shift ;;
    --limit) LIMIT="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done
[ "$DO_SETUP" = "1" ] && setup
[ -x "$VG" ] || setup
[ -x "$BIN" ] || { echo "找不到 CLI：$BIN" >&2; exit 2; }

export VALGRIND_LIB="$VG_DIR/extract/usr/libexec/valgrind"
export MALLOC_CHECK_=3 MALLOC_PERTURB_=165

mapfile -t CASES < <(find "$ROOT/tests/conformance" -name '*.py' 2>/dev/null | sort)
[ "${#CASES[@]}" -eq 0 ] && { echo "找不到语料 .py ✗" >&2; exit 2; }
if [ "$LIMIT" -gt 0 ] && [ "${#CASES[@]}" -gt "$LIMIT" ]; then CASES=("${CASES[@]:0:$LIMIT}"); fi

errs=0; bad=0
: > "$OUT"
printf "file\terrors\tnote\n" >> "$OUT"
for c in "${CASES[@]}"; do
  log=$(mktemp)
  "$VG" --error-exitcode=99 --leak-check=no --track-origins=yes \
        --errors-for-leak-kinds=none -q "$BIN" "$c" >/dev/null 2>"$log"
  rc=$?
  n=$(grep -oE 'ERROR SUMMARY: [0-9]+' "$log" | grep -oE '[0-9]+' | head -1)
  [ -z "${n:-}" ] && n=0
  note=""
  [ "$rc" -ne 0 ] && { note="exit=$(echo $rc)"; bad=$((bad+1)); }
  if [ "$n" -gt 0 ] 2>/dev/null; then
    errs=$((errs+n)); bad=$((bad+1))
    note="$(grep -m1 -E 'Invalid |double free|uninitialised' "$log" | cut -c1-110)"
  fi
  printf "%s\t%s\t%s\n" "$(basename "$c")" "$n" "$note" >> "$OUT"
  rm -f "$log"
done

echo "memcheck：${#CASES[@]} 个用例 ｜ 累计错误 $errs ｜ 有问题用例 $bad"
echo "  明细 → $OUT"
if [ "$errs" -gt 0 ] || [ "$bad" -gt 0 ]; then exit 99; fi
echo "  ✓ 无 invalid read/write／double free／uninitialised"
