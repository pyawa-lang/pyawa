#!/usr/bin/env node
// **E7 验收：wasm 作为库嵌入**（`PLAN §12.21` E6 扩展／`PLAN §12.26` P1 库嵌入档）。
//
// 演示的是"宿主把 pyawa 当库用"的最小面（不是 CLI 冒烟）：
//   ① 同一 wasm 模块**并存多个实例**（隔离：一个实例的全局名在另一个实例里不可见）；
//   ② 宿主函数（`pa_register` ＋ wasm trampoline 槽位）跨边界调用；
//   ③ **stdout 走宿主**（P1 的可选 fs 标准流写）：`print` 的字节由 JS 收走；
//   ④ 错误消息经 `pa_errmsg` 带回（不崩、可继续用实例）。
//
// 用法：node tests/wasm/e7_embedding.js [wasm 路径]
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;
const PA_ERR_RUNTIME = 1;

function fail(message) {
  console.error(`E7 FAIL: ${message}`);
  process.exit(1);
}

function main() {
  const wasmPath =
    process.argv[2] ||
    path.join(__dirname, '..', '..', 'target', 'wasm32-unknown-unknown', 'debug', 'pyawa_runtime.wasm');
  if (!fs.existsSync(wasmPath)) fail(`找不到 wasm 产物：${wasmPath}`);

  const imports = {
    pyawa_host: {
      pa_host_slot_call: (slot, state) => {
        if (slot !== 0) return 7; // 非本脚本注册的槽位：如实报错（不静默成功）
        const out = wasm.pyawa_wasm_alloc(8);
        try {
          // 本函数 1 个形参 ⇒ 实参在栈顶（-1）；读实参是**借用**语义。
          const rc = wasm.pa_tointeger(state, -1, out) | 0;
          if (rc !== PA_OK) return rc;
          const value = Number(view().getBigInt64(out, true));
          // 宿主函数约定：读实参（借用）→ 把结果压回虚拟栈顶 → 返回状态码。
          return wasm.pa_pushinteger(state, BigInt(value * 2)) | 0;
        } finally {
          wasm.pyawa_wasm_free(out, 8);
        }
      },
      clock_now_ns: () => 0n,
      clock_monotonic_ns: () => 0n,
      clock_sleep_ns: () => {},
      fs_write: (handle, ptr, len) => {
        const text = Buffer.from(new Uint8Array(wasm.memory.buffer).slice(ptr, ptr + len));
        if (handle === 2) stderr.push(text);
        else stdout.push(text);
        return 0;
      },
    },
  };
  const wasm = new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(wasmPath)), imports).exports;
  const bytes = () => new Uint8Array(wasm.memory.buffer);
  const view = () => new DataView(wasm.memory.buffer);
  const stdout = [];
  const stderr = [];

  function cstr(text) {
    const raw = Buffer.from(text, 'utf8');
    const len = raw.length + 1;
    const ptr = wasm.pyawa_wasm_alloc(len);
    if (!ptr) throw new Error('pyawa_wasm_alloc 失败');
    bytes().set(raw, ptr);
    bytes()[ptr + raw.length] = 0;
    return { ptr, len };
  }
  function freeCString(handle) {
    wasm.pyawa_wasm_free(handle.ptr, handle.len);
  }
  function errmsg(state) {
    const ptr = wasm.pa_errmsg(state);
    if (!ptr) return '';
    const mem = bytes();
    let end = ptr;
    while (mem[end] !== 0) end += 1;
    return Buffer.from(mem.slice(ptr, end)).toString('utf8');
  }
  function exec(state, source) {
    const sourceHandle = cstr(source);
    const chunk = cstr('<e7>');
    const mode = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunk.ptr, mode.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunk);
    freeCString(mode);
    return { rc, message: errmsg(state) };
  }
  function freshState() {
    const state = wasm.paL_newstate();
    if (!state) fail('paL_newstate 返回 NULL');
    return state;
  }
  function installHostFunction(state) {
    const slot = 0;
    const functionPointer = wasm.pyawa_wasm_trampoline(slot);
    const signature = wasm.pyawa_wasm_sig_basic(1);
    const name = cstr('host_double');
    try {
      const rc = wasm.pa_register(state, name.ptr, functionPointer, signature) | 0;
      if (rc !== PA_OK) fail(`pa_register(host_double) => ${rc}: ${errmsg(state)}`);
    } finally {
      freeCString(name);
    }
  }

  // ① 两个实例并存 + 隔离
  const first = freshState();
  const second = freshState();
  installHostFunction(first);
  installHostFunction(second);
  let run = exec(first, 'marker = 41\nprint("first", marker)\nprint("host", host_double(21))\n');
  if (run.rc !== PA_OK) fail(`实例 1 执行失败：${run.message}`);
  run = exec(second, 'print("second", marker)');
  if (run.rc === PA_OK) fail('实例隔离失败：实例 2 读到了实例 1 的 marker');
  if (!run.message.includes('NameError')) fail(`实例 2 应报 NameError，实际：${run.message}`);
  run = exec(second, 'print("second", host_double(4))');
  if (run.rc !== PA_OK) fail(`实例 2 执行失败：${run.message}`);

  const lines = Buffer.concat(stdout).toString('utf8').trim().split('\n');
  if (lines.length !== 3) fail(`stdout 行数应为 3，实际 ${lines.length}：${lines}`);
  if (lines[0] !== 'first 41') fail(`第 1 行应为 'first 41'，实际 ${lines[0]}`);
  if (lines[1] !== 'host 42') fail(`第 2 行应为 'host 42'，实际 ${lines[1]}`);
  if (lines[2] !== 'second 8') fail(`第 3 行应为 'second 8'，实际 ${lines[2]}`);
  if (stderr.length !== 0) fail(`stderr 应为空：${Buffer.concat(stderr).toString('utf8')}`);

  wasm.pa_destroy(first);
  wasm.pa_destroy(second);
  console.log('E7 PASS: 库嵌入（多实例隔离／宿主函数／stdout 经宿主／错误消息）');
}

main();
