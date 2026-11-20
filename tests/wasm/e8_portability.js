#!/usr/bin/env node
// **E8：wasm 可移植子集的 canonical 原始产物**（`PLAN §12.26` P1）。
//
// 对 `tests/portability/wasm_subset.txt` 里每个脚本：**新建实例**（隔离）→ 读脚本 → 执行 →
// 由宿主 `fs_write` 收走 stdout → 产出 `{results: {脚本: {exit, stdout, stderr}}}` 原始 JSON。
// **归一化不在本脚本做**：交给 `tests/portability/runner.py --raw-json`（一处真相）。
//
// 用法：node tests/wasm/e8_portability.js [wasm 路径] --out raw.json
'use strict';

const fs = require('fs');
const path = require('path');

const PA_OK = 0;
const PA_ERR_RUNTIME = 1;

function fail(message) {
  console.error(`E8 FAIL: ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const args = { wasm: null, out: null };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--out') {
      args.out = argv[index + 1];
      index += 1;
    } else if (!args.wasm) {
      args.wasm = argv[index];
    }
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const repo = path.join(__dirname, '..', '..');
  const wasmPath =
    args.wasm ||
    path.join(repo, 'target', 'wasm32-unknown-unknown', 'debug', 'pyawa_runtime.wasm');
  if (!fs.existsSync(wasmPath)) fail(`找不到 wasm 产物：${wasmPath}`);
  const subsetPath = path.join(repo, 'tests', 'portability', 'wasm_subset.txt');
  const scripts = fs
    .readFileSync(subsetPath, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#'));

  let current = null; // { stdout: Buffer[], stderr: Buffer[] }
  const imports = {
    pyawa_host: {
      pa_host_slot_call: () => 7, // 本子集不注册宿主函数
      clock_now_ns: () => 0n,
      clock_monotonic_ns: () => 0n,
      clock_sleep_ns: () => {},
      fs_write: (handle, ptr, len) => {
        if (!current) return 0;
        const bytes = Buffer.from(new Uint8Array(wasm.memory.buffer).slice(ptr, ptr + len));
        if (handle === 2) current.stderr.push(bytes);
        else current.stdout.push(bytes);
        return 0;
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
    const chunk = cstr('<e8>');
    const mode = cstr('python');
    const rc = wasm.pa_exec_string(state, sourceHandle.ptr, -1, chunk.ptr, mode.ptr, 0) | 0;
    freeCString(sourceHandle);
    freeCString(chunk);
    freeCString(mode);
    return { rc, message: errmsg(state) };
  }

  const results = {};
  for (const script of scripts) {
    const source = fs.readFileSync(path.join(repo, 'tests', 'portability', 'corpus', script), 'utf8');
    const state = wasm.paL_newstate();
    if (!state) fail(`paL_newstate 返回 NULL（脚本 ${script}）`);
    current = { stdout: [], stderr: [] };
    try {
      const run = exec(state, source);
      const stdout = Buffer.concat(current.stdout).toString('utf8').split('\n');
      const stderr = Buffer.concat(current.stderr).toString('utf8').split('\n');
      if (run.rc !== PA_OK) stderr.push(run.message);
      // 去掉末尾空行（runner.py 的 splitlines 语义也在归一化时处理；这里保证不产出多余尾行）。
      const trim = (lines) => {
        while (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
        return lines;
      };
      results[script] = {
        exit: run.rc === PA_OK ? 0 : run.rc,
        stdout: trim(stdout),
        stderr: trim(stderr).filter((line) => line !== ''),
      };
    } finally {
      current = null;
      wasm.pa_destroy(state);
    }
  }

  const document = {
    meta: { platform: 'wasm32', os_name: 'wasm', corpus: scripts.length, binary: wasmPath },
    results,
  };
  const text = JSON.stringify(document, null, 2);
  if (args.out) {
    fs.writeFileSync(args.out, text + '\n');
    console.log(`E8 OK: wasm 可移植子集 ${scripts.length} 个脚本 → ${args.out}`);
  } else {
    process.stdout.write(text + '\n');
  }
}

main();
