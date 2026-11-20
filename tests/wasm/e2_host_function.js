#!/usr/bin/env node
// **E2 验收：JS 注册宿主函数、guest 调用它**（`docs/PLAN-milestones.md` §12.21 E2）。
//
// 协议（wasm 侧见 `crates/pyawa-runtime/src/wasm_bridge.rs`）：
// 1. wasm 导出一个 import `pyawa_host.pa_host_slot_call(slot, state)`；
// 2. wasm 导出固定槽位的 trampoline 表索引：`pyawa_wasm_trampoline(slot)`；
// 3. JS 用 `pyawa_wasm_sig_basic(nparams)` 造基础签名，把 trampoline 指针传给 `pa_register`；
// 4. JS 在 `hostTable[slot]` 放回调：读实参（`pa_tointeger`）、留结果在栈顶（`pa_pushinteger`）、
//    返回状态码（与 C 宿主同一套约定）。
//
// 用法：node tests/wasm/e2_host_function.js [wasm 路径]
// 缺省：target/wasm32-unknown-unknown/debug/pyawa_runtime.wasm
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;
const PA_ERR_INVALID = 6; // 只用于本脚本的失败信号，不依赖 C 头

function fail(message) {
  console.error(`E2 FAIL: ${message}`);
  process.exit(1);
}

