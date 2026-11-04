# টুলচেইন — Debugger, Formatter, Linter, Test Runner

সবগুলো `vortexc`-এর নিজস্ব সাবকমান্ড — কোনো external tool নেই।

## Debugger — `vortexc debug <file.vx>`

VIR-level interactive debugger (নিজস্ব `vm::Machine` step API-র উপর):

```
(vdbg) কমান্ড:
  s / step      এক VIR instruction এগোও
  n <k>         k instruction এগোও
  c / continue  পরের breakpoint পর্যন্ত চালাও
  b <fn> <ip>   breakpoint বসাও (যেমন b main 5)
  bl / db       breakpoint list / delete
  st            value stack দেখাও
  lc            বর্তমান frame-এর locals
  fr            call frames
  ir            বর্তমান instruction
  out           এ পর্যন্ত show() আউটপুট
  q             quit
```

## Formatter — `vortexc fmt <file.vx> [-w]`

AST → পরিচ্ছন্ন কোড: 4-স্পেস ইনডেন্ট, `} else if` চেইন এক লাইনে,
স্পেস-নরমালাইজেশন। Idempotent — `fmt(fmt(x)) == fmt(x)` (টেস্টে প্রমাণিত)।
`-w` দিলে ফাইলেই লিখে দেয়।

## Linter — `vortexc lint <file.vx>`

| কোড | সমস্যা |
|---|---|
| L001 | ঘোষিত কিন্তু ব্যবহার হয়নি এমন variable |
| L002 | কোথাও কল হয়নি এমন ফাংশন |
| L003 | built-in-এর নামে variable (shadowing) |
| L004 | খালি block |
| L005 | return-এর পরে unreachable কোড |
| L006 | `while true`-এ break নেই — সম্ভাব্য অসীম লুপ |

সমস্যা থাকলে exit code 1 — CI-তে সরাসরি ব্যবহারযোগ্য।

## Test Runner — `vortexc test <file.vx>`

`test_*` নামের প্যারামিটার-বিহীন ফাংশনগুলো একে একে চালায়
(`vm::execute_fn` — নিজস্ব step-machine):

```vortex
fn test_add() {
    assert_eq(1 + 1, 2);
    assert(5 > 3);
}
```

আউটপুট: `✓ test_add` / `✗ test_fail — assert_eq ব্যর্থ (assertion failed): 1 != 2`
+ summary; ব্যর্থ থাকলে exit 1।
