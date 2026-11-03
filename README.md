<p align="center">
  <img src="assets/vortex-logo.png" alt="VORTEX Universal Toolchain" width="440"/>
</p>

# VORTEX UNIVERSAL TOOLCHAIN 🌀

> **ভাষা, কম্পাইলার, VIR, VM, CLI — সব নিজের।**
> একটিও third-party crate নেই — পুরো টুলচেইন ১০০% Rust std দিয়ে শূন্য থেকে বানানো।

**বর্তমান সংস্করণ: v0.1.0 — Language Engine MVP (কাজ করছে ✅)**

---

## ⚡ এই মুহূর্তে যা কাজ করে

- **নিজস্ব ভাষা VORTEX** — English, অসমীয়া (Assamese) ও বাংলা (Bengali) keyword একসাথে চলে
- **Lexer** — Unicode identifier (ৰ, ৱ, matra/hasanta সহ), string escape, line/block comment
- **Recursive-descent Parser** → AST
- **Compiler** → **VIR** (Vortex Intermediate Representation) — stack-based bytecode
- **VIR serializer** — compile করে `.vxc` artifact, পরে সরাসরি চালানো যায়
- **VORTEX VM** — call frames, slot-based locals, 10,000-depth call stack, step limit
- **Short-circuit `&&` / `||`**, recursion, mutual recursion, else-if chain
- **Int + Float অ্যারিথমেটিক**, string concat (`"মোট: " + 1000`), comparison, logical ops
- **CLI**: `run` `ir` `ast` `tokens` `build` `exec` `repl` `keywords`
- **২৮টি টেস্ট** সব সবুজ ✅ + GitHub Actions CI

## 🚀 Build ও Run

```bash
cargo build --release
cargo run --release -- run examples/hello.vx
```

দরকার শুধু Rust (edition 2021)। অন্য কিছু নয়।

## 🌐 তিন ভাষায় একই ভাষা — এটাই VORTEX-এর পরিচয়

**English keywords** (`examples/hello.vx`):

```vortex
show("Hello, VORTEX!");
show("নমস্কাৰ, পৃথিৱী!");
```

```
Hello, VORTEX!
নমস্কাৰ, পৃথিৱী!
```

**অসমীয়া keywords** (`examples/assamese_hello.vx`):

```vortex
ধৰো নাম = "অসম";
ধৰো বয়স = 50;

যদি (বয়স >= 18) {
    দেখুৱাওক(নাম + " — ভোট দিব পাৰে");
} নহলে {
    দেখুৱাওক("সৰু ছাত্র");
}

লুপ i ভিতৰত 1..4 {
    দেখুৱাওক("ঘূৰণ " + i);
}

কাম যোগফল(a, b) {
    দিয়ক a + b;
}

দেখুৱাওক("১০ + ২০ = " + যোগফল(10, 20));
```

```
অসম — ভোট দিব পাৰে
ঘূৰণ 1
ঘূৰণ 2
ঘূৰণ 3
১০ + ২০ = 30
```

**বাংলা keywords** (`examples/bengali_demo.vx`):

```vortex
ধরো দাম = 250;
ধরো সংখ্যা = 4;
ধরো মোট = দাম * সংখ্যা;

যদি (মোট > 900) {
    দেখাও("মোট: " + মোট + " — বেশি!");
} নাহলে {
    দেখাও("মোট: " + মোট);
}
```

```
মোট: 1000 — বেশি!
ধাপ 1
ধাপ 2
ধাপ 3
```

## 📖 ভাষার ঝলক

```vortex
// recursion — fib(10)
fn fib(n) {
    if (n < 2) { return n; }
    return fib(n - 1) + fib(n - 2);
}
show(fib(10));        // 55

// string + সংখ্যা সরাসরি জোড়া লাগে
show("fib(7) = " + fib(7));   // fib(7) = 13

// for, while, else-if — সব আছে
for i in 1..6 { show(i); }
let n = 3;
while (n > 0) { show("countdown " + n); n = n - 1; }
```

সম্পূর্ণ spec: [`docs/SPEC.md`](docs/SPEC.md) • keyword টেবিল দেখতে: `vortexc keywords`

## 🏗️ Architecture

