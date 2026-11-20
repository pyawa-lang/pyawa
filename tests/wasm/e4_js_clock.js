#!/usr/bin/env node
// **E4 验收：JS 提供 clock 能力域；fs／net 如实"未提供"**（`PLAN §12.21` E4）。
//
// 协议（wasm 侧见 `crates/pyawa-runtime/src/wasm_bridge.rs` 的 E4 段）：import 模块 `pyawa_host`
// 提供 `clock_now_ns()`（挂钟纳秒）、`clock_monotonic_ns()`（单调钟纳秒）、`clock_sleep_ns(ns)`
// （有界睡眠：VM 侧切片 ≤10 ms）。wasm 侧 provider 把它们注册成 `clock` 能力域；`fs`／`net`
// **不注册** ⇒ 脚本访问时必须如实报"未实现"，不得伪造成功。
//
// 用法：node tests/wasm/e4_js_clock.js [wasm 路径]
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;
// 固定"当前时间"：JS 侧给什么，脚本就必须看到什么（可判定）。
const WALL_NS = 1_700_000_000_000_000_000n; // 2023-11-14T22:13:20Z
const WALL_SECONDS = 1_700_000_000;

function fail(message) {
  console.error(`E4 FAIL: ${message}`);
  process.exit(1);
}

function main() {
  const wasmPath =
    process.argv[2] ||
    path.join(__dirname, '..', '..', 'target', 'wasm32-unknown-unknown', 'debug', 'pyawa_runtime.wasm');
  if (!fs.existsSync(wasmPath)) fail(`找不到 wasm 产物：${wasmPath}`);

  // 单调钟：JS 侧从 5 ms 起；每次 sleep 让"时间"前进相应的纳秒数（模拟真实睡眠）。
  let monotonic = 5_000_000n;
  const sleepCalls = [];
  const imports = {
    pyawa_host: {
      pa_host_slot_call: () => 6,
      clock_now_ns: () => WALL_NS,
      clock_monotonic_ns: () => monotonic,
      clock_sleep_ns: (ns) => {
        sleepCalls.push(ns);
        monotonic += BigInt(ns);
      },
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
  function readCString(ptr) {
    const mem = bytes();
    let end = ptr;
    while (mem[end] !== 0) end += 1;
    return Buffer.from(mem.slice(ptr, end)).toString('utf8');
  }
  const errorMessage = (state) => {
    const ptr = wasm.pa_errmsg(state);
    return ptr ? readCString(ptr) : '';
  };
  function exec(state, source) {
    const sourceHandle = cstr(source);
    const chunkHandle = cstr('<e4>');
    const modeHandle = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunkHandle.ptr, modeHandle.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunkHandle);
    freeCString(modeHandle);
    return { rc, message: errorMessage(state) };
  }
  function pushGlobal(state, name) {
    const nameHandle = cstr(name);
    const rc = wasm.pa_getglobal(state, nameHandle.ptr) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_getglobal(${name}) => ${rc}: ${errorMessage(state)}`);
  }
  function globalInt(state, name) {
    pushGlobal(state, name);
    const out = wasm.pyawa_wasm_alloc(8);
    try {
      const valueRc = wasm.pa_tointeger(state, -1, out) | 0;
      wasm.pa_pop(state, 1);
      if (valueRc !== PA_OK) fail(`pa_tointeger(${name}) => ${valueRc}`);
      return Number(view().getBigInt64(out, true));
    } finally {
      wasm.pyawa_wasm_free(out, 8);
    }
  }
  function globalFloat(state, name) {
    pushGlobal(state, name);
    const out = wasm.pyawa_wasm_alloc(8);
    try {
      const valueRc = wasm.pa_tonumber(state, -1, out) | 0;
      wasm.pa_pop(state, 1);
      if (valueRc !== PA_OK) fail(`pa_tonumber(${name}) => ${valueRc}`);
      return view().getFloat64(out, true);
    } finally {
      wasm.pyawa_wasm_free(out, 8);
    }
  }

  const state = wasm.paL_newstate();
  if (!state) fail('paL_newstate 返回 NULL');

  // 1) 挂钟：脚本看到的时间＝JS 给的常量
  let result = exec(state, 'import time\nt = time.time_ns()');
  if (result.rc !== PA_OK) fail(`time.time_ns() 失败：${result.message}`);
  const nowNs = globalInt(state, 't');
  if (nowNs !== Number(WALL_NS)) fail(`time.time_ns() 应为 ${WALL_NS}，实际 ${nowNs}`);

  // 2) 浮点秒：同一来源折秒
  result = exec(state, 's = time.time()');
  if (result.rc !== PA_OK) fail(`time.time() 失败：${result.message}`);
  const nowSeconds = globalFloat(state, 's');
  if (nowSeconds !== WALL_SECONDS) fail(`time.time() 应为 ${WALL_SECONDS}，实际 ${nowSeconds}`);

  // 3) 单调钟：脚本看到 JS 的单调值
  result = exec(state, 'm = time.monotonic_ns()');
  if (result.rc !== PA_OK) fail(`time.monotonic_ns() 失败：${result.message}`);
  const monotonicSeen = globalInt(state, 'm');
  if (monotonicSeen !== Number(monotonic)) fail(`time.monotonic_ns() 应为 ${monotonic}，实际 ${monotonicSeen}`);

  // 4) 睡眠：VM 侧切片 → JS `clock_sleep_ns`；时间前进由 JS 决定
  const beforeSleep = monotonic;
  result = exec(state, 'import time\ntime.sleep(0.01)');
  if (result.rc !== PA_OK) fail(`time.sleep(0.01) 失败：${result.message}`);
  if (sleepCalls.length === 0) fail('VM 没有调用 JS clock_sleep_ns');
  if (monotonic - beforeSleep !== 10_000_000n) {
    fail(`sleep 0.01s 应让 JS 单调钟前进 10 ms，实际 ${monotonic - beforeSleep} ns`);
  }

  // 5) fs 如实"未提供"（浏览器没有文件系统）
  result = exec(state, 'f = open("/tmp/e4")');
  if (result.rc === PA_OK) fail('没有 fs 域时 open() 竟然成功（禁止伪造能力）');
  const fsMessage = result.message;
  if (!fsMessage.includes('fs')) fail(`open() 的错误消息应指向 fs 域：${fsMessage}`);

  // 6) clock 的日历槽未接（`CP-3`）⇒ 如实 NotImplementedError，而不是崩溃或假数据
  result = exec(state, 'x = time.strftime("%Y")');
  if (result.rc === PA_OK) fail('未接的 clock.strftime 槽位竟然成功');
  if (!result.message.includes('NotImplementedError')) fail(`strftime 应如实报未实现：${result.message}`);

  wasm.pa_destroy(state);

  console.log(`E4 PASS: clock.now_ns 由 JS 提供 ⇒ guest time.time_ns() = ${nowNs}`);
  console.log(`E4 PASS: guest time.time() = ${nowSeconds}（同源折秒）`);
  console.log(`E4 PASS: clock.monotonic_ns 由 JS 提供 ⇒ guest 读到 ${monotonicSeen}`);
  console.log(`E4 PASS: time.sleep(0.01) 走 JS clock_sleep_ns（${sleepCalls.length} 次、前进 10 ms）`);
  console.log(`E4 PASS: fs 未注册 ⇒ open() 如实报错：「${fsMessage}」`);
  console.log('E4 PASS: clock 的日历槽未接 ⇒ time.strftime 如实 NotImplementedError');
}

main();
