#!/usr/bin/env bash
# 快档验收（轮内反复用；慢档十项闸门见 agents-rules/round-rule.md §3）。
#
# 用法：tools/quickcheck.sh [探针脚本]
#   tools/quickcheck.sh                          # 只跑 ①～③
#   tools/quickcheck.sh target/recon/loop_class.py
#
# 退出码：0 = 全过；非 0 = 第几步失败（见输出）。
set -uo pipefail
cd "$(dirname "$0")/.."

step() { printf '%s … ' "$1"; }

step "① 编译 pyawa-runtime"
if ! cargo build -p pyawa-runtime --bin pyawa >/tmp/qc_build.log 2>&1; then
  echo "✗"; tail -20 /tmp/qc_build.log; exit 1
fi
echo "✓"

step "② 编译期用例（pyawa-core --test compile）"
if ! cargo test -p pyawa-core --test compile --quiet >/tmp/qc_compile.log 2>&1; then
  echo "✗"; tail -20 /tmp/qc_compile.log; exit 2
fi
echo "✓"

step "③ 关键护栏（pyawa-runtime --test meta_path_shapes）"
if ! cargo test -p pyawa-runtime --test meta_path_shapes --quiet >/tmp/qc_guard.log 2>&1; then
  echo "✗"; tail -20 /tmp/qc_guard.log; exit 3
fi
echo "✓"

if [ "$#" -ge 1 ]; then
  probe="$1"
  step "④ 探针 $probe"
  rm -rf target/recon/__pyawa__
  if ! ./target/debug/pyawa "$probe" >/tmp/qc_probe.log 2>&1; then
    echo "✗（退出码 $?）"; tail -20 /tmp/qc_probe.log; exit 4
  fi
  echo "✓"; tail -5 /tmp/qc_probe.log
fi

echo "quickcheck: OK"