```
            examples/hello.vx
                   │
                   ▼
        ┌─────────────────────┐
        │       LEXER         │  character → token
        └──────────┬──────────┘
                   ▼
        ┌─────────────────────┐
        │  PARSER (recursive- │  token → AST
        │      descent)       │
        └──────────┬──────────┘
                   ▼
        ┌─────────────────────┐
        │      COMPILER       │  AST → VIR bytecode
        │  (scope + slot      │  (compile-time-এ undefined var/
        │   allocation)       │   unknown fn / arity error ধরা পড়ে)
        └──────────┬──────────┘
                   ▼
        ┌─────────────────────┐
        │   VIR (.vxc ফাইল)   │  টেক্সট ফরম্যাটে serialize
        └──────────┬──────────┘
                   ▼
        ┌─────────────────────┐
        │    VORTEX VM        │  stack machine + call frames
        └─────────────────────┘
```

`vortexc ir` দিয়ে VIR চোখে দেখা যায়:

```vir
# VORTEX VIR v1
FUNC main 0 0
CONST S Hello, VORTEX!
CALLB show 1
POP
CONST S নমস্কাৰ, পৃথিৱী!
CALLB show 1
POP
CONST U
RET
```

## 🛠️ CLI কমান্ড

| কমান্ড | কাজ |
|---|---|
| `vortexc run file.vx` | প্রোগ্রাম চালাও |
| `vortexc ir file.vx` | VIR bytecode দেখাও |
| `vortexc ast file.vx` | AST দেখাও |
| `vortexc tokens file.vx` | token list দেখাও |
| `vortexc build file.vx -o out.vxc` | VIR artifact বানাও |
| `vortexc exec out.vxc` | artifact সরাসরি চালাও |
| `vortexc repl` | interactive REPL (let/fn persist হয়) |
| `vortexc keywords` | সব keyword ও built-in টেবিল |

REPL-এ লাইভ:

```
VORTEX> 1 + 2
= 3
VORTEX> ধৰো x = 40
✓ x
VORTEX> x * 2
= 80
VORTEX> fn double(n) { দিয়ক n * 2; }
✓ double
VORTEX> double(21)
= 42
```

## 🗺️ Roadmap — এক নজরে

পুরো ৩২-ক্যাটাগরি পরিকল্পনা: [`docs/ROADMAP.md`](docs/ROADMAP.md)

| Phase | লক্ষ্য | অবস্থা |
|---|---|---|
| **P0** | Language Engine + Compiler (VIR) + VM + CLI | ✅ **এই repo** |
| P1 | ভাষা বড় করা: static typing, arrays, maps, error handling, modules | ⏳ |
| P2 | নিজস্ব WASM backend + browser playground + নিজস্ব web editor | ⏳ |
| P3 | নিজস্ব native backend (EXE/ELF) — LLVM optional | ⏳ |
| P4 | Visual Design Engine + নিজস্ব UI render engine | ⏳ |
| P5 | AI Engine (নিজের ভাষায় কথা বললেই কোড), Cloud build, Package manager | ⏳ |
| P6 | Marketplace, Developer accounts, Admin center | ⏳ |

## 📁 Project structure

```
vortex-universal-toolchain/
├── Cargo.toml          # কোনো dependency নেই
├── src/
│   ├── token.rs        # token সংজ্ঞা
│   ├── keywords.rs     # multilingual keyword টেবিল
│   ├── lexer.rs        # tokenizer (Unicode-aware)
│   ├── parser.rs       # recursive-descent parser
│   ├── ast.rs          # AST + pretty printer
│   ├── compiler.rs     # AST → VIR (scope/slot allocation)
│   ├── vir.rs          # VIR + টেক্সট serializer/parser
│   ├── value.rs        # runtime values
│   ├── vm.rs           # stack VM
│   ├── lib.rs          # library entry
│   └── main.rs         # CLI + REPL
├── tests/e2e.rs        # ২৮টি end-to-end টেস্ট
├── examples/           # ৬টি প্রোগ্রাম (English/অসমীয়া/বাংলা)
└── docs/               # SPEC • VIR • ROADMAP
```

## 🧪 টেস্ট

```bash
cargo test        # 28 tests
```

---

© 2026 alihonab189-bit — VORTEX Universal Toolchain। সর্বস্বত্ব সংরক্ষিত।
