#!/usr/bin/env bash
# **快档提交前验收（release）**：纯诊断轮／默认零输出轮用（2026-11-09 用户批准的分档）。
# 覆盖：0 警告 ＋ 逐字节 ＋ tests/ci/check.py ＋ 夹具 ＋ 语料下限 ＋ 少量"必须保持绿"的官方套件烟测。
#
# 用法：tools/fastcheck.sh
# 退出码：0＝全过；非 0＝第几步失败。
# 注意：**行为变更轮仍必须跑 `tools/roundcheck.sh`（十项全绿＋survey）**；本脚本不能替代它。
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="$PWD/target/cargo-home"
mkdir -p "$CARGO_HOME"
START=$(date +%s)
STEP=0
fail() { echo "fastcheck: 红 ✗（第 $STEP 步：$1）  墙钟 $(( $(date +%s) - START ))s"; exit "$STEP"; }

STEP=$((STEP + 1)); echo -n "① cargo check --all-targets（0 警告）… "
out=$(cargo check --workspace --all-targets 2>&1); n=$(printf "%s" "$out" | grep -cE "^warning")
if [ "$n" != "0" ]; then printf "%s\n" "$out" | tail -20; fail "警告数=$n"; fi
echo "✓"

STEP=$((STEP + 1)); echo -n "② 编译期用例（逐字节）… "
cargo test --release -p pyawa-core --test compile >target/fastcheck_compile.log 2>&1 || { tail -20 target/fastcheck_compile.log; fail "compile"; }
echo "✓"

STEP=$((STEP + 1)); echo -n "③ tests/ci/check.py（14 项）… "
python3 tests/ci/check.py >target/fastcheck_ci.log 2>&1 || { tail -20 target/fastcheck_ci.log; fail "check.py"; }
echo "✓"

STEP=$((STEP + 1)); echo -n "④ 夹具守卫 … "
python3 tools/check_fixture_cases.py >target/fastcheck_fix.log 2>&1 || { tail -20 target/fastcheck_fix.log; fail "fixtures"; }
echo "✓"

STEP=$((STEP + 1)); echo -n "⑤ 语料下限 … "
python3 tools/check_corpus_floor.py >target/fastcheck_corpus.log 2>&1 || { tail -20 target/fastcheck_corpus.log; fail "corpus"; }
echo "✓"

STEP=$((STEP + 1)); echo -n "⑥ 绿套件烟测（test_rlcompleter ＋ test_dict 的 1F 基线）… "
TD="$PWD/target/upstream/extracted/usr/lib/python3.14"
rm -rf target/recon/__pyawa__
if ! PYAWA_DANGLING=1 PYTHONPATH="$TD:Lib" ./target/release/pyawa "$TD/test/test_rlcompleter.py" >target/fastcheck_smoke1.log 2>&1; then tail -5 target/fastcheck_smoke1.log; fail "test_rlcompleter"; fi
# test_dict 的基线（round 104 起应为全绿）：不放松判定——**必须**是 OK(skipped=10)，否则视为漂移
PYAWA_DANGLING=1 PYTHONPATH="$TD:Lib" ./target/release/pyawa "$TD/test/test_dict.py" >target/fastcheck_smoke2.log 2>&1
grep -qE "^OK \(skipped=10\)" target/fastcheck_smoke2.log || { tail -10 target/fastcheck_smoke2.log; fail "test_dict 基线漂移（期望 OK (skipped=10)）"; }
# test_list 的基线（round 104 起）：只剩 test_deopt_from_append_list 一个 error、无 failure
PYAWA_DANGLING=1 PYTHONPATH="$TD:Lib" ./target/release/pyawa "$TD/test/test_list.py" >target/fastcheck_smoke3.log 2>&1
grep -qE "^FAILED \(errors=1, skipped=2\)" target/fastcheck_smoke3.log || { tail -10 target/fastcheck_smoke3.log; fail "test_list 基线漂移（期望 errors=1, skipped=2）"; }
echo "✓"

echo "fastcheck: OK ✓（快档）  墙钟 $(( $(date +%s) - START ))s"
