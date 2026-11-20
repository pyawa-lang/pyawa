#!/usr/bin/env node
// **E6 验收：显式帧栈下的深递归（`PLAN §12.22` S4／§12.21 E6 扩展）**。
//
// wasm 的原生栈比宿主更小；S2 把"脚本→脚本调用"改成**堆上的显式帧栈**后，深度 900 的合法递归
// 在 wasm 上也应跑通，且无限递归要抛**可捕获**的 `RecursionError`（不 trap、不崩实例）。
//
// 用法：node tests/wasm/e6_recursion.js [wasm 路径]
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;

function fail(message) {
  console.error(`E6 FAIL: ${message}`);
  process.exit(1);
}

function main() {
  const wasmPath =
    process.argv[2] ||
    path.join(__dirname, '..', '..', 'target', 'wasm32-unknown-unknown', 'debug', 'pyawa_runtime.wasm');
  if (!fs.existsSync(wasmPath)) fail(`找不到 wasm 产物：${wasmPath}`);

  const imports = {
    pyawa_host: {
      pa_host_slot_call: () => 6,
      clock_now_ns: () => 0n,
      clock_monotonic_ns: () => 0n,
      clock_sleep_ns: () => {},
    },
  };
  const wasm = new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(wasmPath)), imports).exports;
  const bytes = () => new Uint8Array(wasm.memory.buffer);
  const view = () => new DataView(wasm.memory.buffer);

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
  const errorMessage = (state) => {
    const ptr = wasm.pa_errmsg(state);
    if (!ptr) return '';
    const mem = bytes();
    let end = ptr;
    while (mem[end] !== 0) end += 1;
    return Buffer.from(mem.slice(ptr, end)).toString('utf8');
  };
  function exec(state, source) {
    const sourceHandle = cstr(source);
    const chunkHandle = cstr('<e6>');
    const modeHandle = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunkHandle.ptr, modeHandle.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunkHandle);
    freeCString(modeHandle);
    return { rc, message: errorMessage(state) };
  }
  function globalInt(state, name) {
    const nameHandle = cstr(name);
    const rc = wasm.pa_getglobal(state, nameHandle.ptr) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_getglobal(${name}) => ${rc}: ${errorMessage(state)}`);
    const out = wasm.pyawa_wasm_alloc(8);
    try {
      const status = wasm.pa_tointeger(state, -1, out) | 0;
      wasm.pa_pop(state, 1);
      if (status !== PA_OK) fail(`pa_tointeger(${name}) => ${status}`);
      return Number(view().getBigInt64(out, true));
    } finally {
      wasm.pyawa_wasm_free(out, 8);
    }
  }

  const state = wasm.paL_newstate();
  if (!state) fail('paL_newstate 返回 NULL');

  const source = [
    'def down(n):',
    '    if n == 0:',
    '        return 0',
    '    return down(n - 1) + 1',
    'deep = down(900)',
    'def infinite(n):',
    '    return infinite(n + 1)',
    'try:',
    '    infinite(0)',
    'except RecursionError:',
    '    caught = 1',
    'else:',
    '    caught = 0',
    // 守卫触发之后实例仍可用
    'after = down(30)',
  ].join('\n');
  const run = exec(state, source);
  if (run.rc !== PA_OK) fail(`脚本执行失败 rc=${run.rc}: ${run.message}`);

  const deep = globalInt(state, 'deep');
  const caught = globalInt(state, 'caught');
  const after = globalInt(state, 'after');
  if (deep !== 900) fail(`wasm 上 down(900) 应为 900，实际 ${deep}`);
  if (caught !== 1) fail(`无限递归应抛可捕获 RecursionError（caught=${caught}）`);
  if (after !== 30) fail(`守卫之后实例应仍可用（after=${after}）`);

  // SAFETY: state 由 paL_newstate 交出、只销毁一次。
  wasm.pa_destroy(state);
  console.log('E6 PASS: wasm 显式帧栈深递归 900 通过、无限递归可捕获 RecursionError');
}

main();
