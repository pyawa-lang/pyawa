#!/usr/bin/env bash
# **移动端构建档**（`PLAN-milestones.md` §12.26 P2）：Android／iOS 交叉编译 ＋ 产物存在 ＋ ABI 导出符号检查。
#
# 用法：
#   tools/ci/mobile_build.sh android   # Linux／Android NDK ⇒ aarch64-linux-android
#   tools/ci/mobile_build.sh ios       # macOS／Xcode ⇒ aarch64-apple-ios
#
# **如实边界（2026-11-28，round 294）**：
# - 只到**构建档**：交付物是 staticlib（`libpyawa_abi.a`／`libpyawa_runtime.a`）；
#   Android 另出 cdylib（`.so`）。**iOS 只出 staticlib**（Apple 平台标准；且 Darwin 上 cdylib
#   链接会缺运行时的 glibc errno 符号 `__errno_location`——运行档如实未接，见 RELEASE）。
#   **未**在真机／模拟器运行、**未**接移动端能力层（fs／net／clock 未验证）、**未**做 App
#   集成／签名／打包。
# - ABI 冒烟仅**符号级**（`llvm-nm` 查导出符号）；不做宿主链接冒烟（那是运行档的事）。
# - CI 侧只在 **main push 与 workflow_dispatch** 跑（PR 不跑：macOS runner 分钟数贵；本 job
#   **不是**必需检查）。
#
# 依赖：
# - android：Android NDK。探测顺序 `$ANDROID_NDK_HOME`／`$ANDROID_NDK`／`$ANDROID_NDK_LATEST_HOME`／
#   `$ANDROID_NDK_ROOT`，再退到 `$ANDROID_HOME/ndk/*`／`$ANDROID_SDK_ROOT/ndk/*`／
#   `/usr/local/lib/android/sdk/ndk/*`（GitHub 托管 ubuntu runner 的预装布局）。用其中的
#   `aarch64-linux-android<api>-clang` 当 linker。
#   已有 `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` 时直接沿用（受限宿主／本地复现用）。
# - ios：macOS ＋ Xcode；rustc 走宿主 `cc`＋iphoneos SDK（脚本显式导出 `SDKROOT` 以求确定）。
#   不签名、不打包。
# - rust-std：目标 std 缺失时自动 `rustup target add <triple>`（`PYAWA_MOBILE_SKIP_TARGET_INSTALL=1`
#   可跳过——受限宿主自备 sysroot 时用）。
set -euo pipefail
cd "$(dirname "$0")/../.."

TRIPLE=""
case "${1:-}" in
  android) TRIPLE=aarch64-linux-android ;;
  ios) TRIPLE=aarch64-apple-ios ;;
  *)
    echo "用法: $0 android|ios" >&2
    exit 2
    ;;
esac
die() {
  echo "mobile_build: $*" >&2
  exit 1
}

# --------------------------------------------------------------------------- #
# rust-std
# --------------------------------------------------------------------------- #
if [ "${PYAWA_MOBILE_SKIP_TARGET_INSTALL:-0}" != "1" ]; then
  TRIPLE_LIBDIR="$(rustc --print target-libdir --target "$TRIPLE" 2>/dev/null || true)"
  if [ -z "$TRIPLE_LIBDIR" ] || [ ! -d "$TRIPLE_LIBDIR" ]; then
    echo "== 安装 rust-std：$TRIPLE"
    rustup target add "$TRIPLE"
  fi
fi

# --------------------------------------------------------------------------- #
# 平台工具链（linker／SDK）
# --------------------------------------------------------------------------- #
TOOLCHAIN=""
if [ "$TRIPLE" = "aarch64-linux-android" ]; then
  if [ -n "${CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER:-}" ]; then
    echo "== linker（环境已给）：${CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER}"
  else
    find_ndk() {
      local candidate
      for candidate in \
        "${ANDROID_NDK_HOME:-}" "${ANDROID_NDK:-}" \
        "${ANDROID_NDK_LATEST_HOME:-}" "${ANDROID_NDK_ROOT:-}"; do
        if [ -n "$candidate" ] && [ -d "$candidate/toolchains/llvm/prebuilt" ]; then
          printf '%s' "$candidate"
          return 0
        fi
      done
      local sdk
      for sdk in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" /usr/local/lib/android/sdk; do
        [ -n "$sdk" ] && [ -d "$sdk/ndk" ] || continue
        candidate="$(find "$sdk/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1)"
        if [ -n "$candidate" ] && [ -d "$candidate/toolchains/llvm/prebuilt" ]; then
          printf '%s' "$candidate"
          return 0
        fi
      done
      return 1
    }
    NDK_ROOT="$(find_ndk || true)"
    [ -n "$NDK_ROOT" ] || die "找不到 Android NDK（设 ANDROID_NDK_HOME 或预置 CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER）"
    if [ -f "$NDK_ROOT/source.properties" ]; then
      echo "== NDK：${NDK_ROOT}（$(grep -E '^Revision' "$NDK_ROOT/source.properties" || true)）"
    else
      echo "== NDK：$NDK_ROOT"
    fi
    # NDK 的 prebuilt 目录名：linux-x86_64／darwin-x86_64（取实际存在的那个）。
    for candidate in "$NDK_ROOT/toolchains/llvm/prebuilt/"*; do
      [ -d "$candidate/bin" ] && TOOLCHAIN="$candidate" && break
    done
    [ -n "$TOOLCHAIN" ] || die "NDK 里找不到 llvm prebuilt 工具链：$NDK_ROOT/toolchains/llvm/prebuilt"
    # 最低 API 的 clang 包装（API 越高、产物要求的最低 Android 版本越高；构建档取兼容面最大者）。
    LINKER="$(find "$TOOLCHAIN/bin" -maxdepth 1 -name 'aarch64-linux-android*-clang' 2>/dev/null \
      | sort -V | head -n 1)"
    [ -n "$LINKER" ] || die "NDK 里找不到 aarch64-linux-android<api>-clang：$TOOLCHAIN/bin"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$LINKER"
    echo "== linker：$LINKER"
  fi