function main() {
  const wasmPath =
    process.argv[2] ||
    path.join(__dirname, '..', '..', 'target', 'wasm32-unknown-unknown', 'debug', 'pyawa_runtime.wasm');
  if (!fs.existsSync(wasmPath)) fail(`找不到 wasm 产物：${wasmPath}（先 cargo build --target wasm32-unknown-unknown）`);

  const hostTable = new Array(64).fill(null);
  let hostCalls = 0;
  let hostError = null;
  let wasm = null;

  // `pyawa_host` 的 import 面＝E2（宿主调用）＋ E4（JS clock 三条）；E2 只需前者，clock 给占位。
  const imports = {
    pyawa_host: {
      clock_now_ns: () => 0n,
      clock_monotonic_ns: () => 0n,
      clock_sleep_ns: () => {},
      pa_host_slot_call(slot, state) {
        const callback = hostTable[slot];
        if (!callback) return PA_ERR_INVALID; // 未注册的槽位：如实报错，不静默成功
        hostCalls += 1;
        try {
          return callback(state) | 0;
        } catch (error) {
          hostError = error; // JS 异常不得穿回 wasm（wasm32 是 panic=abort）
          return 1; // PA_ERR_RUNTIME 一类：让 guest 看到宿主失败
        }
      },
    },
  };

  const module = new WebAssembly.Module(fs.readFileSync(wasmPath));
  const moduleImports = WebAssembly.Module.imports(module);
  const expectedImports = ['pa_host_slot_call', 'clock_now_ns', 'clock_monotonic_ns', 'clock_sleep_ns'];
  const actualImports = moduleImports.map((entry) => `${entry.module}.${entry.name}`).sort();
  if (actualImports.join(',') !== expectedImports.map((name) => `pyawa_host.${name}`).sort().join(',')) {
    fail(`import 面不符：${JSON.stringify(moduleImports)}`);
  }
  wasm = new WebAssembly.Instance(module, imports).exports;

  const bytes = () => new Uint8Array(wasm.memory.buffer);
  const view = () => new DataView(wasm.memory.buffer);

  /** 把 JS 字符串写进 wasm 线性内存（含 NUL）；返回 {ptr, len}，用完 `freeCString`。 */
  function cstr(text) {
    const raw = Buffer.from(text, 'utf8');
    const len = raw.length + 1;
    const ptr = wasm.pyawa_wasm_alloc(len);
    if (!ptr) throw new Error(`pyawa_wasm_alloc(${len}) 返回 NULL`);
    const mem = bytes();
    mem.set(raw, ptr);
    mem[ptr + raw.length] = 0;
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
  function errorMessage(state) {
    const ptr = wasm.pa_errmsg(state);
    return ptr ? readCString(ptr) : '(no message)';
  }

  /** 取栈上整数；失败返回 [状态码, null]。 */
  function stackInt(state, index) {
    const out = wasm.pyawa_wasm_alloc(8);
    if (!out) throw new Error('pyawa_wasm_alloc(8) 返回 NULL');
    try {
      const rc = wasm.pa_tointeger(state, index, out) | 0;
      if (rc !== PA_OK) return [rc, null];
      return [PA_OK, Number(view().getBigInt64(out, true))];
    } finally {
      wasm.pyawa_wasm_free(out, 8);
    }
  }

  function pushInt(state, value) {
    return wasm.pa_pushinteger(state, BigInt(value)) | 0;
  }

  // 宿主回调（与 C 宿主同一套：读实参 → 压结果 → 返回状态码）
  function hostAdd(state) {
    const [leftRc, left] = stackInt(state, -2);
    if (leftRc !== PA_OK) return leftRc;
    const [rightRc, right] = stackInt(state, -1);
    if (rightRc !== PA_OK) return rightRc;
    return pushInt(state, left + right);
  }
  function hostMultiply(state) {
    const [leftRc, left] = stackInt(state, -2);
    if (leftRc !== PA_OK) return leftRc;
    const [rightRc, right] = stackInt(state, -1);
    if (rightRc !== PA_OK) return rightRc;
    return pushInt(state, left * right);
  }

  const capacity = wasm.pyawa_wasm_trampoline_capacity() >>> 0;
  if (capacity < 2) fail(`槽位上限 ${capacity} < 2`);
  if ((wasm.pyawa_wasm_trampoline(capacity) >>> 0) !== 0) fail('越界槽位应返回 0');

  const state = wasm.paL_newstate();
  if (!state) fail('paL_newstate 返回 NULL');

  function register(name, slot, callback) {
    hostTable[slot] = callback;
    const nameHandle = cstr(name);
    const signature = wasm.pyawa_wasm_sig_basic(2);
    const trampoline = wasm.pyawa_wasm_trampoline(slot) >>> 0;
    const rc = wasm.pa_register(state, nameHandle.ptr, trampoline, signature) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_register(${name}) => ${rc}: ${errorMessage(state)}`);
  }
  register('host_add', 0, hostAdd);
  register('host_mul', 1, hostMultiply);

  function exec(source) {
    const sourceHandle = cstr(source);
    const chunkHandle = cstr('<e2>');
    const modeHandle = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunkHandle.ptr, modeHandle.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunkHandle);
    freeCString(modeHandle);
    if (rc !== PA_OK) fail(`pa_exec_string => ${rc}: ${errorMessage(state)}`);
  }

  function globalInt(name) {
    const nameHandle = cstr(name);
    const rc = wasm.pa_getglobal(state, nameHandle.ptr) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_getglobal(${name}) => ${rc}: ${errorMessage(state)}`);
    const [valueRc, value] = stackInt(state, -1);
    wasm.pa_pop(state, 1);
    if (valueRc !== PA_OK) fail(`pa_tointeger(${name}) => ${valueRc}: ${errorMessage(state)}`);
    return value;
  }

  exec('x = host_add(1, 2)\ny = host_mul(3, 4)');
  const x = globalInt('x');
  const y = globalInt('y');
  exec('z = host_add(10, -4)');
  const z = globalInt('z');
  if (hostError) fail(`JS 回调内异常：${hostError}`);
  if (hostCalls !== 3) fail(`宿主回调应跨边界 3 次，实际 ${hostCalls}`);
  if (x !== 3) fail(`host_add(1, 2) 应为 3，实际 ${x}`);
  if (y !== 12) fail(`host_mul(3, 4) 应为 12，实际 ${y}`);
  if (z !== 6) fail(`host_add(10, -4) 应为 6，实际 ${z}`);

  const globalCount = wasm.pa_gettop(state) | 0;
  wasm.pa_destroy(state);

  console.log('E2 PASS: wasm import 面 = pyawa_host.{pa_host_slot_call, clock_*}（4 条）');
  console.log(`E2 PASS: 固定槽位表 capacity=${capacity}；越界槽位返回 0`);
  console.log(`E2 PASS: JS 注册 host_add(slot 0)／host_mul(slot 1)，guest 调用 ${hostCalls} 次`);
  console.log(`E2 PASS: guest x = host_add(1, 2) => ${x}`);
  console.log(`E2 PASS: guest y = host_mul(3, 4) => ${y}`);
  console.log(`E2 PASS: guest z = host_add(10, -4) => ${z}`);
  console.log(`E2 PASS: 调用后虚拟栈净空（top=${globalCount}）`);
}

main();
