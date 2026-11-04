# VIR — Vortex Intermediate Representation

> VIR হলো VORTEX-এর নিজস্ব intermediate representation — stack-based bytecode।
> Compiler সব source-কে VIR-এ নামায়; VM শুধু VIR চালায়। ভবিষ্যতের WASM/native
> backend-ও VIR-ই ইনপুট নেবে — এক compiler, অনেক target।

## 1. প্রোগ্রাম গঠন

```
VProgram
├── funcs: Vec<Func>     — সব ফাংশন (user fns + main)
└── entry: usize         — main-এর index

Func
├── name: String
├── params: Vec<String>
├── nslots: usize        — এই ফাংশনে মোট local slot
└── code: Vec<Instr>
```

- Variable = slot index। Compiler compile-time-এ প্রতিটি নামকে slot-এ বাঁধে
  (scope অনুযায়ী) — তাই VM-এ নাম খোঁজার দরকার নেই, শুধু index।
- Call convention: caller argument-গুলো বাঁয় থেকে ডানে stack-এ push করে;
  `CALLF` হলে callee-র slot 0..argc ভরে যায়; `RET` মান ফেরত দেয়।

## 2. Instruction set

| Instruction | Operand | কাজ |
|---|---|---|
| `CONST I n` | int | stack-এ int push |
| `CONST F x` | float | stack-এ float push |
| `CONST S s` | escaped string | stack-এ string push |
| `CONST B b` | `true`/`false` | stack-এ bool push |
| `CONST U` | — | stack-এ unit push |
| `LD n` | slot | slot n-এর মান push |
| `ST n` | slot | pop করে slot n-এ রাখো |
| `ADD` `SUB` `MUL` `DIV` `MOD` | — | দুটো pop → ফল push |
| `NEG` `NOT` | — | unary |
| `EQ` `NE` `LT` `GT` `LE` `GE` | — | comparison → bool push |
| `DUP` | — | top-এর copy push |
| `POP` | — | top ফেলে দাও |
| `JMP n` | target | সরাসরি jump |
| `JPF n` | target | pop; false হলে jump |
| `JPT n` | target | pop; true হলে jump |
| `CALLF f a` | fn idx, argc | user ফাংশন কল |
| `CALLB b a` | builtin, argc | built-in কল (`show` `len` `str` `num`) |
| `RET` | — | pop → return value; frame unwind |

`JPF`/`JPT` operand-ও stack-এর মান pop করে — তাই short-circuit `&&`/`||`
`DUP` + `JPF/JPT` + `POP` দিয়ে নামানো হয়:

```
a && b  →  [a] DUP JPF L1 POP [b] JMP L2 ; L1: ; L2:
           (a=false হলে সেই false-ই ফলাফল)
```

## 3. টেক্সট ফরম্যাট

VIR একটি পাঠযোগ্য টেক্সট ফরম্যাটে serialize হয় — `.vxc` artifact হিসেবে
ফাইলে থাকে, আবার পড়েও চালানো যায়:

```bash
vortexc build examples/fibonacci.vx -o fib.vxc
vortexc exec fib.vxc
```

নিয়ম:

- প্রথম লাইন: `# VORTEX VIR v1` (comment — `#` দিয়ে যেকোনো লাইন comment)
- ফাংশন শুরু: `FUNC <name> <arity> <nslots>`
- প্রতিটি instruction এক লাইন; string-এ `\n` `\t` `\r` `\\` escape হয়
- line-এর শেষের space **সংরক্ষিত** (string constant-এর অংশ হতে পারে)

উদাহরণ (`vortexc ir examples/hello.vx`):

```
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

if/loop-সহ বড় উদাহরণ:

```
FUNC main 0 2
CONST I 10
ST 0
LD 0
CONST I 5
GT
JPF 10
CONST S big
CALLB show 1
POP
JMP 13
CONST S small
CALLB show 1
POP
CONST U
RET
```

## 4. কেন নিজস্ব VIR?

1. **এক compiler, অনেক backend** — ভবিষ্যতে WASM/native backend VIR-ই নেবে
2. **Compile-time safety** — undefined variable/fn compile-এই ধরা পড়ে
3. **Artifact-first** — `.vxc` ফাইল বিতরণ করা যায়, source ছাড়াই চালানো যায়
4. **Debuggable** — মানুষ-পঠনযোগ্য bytecode; `vortexc ir` দিয়ে যা ইচ্ছা দেখো
