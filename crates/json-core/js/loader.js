// loader.js — zero-dependency loader for json_core.wasm
// Usage (browser):
//   import { init, validate } from "./loader.js";
//   await init(fetch("/json_core.wasm"));
//   const { ok, diagnostics } = validate(textareaValue, { mode: "jsonc" });
// Usage (Node):
//   await init(fs.readFileSync("json_core.wasm"));

let wasm = null;
const enc = new TextEncoder();
const dec = new TextDecoder();

export async function init(source) {
  if (wasm) return;
  let module;
  if (source instanceof Promise || (typeof Response !== "undefined" && source instanceof Response)) {
    module = await WebAssembly.instantiateStreaming(await source, {});
  } else {
    module = await WebAssembly.instantiate(source, {});
  }
  wasm = module.instance.exports;
}

export function validate(text, { mode = "strict" } = {}) {
  if (!wasm) throw new Error("call init() first");
  const bytes = enc.encode(text);

  const inPtr = wasm.jc_alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer, inPtr, bytes.length).set(bytes);

  const outPtr = wasm.jc_validate(inPtr, bytes.length, mode === "jsonc" ? 1 : 0);

  const lenView = new DataView(wasm.memory.buffer, outPtr, 4);
  const len = lenView.getUint32(0, true);
  const report = dec.decode(new Uint8Array(wasm.memory.buffer, outPtr + 4, len));

  wasm.jc_dealloc(inPtr, bytes.length);
  wasm.jc_dealloc(outPtr, len + 4);

  return JSON.parse(report);
}
