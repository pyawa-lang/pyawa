#!/usr/bin/env node
// **E3 验收：JS 经最小 ABI 建实例、执行脚本、取返回值与错误消息**（`PLAN §12.21` E3）。
//
// 与 E2 的区别：E2 用电池版 `paL_newstate` 演示宿主函数跨边界；这里走**纯 ABI 路径**
// （`pa_create` ＋ 宿主自己填 `pa_host`），验证 JS 可见的 `pa_*` 面、状态码与 `pa_errmsg`。
//
// 用法：node tests/wasm/e3_abi_surface.js [wasm 路径]
// 缺省：target/wasm32-unknown-unknown/debug/pyawa_runtime.wasm
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;
const PA_ERR_RUNTIME = 1;
const PA_ERR_SYNTAX = 2;

function fail(message) {
  console.error(`E3 FAIL: ${message}`);
  process.exit(1);
}

function main() {
  const wasmPath =
    process.argv[2] ||
    path.join(__dirname, '..', '..', 'target', 'wasm32-unknown-unknown', 'debug', 'pyawa_runtime.wasm');
  if (!fs.existsSync(wasmPath)) fail(`找不到 wasm 产物：${wasmPath}`);

  const module = new WebAssembly.Module(fs.readFileSync(wasmPath));
  const imports = {
    pyawa_host: {
      pa_host_slot_call: () => 6,
      clock_now_ns: () => 0n,
      clock_monotonic_ns: () => 0n,
      clock_sleep_ns: () => {},
      fs_write: () => 0,
    },
  };
  const wasm = new WebAssembly.Instance(module, imports).exports;

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
  function stackInt(state, index) {
    const out = wasm.pyawa_wasm_alloc(8);
    try {
      const rc = wasm.pa_tointeger(state, index, out) | 0;
      if (rc !== PA_OK) return [rc, null];
      return [PA_OK, Number(view().getBigInt64(out, true))];
    } finally {
      wasm.pyawa_wasm_free(out, 8);
    }
  }

  // ---- ABI 面可见性 ----
  const exports = Object.keys(wasm);
  const abiExports = exports.filter((name) => name.startsWith('pa_') || name.startsWith('paL_'));
  if (abiExports.length < 80) fail(`JS 可见的 pa_/paL_ 导出只有 ${abiExports.length} 个（期望 ≥ 80）`);
  for (const required of ['pa_create', 'pa_destroy', 'pa_exec_string', 'pa_getglobal', 'pa_tointeger', 'pa_errmsg']) {
    if (!exports.includes(required)) fail(`缺导出 ${required}`);
  }
  const versionPtr = wasm.pa_version();
  const versionText = versionPtr ? readCString(versionPtr) : '';
  const abiSize = wasm.pa_abi_size() >>> 0;
  const abiVersion = wasm.pa_abi_version() >>> 0;
  if (!versionText) fail('pa_version() 返回空');
  if (abiSize === 0 || abiVersion === 0) fail('pa_abi_size()/pa_abi_version() 返回 0');

  // ---- 用宿主 buffer 填 pa_host，再 pa_create ----
  const host = wasm.pyawa_wasm_alloc(abiSize);
  if (!host) fail('pa_host 分配失败');
  bytes().fill(0, host, host + abiSize);
  view().setUint32(host, abiSize, true); // size_t abi_size
  view().setUint32(host + 4, abiVersion, true); // uint32_t abi_version
  view().setUint32(host + 8, 0, true); // const void *capabilities = NULL（E3 不要能力域）
  const stateOut = wasm.pyawa_wasm_alloc(4);
  const created = wasm.pa_create(host, stateOut) | 0;
  const state = view().getUint32(stateOut, true);
  wasm.pyawa_wasm_free(stateOut, 4);
  wasm.pyawa_wasm_free(host, abiSize);
  if (created !== PA_OK) fail(`pa_create => ${created}`);
  if (!state) fail('pa_create 交出 NULL 实例');

  function exec(source) {
    const sourceHandle = cstr(source);
    const chunkHandle = cstr('<e3>');
    const modeHandle = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunkHandle.ptr, modeHandle.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunkHandle);
    freeCString(modeHandle);
    return { rc, message: errorMessage(state) };
  }
  function globalInt(name) {
    const nameHandle = cstr(name);
    const rc = wasm.pa_getglobal(state, nameHandle.ptr) | 0;
    freeCString(nameHandle);
    if (rc !== PA_OK) fail(`pa_getglobal(${name}) => ${rc}: ${errorMessage(state)}`);
    const [valueRc, value] = stackInt(state, -1);
    wasm.pa_pop(state, 1);
    if (valueRc !== PA_OK) fail(`pa_tointeger(${name}) => ${valueRc}`);
    return value;
  }

  const ok = exec('y = 6 * 7');
  if (ok.rc !== PA_OK) fail(`合法脚本执行失败 rc=${ok.rc}: ${ok.message}`);
  const y = globalInt('y');
  if (y !== 42) fail(`6 * 7 应为 42，实际 ${y}`);

  const syntax = exec('x = (');
  if (syntax.rc !== PA_ERR_SYNTAX) fail(`语法错应返回 PA_ERR_SYNTAX(2)，实际 ${syntax.rc}`);
  if (!syntax.message) fail('语法错没有 pa_errmsg 消息');

  const runtime = exec('z = no_such_name');
  if (runtime.rc !== PA_ERR_RUNTIME) fail(`未定义名应返回 PA_ERR_RUNTIME(1)，实际 ${runtime.rc}`);
  if (!runtime.message.includes('NameError')) fail(`运行时错消息应含 NameError，实际：${runtime.message}`);

  const stackTop = wasm.pa_gettop(state) | 0;
  if (stackTop !== 0) fail(`调用后虚拟栈应净空，实际 top=${stackTop}`);
  if ((wasm.pa_destroy(state) | 0) !== PA_OK) fail('pa_destroy 失败');

  console.log(`E3 PASS: JS 可见 pa_/paL_ 导出 ${abiExports.length} 个；pa_version()=${versionText}`);
  console.log(`E3 PASS: pa_host（${abiSize} 字节）＋ pa_create 建实例（abi_version=${abiVersion}）`);
  console.log(`E3 PASS: pa_exec_string 合法脚本 => PA_OK；读回 y = 6 * 7 = ${y}`);
  console.log(`E3 PASS: 语法错 => PA_ERR_SYNTAX(2) ＋ pa_errmsg 非空`);
  console.log(`E3 PASS: 未定义名 => PA_ERR_RUNTIME(1) ＋ 消息含 NameError`);
  console.log(`E3 PASS: 调用后虚拟栈净空（top=${stackTop}）；pa_destroy => PA_OK`);
}

main();