else
  echo "== iOS：rustc 走宿主 cc ＋ iphoneos SDK（不签名、不打包）"
  if command -v xcrun >/dev/null 2>&1; then
    SDKROOT_XCRUN="$(xcrun --sdk iphoneos --show-sdk-path 2>/dev/null || true)"
    if [ -n "$SDKROOT_XCRUN" ]; then
      export SDKROOT="$SDKROOT_XCRUN"
      echo "== iphoneos SDK：$SDKROOT"
    fi
  fi
fi

# --------------------------------------------------------------------------- #
# 构建（staticlib ＋ cdylib；rlib 随 --lib 一并出）
# --------------------------------------------------------------------------- #
if [ "$TRIPLE" = "aarch64-apple-ios" ]; then
  # **iOS 只出 staticlib**：Apple 平台的应用内嵌库标准产物就是 `.a`（不做 dylib 分发）；
  # 另外 `pyawa-runtime` 的 unix 平台点仍用 **glibc 的 `__errno_location`**（Darwin 是
  # `__error`）⇒ cdylib 的**链接**在 iOS 上会缺这个符号。构建档如实只承诺 staticlib，
  # 运行档（Darwin errno／日历槽）如实未接——见 RELEASE 的已知限制。
  # cargo 不能按目标挑 `crate-type` ⇒ 用 rustc wrapper 把 `cdylib` 从 crate-type 列表里去掉。
  WRAPPER="target/ci/rustc-no-cdylib.sh"
  mkdir -p "$(dirname "$WRAPPER")"
  cat >"$WRAPPER" <<'WRAP'
#!/usr/bin/env bash
# 仅构建档用：把 rustc 的 crate-type 列表里的 `cdylib` 去掉（其余参数原样透传）。
set -euo pipefail
rustc_bin="$1"
shift
# cargo 会借 rustc 探测目标信息（`--print=file-names` 等）——**原样透传**，否则它数不对 crate-type
# 的输出行数（"output of --print=file-names missing"）。
case " $* " in
  *" --print"*)
    exec "$rustc_bin" "$@"
    ;;
esac
args=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --crate-type=*)
      types="${1#--crate-type=}"
      kept=""
      IFS=',' read -r -a parts <<<"$types"
      for part in "${parts[@]}"; do
        if [ "$part" != "cdylib" ] && [ -n "$part" ]; then
          kept="${kept:+$kept,}$part"
        fi
      done
      [ -n "$kept" ] && args+=("--crate-type=$kept")
      shift
      ;;
    --crate-type)
      types="$2"
      kept=""
      IFS=',' read -r -a parts <<<"$types"
      for part in "${parts[@]}"; do
        if [ "$part" != "cdylib" ] && [ -n "$part" ]; then
          kept="${kept:+$kept,}$part"
        fi
      done
      [ -n "$kept" ] && args+=("--crate-type" "$kept")
      shift 2
      ;;
    *)
      args+=("$1")
      shift
      ;;
  esac
done
exec "$rustc_bin" "${args[@]}"
WRAP
  chmod +x "$WRAPPER"
  export RUSTC_WRAPPER="$PWD/$WRAPPER"
  echo "== iOS：staticlib-only（rustc wrapper 去掉 cdylib；不签名、不打包）"
fi
echo "== cargo build --release --lib --target $TRIPLE -p pyawa-abi -p pyawa-runtime"
tools/cargo.sh build --release --lib --target "$TRIPLE" -p pyawa-abi -p pyawa-runtime

