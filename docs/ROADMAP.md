# VORTEX ROADMAP — মাস্টার প্ল্যান (৩২ ক্যাটাগরি → ৭ Phase)

> মূলনীতি: **সব নিজের হাতে** — নিজস্ব ভাষা, নিজস্ব compiler, নিজস্ব VM,
> নিজস্ব backend, নিজস্ব IDE engine, নিজস্ব cloud। Flutter/React-এর মতো
> framework-এ ভরসা নয়। প্রতিটি phase কাজ করা অবস্থায় থাকবে — কখনোই
> "সব বানানোর আগে কিছুই চলবে না" এমন ফাঁদে পা দেওয়া হবে না।

---

## ✅ Phase 0 — Language Engine MVP (**এই repo, v0.1.0**)

মূল পরিকল্পনার ক্যাটাগরি **#1 Language Engine** ও **#9 Compiler Engine**-এর ভিত্তি:

| উপাদান | অবস্থা |
|---|---|
| নিজস্ব ভাষা VORTEX — English + অসমীয়া + বাংলা keyword | ✅ |
| Lexer (Unicode, matra/hasanta সহ) | ✅ |
| Recursive-descent Parser → AST | ✅ |
| Compiler → **VIR** (নিজস্ব bytecode) | ✅ |
| VIR serializer → `.vxc` artifact + `exec` | ✅ |
| Stack VM (call frames, slot locals) | ✅ |
| CLI (`run` `ir` `ast` `tokens` `build` `exec` `repl` `keywords`) | ✅ |
| ২৮টি e2e টেস্ট + CI | ✅ |

## Phase 1 — ভাষা পূর্ণতা (v0.2–v0.4)

ক্যাটাগরি #1-এর বাকি অংশ — **v0.2.0-তে যা পূর্ণ হলো:** Array, Map,
for-in (array/string/map), break/continue, string methods, math builtins,
ভারতীয় অংক, variadic show, deep equality, step-machine VM ✅

**এখনো বাকি:**

- **Static type system** — `let x: int = 5`; compile-time type checking
- **Sets** + literal syntax
- **try/catch error handling**
- **Modules & namespaces** (`use math;`), **imports**
- **Structs, classes, inheritance, polymorphism, encapsulation**
- **Generics, interfaces, annotations, pattern matching**
- **Operator overloading**, async/await (VM scheduler সহ)
- **Custom keyword aliases** — যেকোনো ভারতীয় ভাষায় VORTEX (user-configurable keyword টেবিল)
- Macro system, documentation generator

## Phase 2 — নিজস্ব Web Backend + Editor (v0.5–v0.7)

ক্যাটাগরি **#3 Code Editor**, **#9**-এর WASM অংশ — **v0.2.0-তে ভিত্তি পড়ল:**
নিজস্ব WASM binary encoder (হাতে লেখা, Node.js-এ যাচাইকৃত ✅) আর
নিজস্ব playground পেজ ✅ — বাকি:

- **Browser-এ compiler** — নিজস্ব compiler নিজেই WASM-এ compile হয়ে browser-এ চলবে
- **নিজস্ব web editor** — vanilla JS/WASM-এ: syntax highlighting, auto-complete,
  error marker, minimap — কোনো Monaco/CodeMirror ছাড়া
- Language Server Protocol (VS Code-এও VORTEX চলবে — এটা সুবিধা, dependency নয়)

## Phase 3 — নিজস্ব Native Backend (v1.0)

ক্যাটাগরি **#9 Compiler Engine** সম্পূর্ণ, **#10 Universal Platform Builder**:

- **নিজস্ব native backend** — VIR → x86-64 / ARM64 machine code
- নিজস্ব linker + object format (Phase 6-এর advanced list থেকে)
- EXE (Windows) / ELF (Linux) / Mach-O (macOS) binary generation
- Cross-compilation, incremental compilation, compiler cache
- LLVM শুধু **optional accelerator** হিসেবে (মূল পরিকল্পনায় আছে; ভিত্তি নিজের)
- APK/AAB/IPA packaging — নিজস্ব build pipeline দিয়ে

## Phase 4 — Visual Design + UI Engine

ক্যাটাগরি **#4 Visual Design Engine**, **#5 Colour Studio**, **#6 UI Components**,
**#7 Typography & Graphics**, **#8 UI Render Engine**, **#24 Game Engine**:

- **নিজস্ব UI render engine** — GPU-accelerated, cross-platform
- Drag-and-drop design canvas — grid, constraint, auto-layout
- নিজস্ব component library + theme engine
- Animation engine — timeline, keyframe, particle
- Colour studio — picker, gradient, contrast checker, AI suggestion
- Game engine layer — physics, scene editor, asset manager

## Phase 5 — AI + Cloud + Security

ক্যাটাগরি **#2 Natural Language Programming**, **#17 Cloud**,
**#18 Security**, **#12 Package Manager**, **#14 Testing**, **#15 Database**:

- **AI Engine** — ✅ v0.2.0: নিজস্ব rule-based intent engine (তিন ভাষায়,
  ১২ intent, explain সহ) — কোনো external API ছাড়াই কাজ করছে; এর পরের
  ধাপ নিজস্ব neural স্তর
- Voice-to-code, code explanation, auto bug fix
- **নিজস্ব package manager** — ✅ v0.2.0: নিজস্ব manifest + .vpa archive +
  নিজস্ব CRC32 + topo-sort resolver (local); বাকি: নিজস্ব registry server, signing
- **Cloud build system** — distributed workers, build queue, artifact storage
- **Security engine** — source encryption, binary signing, license validation,
  tamper detection, vulnerability scanner
- Testing engine — unit/UI/load, coverage reports
- Database engine + API/network engine

## Phase 6 — Ecosystem (Advanced Technology)

ক্যাটাগরি **#33 Advanced Features** + ব্যবসা স্তর:

- **Self-hosted compiler** — VORTEX-ই VORTEX compiler লিখবে (bootstrapping)
- JIT compilation, PGO, LTO, reproducible builds
- নিজস্ব runtime, GC, memory allocator
- Marketplace (#28), Developer accounts (#29), Admin center (#30),
  Subscription & billing (#31), Documentation center (#32)
- Project manager, version control (নিজস্ব + Git interop), deployment engine

---

## Progress tracker

| Phase | ক্যাটাগরি কভারেজ | অবস্থা |
|---|---|---|
| P0 | #1 ভিত্তি, #9 ভিত্তি | ✅ সম্পূর্ণ (v0.1.0) |
| P1 | #1 সম্পূর্ণ | ⏳ |
| P2 | #3, #9-WASM | ⏳ |
| P3 | #9 সম্পূর্ণ, #10 | ⏳ |
| P4 | #4–8, #24 | ⏳ |
| P5 | #2, #12, #14–18 | ⏳ |
| P6 | #20–23, #26–33 | ⏳ |

**নিয়ম: প্রতিটি phase শেষে একটি বাস্তবে কাজ করা demo — সবসময় চলন্ত টুলচেইন।**
