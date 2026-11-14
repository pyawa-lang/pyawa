#!/usr/bin/env python3
"""**§12.13 #1／T-AB-18**：组合根"电池版"产物的最小 C 宿主探针（**不依赖 numpy**）。

口径（`SPEC-c-abi` §15.4；`PLAN §12.13 #1` 路线 A）：
- `pyawa-runtime` 的 C 产物（cdylib／staticlib）是**单一库**：同时导出 `pa_*` 与 `paL_*`；
- `paL_newstate()` ⇒ 建实例＋注册 `fs`／`clock`／`net` 真实能力＋装 builtins／`sys`（`import` 可用）；
- `pa_create()` 的裸实例只有语言骨架（`print` 等未定义；错误带 `pa_errmsg`）；
- `paL_openlibs(st)` 可给已有实例补 builtins／`sys`（能力域仍由宿主负责）。

用法（在仓库根跑；宿主侧不需要 numpy）：

    python3 tests/ci/host_usability_probe.py --release
    python3 tests/ci/host_usability_probe.py            # debug 产物

退出码 0 ＝ 全绿；1 ＝ 红（打印每条判据）。
"""

from __future__ import annotations

import argparse
import ctypes
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PA_OK = 0
PA_ERR_INVALID = 6
PA_ERR_RUNTIME = 1


class PaHost(ctypes.Structure):
    _fields_ = [
        ("abi_size", ctypes.c_size_t),
        ("abi_version", ctypes.c_uint32),
        ("capabilities", ctypes.c_void_p),
    ]


def library_path(release: bool) -> pathlib.Path:
    directory = ROOT / "target" / ("release" if release else "debug")
    for name in ("libpyawa_runtime.so", "libpyawa_runtime.dylib", "pyawa_runtime.dll"):
        candidate = directory / name
        if candidate.is_file():
            return candidate
    raise SystemExit(
        f"找不到组合根 C 产物（{directory}）——先构建："
        "cargo build -p pyawa-runtime --lib（release 加 --release）"
    )


def bind(library: ctypes.CDLL) -> None:
    library.pa_abi_version.restype = ctypes.c_uint32
    library.paL_newstate.restype = ctypes.c_void_p
    library.paL_openlibs.argtypes = [ctypes.c_void_p]
    library.paL_openlibs.restype = ctypes.c_int
    library.pa_create.argtypes = [ctypes.POINTER(PaHost), ctypes.POINTER(ctypes.c_void_p)]
    library.pa_create.restype = ctypes.c_int
    library.pa_exec_string.argtypes = [
        ctypes.c_void_p,
        ctypes.c_char_p,
        ctypes.c_ssize_t,
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.c_void_p,
    ]
    library.pa_exec_string.restype = ctypes.c_int
    library.pa_errmsg.argtypes = [ctypes.c_void_p]
    library.pa_errmsg.restype = ctypes.c_char_p
    library.pa_destroy.argtypes = [ctypes.c_void_p]
    library.pa_destroy.restype = ctypes.c_int


def message(library: ctypes.CDLL, state: int) -> str:
    raw = library.pa_errmsg(state)
    return raw.decode("utf-8", "replace") if raw else ""


def execute(library: ctypes.CDLL, state: int, source: str, mode: str = "python") -> int:
    encoded = source.encode()
    return library.pa_exec_string(
        state, encoded, len(encoded), b"probe", mode.encode(), None
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args(argv)
    path = library_path(args.release)
    library = ctypes.CDLL(str(path))
    bind(library)
    failures: list[str] = []

    # ① 电池版入口：builtins／import／print 全可用（print 经 fs 域写标准流）。
    state = library.paL_newstate()
    if not state:
        failures.append("paL_newstate 交回 NULL")
    else:
        script = (
            "import math\n"
            "assert math.floor(3.7) == 3\n"
            "assert len([1, 2, 3]) == 3\n"
            "assert ValueError is not None\n"
            "print('host-usability-probe-ok')\n"
        )
        code = execute(library, state, script)
        if code != PA_OK:
            failures.append(f"电池版执行 builtins/import 失败：状态 {code}／{message(library, state)}")
        # ② 未捕获异常 ⇒ 状态码＋可诊断消息（`AB-48`）。
        code = execute(library, state, "raise ValueError('probe')")
        if code != PA_ERR_RUNTIME or not message(library, state):
            failures.append(f"异常路径应带状态码＋消息，实际 {code}／{message(library, state)!r}")
        library.pa_destroy(state)

    # ③ 裸实例（`pa_create`）：只有语言骨架；`print` 未定义、错误带消息。
    host = PaHost(
        abi_size=ctypes.sizeof(PaHost),
        abi_version=library.pa_abi_version(),
        capabilities=None,
    )
    raw_state = ctypes.c_void_p()
    created = library.pa_create(ctypes.byref(host), ctypes.byref(raw_state))
    if created != PA_OK or not raw_state.value:
        failures.append(f"pa_create 裸实例失败：{created}")
    else:
        code = execute(library, raw_state.value, "print(1)")
        if code == PA_OK:
            failures.append("裸实例不应有 builtins（print 却成功）")
        elif not message(library, raw_state.value):
            failures.append("裸实例失败必须带 pa_errmsg")
        # ④ `paL_openlibs`：给已有实例补 builtins／sys（不需要能力域即可跑纯计算）。
        code = library.paL_openlibs(raw_state.value)
        if code != PA_OK:
            failures.append(f"paL_openlibs 失败：{code}／{message(library, raw_state.value)}")
        else:
            code = execute(library, raw_state.value, "assert len([1, 2]) == 2\n")
            if code != PA_OK:
                failures.append(f"paL_openlibs 后纯计算失败：{code}／{message(library, raw_state.value)}")
        library.pa_destroy(raw_state.value)

    # ⑤ 非法参数：`paL_openlibs(NULL)` ⇒ `PA_ERR_INVALID`。
    if library.paL_openlibs(None) != PA_ERR_INVALID:
        failures.append("paL_openlibs(NULL) 应返回 PA_ERR_INVALID")

    if failures:
        for failure in failures:
            print(f"[FAIL] {failure}", file=sys.stderr)
        return 1
    print("[PASS] host-usability：电池版 builtins/import/print、裸实例报错带消息、paL_openlibs 可用")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