LIBDIR="target/$TRIPLE/release"
ABI_LIB="$LIBDIR/libpyawa_abi.a"
RUNTIME_LIB="$LIBDIR/libpyawa_runtime.a"
for lib in "$ABI_LIB" "$RUNTIME_LIB"; do
  [ -s "$lib" ] || die "缺少 staticlib（或为空）：$lib"
  echo "== 产物：${lib}（$(wc -c <"$lib") 字节）"
done
if [ "$TRIPLE" = "aarch64-apple-ios" ]; then
  # iOS 只承诺 staticlib（见上）；cdylib 刻意不产出。
  [ ! -e "$LIBDIR/libpyawa_runtime.dylib" ] \
    || echo "== 提示：出现了未承诺的 dylib（$LIBDIR/libpyawa_runtime.dylib），构建档不使用它"
else
  CDYLIB="$LIBDIR/libpyawa_runtime.so"
  [ -e "$CDYLIB" ] || die "缺少 cdylib：$CDYLIB"
  echo "== 产物：$CDYLIB"
fi

# --------------------------------------------------------------------------- #
# ABI 导出符号（符号级冒烟；`SPEC-c-abi.md` §15.4 的组合根入口 `paL_newstate`）
# --------------------------------------------------------------------------- #
# Mach-O 的 C 符号带前导下划线；ELF 不带。
PREFIX=""
case "$TRIPLE" in
  aarch64-apple-ios) PREFIX="_" ;;
esac

find_nm() {
  # ① 显式覆盖（受限宿主自备匹配版本的 llvm-nm 时用）。
  if [ -n "${PYAWA_MOBILE_NM:-}" ]; then
    printf '%s' "$PYAWA_MOBILE_NM"
    return 0
  fi
  # ② 与 rustc 同源的 `llvm-tools-preview`（LLVM 版本一致 ⇒ 能读 rust 产出的对象；
  #    CI 的构建步骤会 `rustup component add llvm-tools-preview`）。
  local host sysroot
  host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
  sysroot="$(rustc --print sysroot 2>/dev/null || true)"
  if [ -n "$host" ] && [ -n "$sysroot" ] && [ -x "$sysroot/lib/rustlib/$host/bin/llvm-nm" ]; then
    printf '%s' "$sysroot/lib/rustlib/$host/bin/llvm-nm"
    return 0
  fi
  # ③ NDK 自带的 llvm-nm（android）。
  if [ -n "${TOOLCHAIN:-}" ] && [ -x "$TOOLCHAIN/bin/llvm-nm" ]; then
    printf '%s' "$TOOLCHAIN/bin/llvm-nm"
    return 0
  fi
  # ④ Xcode／系统 llvm-nm，最后退到 nm。
  if command -v xcrun >/dev/null 2>&1 && xcrun -f llvm-nm >/dev/null 2>&1; then
    xcrun -f llvm-nm
    return 0
  fi
  if command -v llvm-nm >/dev/null 2>&1; then
    command -v llvm-nm
    return 0
  fi
  command -v nm
}
NM="$(find_nm || true)"
[ -n "$NM" ] || die "找不到 llvm-nm／nm（符号检查需要它）"
echo "== 符号检查：$NM"

check_archive() {
  local lib="$1"
  shift
  local defined symbol
  # 版本不匹配的 nm 会在**部分成员**上报错（rust 的 llvm 比宿主 llvm 新时），但其余成员照常输出
  # ⇒ 不把退出码当失败，只看「需要的符号是否出现在输出里」（找不到才红）。
  defined="$("$NM" -g --defined-only "$lib" 2>/dev/null || true)"
  if [ -z "$defined" ]; then
    # BSD nm 没有 `--defined-only` 时退一层（CI 走 ② 的 llvm-nm，正常不会到这里）。
    defined="$("$NM" -g "$lib" 2>/dev/null || true)"
  fi
  [ -n "$defined" ] || die "nm 读不了产物：$lib"
  for symbol in "$@"; do
    if ! grep -Eq "(^|[[:space:]])${PREFIX}${symbol}$" <<<"$defined"; then
      die "缺少导出符号：${PREFIX}${symbol}（${lib}）"
    fi
    echo "  ✓ ${PREFIX}${symbol}"
  done
}
# 组合根（runtime）静态库：`paL_*` ＋ staticlib 并入依赖 rlib 后的 `pa_*` 都应在。
check_archive "$RUNTIME_LIB" \
  paL_newstate pa_create pa_destroy pa_exec_string pa_getglobal pa_tointeger pa_tostring pa_errmsg
# 最小 ABI 静态库：只有 `pa_*`（`paL_newstate` 由组合根提供）。
check_archive "$ABI_LIB" pa_create pa_destroy pa_exec_string pa_getglobal pa_errmsg

echo "== 移动端构建档 OK：$TRIPLE"
