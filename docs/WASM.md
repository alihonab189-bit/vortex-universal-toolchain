# নিজস্ব WASM Backend — LLVM ছাড়া WebAssembly

> vortexc-এর নিজস্ব WebAssembly emitter — **কোনো LLVM, binaryen, walrus বা
> third-party crate নেই**। পুরো binary encoder হাতে লেখা: LEB128, section
> writer, opcode assembler — সব এক ফাইলে (`src/wasm.rs`)।

## পাইপলাইন

```
source.vx → Lexer → Parser → AST → (নিজস্ব WASM backend) → .wasm বাইনারি
```

ব্যবহার:

```bash
vortexc wasm examples/data_structures.vx -o out.wasm
```

## Runtime মডেল (নিজস্ব ABI)

প্রতিটি VORTEX মান linear memory-তে একটি **16-বাইট box**:

```
{ tag: i32, pad: i32, bits: i64 }
```

- Tag: `0=int 1=float 2=string 3=bool 4=array 5=map 6=unit`
- String: `bits` = ptr → `{len: i32, pad, bytes...}`
- Array: `bits` = ptr → `{len, cap, cells_ptr}`
- Map: `bits` = ptr → `{len, cap, entries_ptr}` (entry = 32B: key-box + value-box)

wasm স্ট্যাকে শুধু box-এর pointer চলে — তাই টাইপ-তথ্য হারায় না।
Runtime helpers (alloc, memcpy, string concat, int→string, float→string,
deep equality, map ops, output buffer) — **প্রতিটি নিজে হাতে লেখা wasm
bytecode**, module-এর ভেতরেই embedded।

## Export

| নাম | স্বাক্ষর | কাজ |
|---|---|---|
| `memory` | memory | linear memory |
| `vortex_main` | `() -> i32` | প্রোগ্রাম চালায়, result box-এর ptr |
| `vortex_output` | `() -> i32` | show()-এর সব আউটপুট string হিসেবে |

ব্রাউজার/Node থেকে চালানো:

```js
const { instance } = await WebAssembly.instantiate(bytes, {});
instance.exports.vortex_main();
const p = Number(instance.exports.vortex_output());
const mem = new DataView(instance.exports.memory.buffer);
const len = mem.getInt32(p, true);
console.log(new TextDecoder().decode(new Uint8Array(instance.exports.memory.buffer, p + 8, len)));
```

`playground/index.html` — তৈরি করা যেকোনো `.wasm` লোড করে চালানোর
নিজস্ব playground (কোনো external লাইব্রেরি নেই)।

## v0.2-তে যা কাজ করে

int/float/bool/string অ্যারিথমেটিক, comparison (string byte-order সহ),
if/while/for (range + collection), break/continue, ফাংশন ও recursion,
array (create/index/push/pop/len, ঋণাত্মক index), map (create/set/get/has/
keys/values), string concat + `contains` + `chars`, `show` (variadic),
`num`/`str`/`type`/`items`, গণিত builtins, UTF-8-aware `len` ও `chars`।

## v0.2-এর স্পষ্ট সীমাবদ্ধতা

- `.upper()` / `.lower()` / `.trim()` / `.split()` / `.replace()` /
  `.join()` / `.repeat()` / `.sub()` — এখনো নেই (VM-এ আছে)। ব্যবহার
  করলে compile error-ই বলে দেবে "VM (vortexc run) ব্যবহার করুন"।
  কারণ: Unicode case-mapping ইত্যাদির টেবিল এখনো হাতে লেখা হয়নি।
- wasm-এ int অ্যারিথমেটিক wrap করে (VM-এ overflow trap)
- `num()` শুধু integer string পার্স করে
- float প্রিন্ট হয় ৬ দশমিক পর্যন্ত (শেষের শূন্য কাটা)

## যাচাই

- 6 unit test: LEB128 roundtrip, section layout, full-language compile,
  unsupported-method rejection
- প্রমাণ: একই বাইনারি Node.js-এর প্রকৃত WebAssembly runtime-এ চলে
  (উপরের export কমান্ড দেখুন) — 58 e2e-র সমান্তরাল আচরণ
