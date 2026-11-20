#!/usr/bin/env node
// **E5 冒烟：纯计算 ＋ 协议派发**（`PLAN §12.21` E5；CI 的 `wasm` job 会跑本脚本）。
//
// 只依赖 `clock` 的占位 import（本脚本不用时间）；不碰 `fs`／`net`（浏览器没有）。
// 目的不是覆盖率，而是"wasm 产物里的 VM 真能跑一段普通 Python"：算术／容器／函数／
// 内建协议（`len`、比较、下标、`sort`、`str.upper`）。
//
// 用法：node tests/wasm/e5_smoke.js [wasm 路径]
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;

function fail(message) {
  console.error(`E5 FAIL: ${message}`);
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
      fs_write: () => 0,
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
    const chunkHandle = cstr('<e5>');
    const modeHandle = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunkHandle.ptr, modeHandle.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunkHandle);
    freeCString(modeHandle);
    return { rc, message: errorMessage(state) };
  }
  function withGlobal(state, name, read) {
    const nameHandle = cstr(name);
    const rc = wasm.pa_getglobal(state, nameHandle.ptr) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_getglobal(${name}) => ${rc}: ${errorMessage(state)}`);
    const out = wasm.pyawa_wasm_alloc(8);
    try {
      return read(out);
    } finally {
      wasm.pyawa_wasm_free(out, 8);
    }
  }
  function globalInt(state, name) {
    return withGlobal(state, name, (out) => {
      const rc = wasm.pa_tointeger(state, -1, out) | 0;
      wasm.pa_pop(state, 1);
      if (rc !== PA_OK) fail(`pa_tointeger(${name}) => ${rc}`);
      return Number(view().getBigInt64(out, true));
    });
  }
  function globalBool(state, name) {
    return withGlobal(state, name, (out) => {
      const value = wasm.pa_toboolean(state, -1) | 0;
      wasm.pa_pop(state, 1);
      return value !== 0;
    });
  }
  function globalString(state, name) {
    const nameHandle = cstr(name);
    const rc = wasm.pa_getglobal(state, nameHandle.ptr) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_getglobal(${name}) => ${rc}: ${errorMessage(state)}`);
    const lengthOut = wasm.pyawa_wasm_alloc(4);
    try {
      const ptr = wasm.pa_tostring(state, -1, lengthOut);
      const length = view().getUint32(lengthOut, true);
      wasm.pa_pop(state, 1);
      if (!ptr) fail(`pa_tostring(${name}) 返回 NULL`);
      return Buffer.from(bytes().slice(ptr, ptr + length)).toString('utf8');
    } finally {
      wasm.pyawa_wasm_free(lengthOut, 4);
    }
  }

  const state = wasm.paL_newstate();
  if (!state) fail('paL_newstate 返回 NULL');

  // 纯计算＋内建／协议派发（不碰 fs／net；不打印）。
  const source = [
    'def add(a, b):',
    '    return a + b',
    'data = [3, 1, 2]',
    'data.sort()',
    'total = data[0] + data[1] * data[2]',
    'n = len(data)',
    'mapping = {"a": 1, "b": 2}',
    'picked = mapping["b"]',
    'same = (1, 2) == (1, 2)',
    'text = "abc".upper()',
    'summed = add(20, 22)',
  ].join('\n');
  const run = exec(state, source);
  if (run.rc !== PA_OK) fail(`冒烟脚本执行失败 rc=${run.rc}: ${run.message}`);

  const total = globalInt(state, 'total');
  const count = globalInt(state, 'n');
  const picked = globalInt(state, 'picked');
  const summed = globalInt(state, 'summed');
  const same = globalBool(state, 'same');
  const text = globalString(state, 'text');

  if (total !== 7) fail(`data 排序后 total 应为 7，实际 ${total}`);
  if (count !== 3) fail(`len(data) 应为 3，实际 ${count}`);
  if (picked !== 2) fail(`dict 下标应为 2，实际 ${picked}`);
  if (summed !== 42) fail(`add(20, 22) 应为 42，实际 ${summed}`);
  if (!same) fail('(1, 2) == (1, 2) 应为 True');
  if (text !== 'ABC') fail(`"abc".upper() 应为 ABC，实际 ${text}`);

  const stackTop = wasm.pa_gettop(state) | 0;
  if (stackTop !== 0) fail(`调用后虚拟栈应净空，实际 top=${stackTop}`);
  wasm.pa_destroy(state);

  console.log(`E5 PASS: 纯计算／函数／容器（total=${total}、len=${count}、dict 下标=${picked}、add=${summed}）`);
  console.log(`E5 PASS: 协议派发（元组比较=${same}、str.upper=${text}）`);
  console.log(`E5 PASS: 调用后虚拟栈净空（top=${stackTop}）`);
}

main();
