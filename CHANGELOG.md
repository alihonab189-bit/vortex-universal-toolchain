# CHANGELOG

## v0.2.0 — Expansion Release (কাজ করা অবস্থায়)

**এক রিলিজে ৮টি নতুন সাবসিস্টেম — সব ১০০% Rust std, শূন্য third-party crate।**

### ভাষা (Language)
- **Array**: `[১, ২, ৩]`, index (ঋণাত্মক সহ: `a[-1]`), `a[i] = v`, `push`/`pop`
- **Map**: `{"নাম": "ৰাম"}`, insertion-order, string key, `m["k"] = v`, `has`/`keys`/`values`
- **for-in**: array, string (অক্ষর), map-keys — পরিসর-loop-ও আগের মতো
- **break / continue** — তিন ভাষায়: `বিৰাম/বিরতি`, `আগুৱাই/এগিয়ে`
- **String methods**: `upper` `lower` `trim` `split` `contains` `replace` `chars` `join` `repeat` `sub`
- **Math builtins**: `floor` `ceil` `abs` `min` `max` `sqrt` `pow`
- **type()**, variadic `show()`, deep `==` (array/map)
- **ভারতীয় অংক** সরাসরি কোডে: `৪২`, `২.৫`
- VM refactored: step-machine (`vm::Machine`) + `execute_fn` (test runner-এর ভিত্তি)

### নিজস্ব WASM Backend (LLVM ছাড়া)
- হাতে লেখা binary encoder: LEB128, section writer, opcode assembler
- 16-byte tagged box runtime + ~৭০টি helper ফাংশন — সব raw wasm bytecode
- Export: `memory`, `vortex_main`, `vortex_output`
- Node.js-এর প্রকৃত WebAssembly runtime-এ চালিয়ে যাচাই করা ✅

### AI Engine
- অসমীয়া/বাংলা/English বাক্য → VORTEX কোড (লিপি শনাক্ত করে, অংকও লিপিতে)
- ১২টি intent: sum, factorial, fibonacci, নামতা, even/odd, ভোট, reverse, গড়, বর্গ, vowel…
- `--run` দিয়ে সাথে সাথে চালানো; `vortexc explain` — কোড → ব্যাখ্যা

### Package Manager (vpm)
- নিজস্ব `vortex.pkg` manifest parser
- নিজস্ব `.vpa` TLV archive + নিজস্ব CRC32 (corruption ধরা পড়ে)
- `init/build/pack/install/list/tree` + topo-sort resolver + cycle detection

### Debugger / Formatter / Linter / Test Runner
- `vortexc debug`: VIR-level stepping, breakpoints, stack/locals/frames inspection
- `vortexc fmt`: idempotent formatter (else-if chain সহ)
- `vortexc lint`: L001–L006 নিয়ম
- `vortexc test`: `test_*` ফাংশন + `assert`/`assert_eq`

### অন্যান্য
- Browser playground (`playground/index.html`) — নিজস্ব WASM চালায়
- টেস্ট: 28 → **64** (সব সবুজ), নতুন docs: WASM/AI/PACKAGE_MANAGER/TOOLS

## v0.1.0 — Language Engine MVP

- Lexer (Unicode, matra/hasanta), recursive-descent parser, AST
- Compiler → VIR (stack bytecode), VIR text serializer (.vxc)
- Stack VM (call frames, 10k depth), দ্বিভাষিক ত্রুটি-বার্তা
- তিন-ভাষার keyword: English + অসমীয়া + বাংলা
- CLI: run/ir/ast/tokens/build/exec/repl/keywords
- ২৮টি e2e টেস্ট + GitHub Actions CI
