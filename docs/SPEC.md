# VORTEX Language Specification — v0.1

> VORTEX-এর একটি প্রোগ্রাম `.vx` ফাইলে থাকে, UTF-8 encoding-এ।
> English, অসমীয়া ও বাংলা — যেকোনো ভাষার keyword মেশানো যায়, একই প্রোগ্রামেও।

---

## 1. Keyword টেবিল

| Canonical | English | অসমীয়া | বাংলা |
|---|---|---|---|
| `let` | `let` | `ধৰো` | `ধরো` |
| `fn` | `fn` | `কাম` | `কাজ` |
| `if` | `if` | `যদি` | `যদি` |
| `else` | `else` | `নহলে` | `নাহলে` |
| `while` | `while` | `যতক্ষণ` | `যতক্ষণ` |
| `for` | `for` | `লুপ` | `লুপ` |
| `in` | `in` | `ভিতৰত` | `মধ্যে` |
| `return` | `return` | `দিয়ক` | `দিয়া` |
| `true` | `true` | `সঁচা` | `সত্য` |
| `false` | `false` | `মিথ্যা` | `মিথ্যা` |

### Built-in ফাংশন

| Canonical | Aliases | কাজ |
|---|---|---|
| `show(x)` | `দেখাও`, `দেখুৱাওক` | মান প্রিন্ট করে |
| `len(s)` | `দৈৰ্ঘ্য`, `দৈর্ঘ্য` | string-এর character সংখ্যা |
| `str(x)` | `স্ট্রিং`, `স্ট্ৰিং` | যেকোনো মান → string |
| `num(s)` | `সংখ্যা` | string → int/float |

ব্যবহারকারী নিজের ফাংশন বানালে একই নামের built-in-কে override করে (call resolution-এ user ফাংশন আগে দেখা হয়)।

## 2. Lexer নিয়ম

- **Identifier**: শুরু হতে হয় letter (Unicode `Alphabetic` — বাংলা/অসমীয়া অক্ষর সহ) বা `_` দিয়ে; পরের অক্ষরে alphanumeric, `_` এবং ভারতীয় লিপির matra/hasanta/nukta (combining marks) চলে। অসমীয়া `ৰ` (U+09F0) ও `ৱ` (U+09F1) সরাসরি চলে।
- **সংখ্যা**: শুধু ASCII digit। Integer (`42`) ও float (`3.14`)।
- **String**: `"..."` — escape: `\n` `\t` `\r` `\0` `\\` `\"`।
- **Comment**: `//` লাইন শেষ পর্যন্ত; `/* ... */` block (nested নয়)।
- **Whitespace**: সব Unicode whitespace স্কিপ হয়।

## 3. Grammar (EBNF)

```ebnf
program   = stmt* ;
stmt      = "let" IDENT "=" expr ";"
          | IDENT "=" expr ";"            (* assignment — আগে let করতে হবে *)
          | "if" expr block ( "else" ( block | if_stmt ) )?
          | "while" expr block
          | "for" IDENT "in" expr ".." expr block
          | "fn" IDENT "(" params? ")" block
          | "return" expr? ";"
          | expr ";" ;
block     = "{" stmt* "}" ;
params    = IDENT ( "," IDENT )* ;

expr      = or_expr ;
or_expr   = and_expr ( "||" and_expr )* ;      (* সবচেয়ে কম precedence *)
and_expr  = cmp_expr ( "&&" cmp_expr )* ;
cmp_expr  = add_expr ( ( "==" | "!=" | "<" | ">" | "<=" | ">=" ) add_expr )? ;
add_expr  = mul_expr ( ( "+" | "-" ) mul_expr )* ;
mul_expr  = unary ( ( "*" | "/" | "%" ) unary )* ;
unary     = ( "-" | "!" ) unary | primary ;
primary   = INT | FLOAT | STRING | "true" | "false"
          | IDENT | IDENT "(" args? ")" | "(" expr ")" ;
args      = expr ( "," expr )* ;
```

- Comparison **non-associative**: `a < b < c` লিখলে parse error (চাইলে bracket দাও)।
- Function শুধু **top level**-এ ঘোষণা করা যায়।

## 4. Types ও Semantics

| টাইপ | উদাহরণ | নোট |
|---|---|---|
| `int` | `42`, `-7` | 64-bit; overflow-এ runtime error |
| `float` | `3.14`, `2.0` | 64-bit IEEE |
| `string` | `"নমস্কাৰ"` | UTF-8 |
| `bool` | `true`, `false` | শর্তে শুধু bool চলে |
| `unit` | `show(x)`-এর ফল | "কিছু নেই" বোঝায় |

v0.1 **dynamically typed** — টাইপ ভুল runtime-এ ধরা পড়ে (বাংলা+English দ্বিভাষিক error message সহ)। Static type checking v0.2-এর পথে।

### অপারেটর

- `+`: সংখ্যা যোগ; **string থাকলে concat** — `"মোট: " + 1000` → `"মোট: 1000"`
- `-`, `*`: সংখ্যা
- `/`: সবসময় **float division** — `7 / 2` → `3.5`; শূন্য দিয়ে ভাগ → error
- `%`: modulo (int/float)
- `==`, `!=`: সব টাইপ; আলাদা টাইপ হলে `false`
- `<`, `>`, `<=`, `>=`: সংখ্যা ও string (lexicographic)
- `&&`, `||`: **short-circuit** — `x != 0 && 10 / x > 1` নিরাপদ
- `!`, unary `-`

### Scope ও ফাংশন

- `let`-এ scope শুরু হয়; ভেতরের block-এ shadow করা যায়, একই scope-এ দুবার নয়।
- ফাংশন **hoist** করা হয় — ঘোষণার আগে কল করলেও চলে; recursion ও mutual recursion চলে।
- ফাংশন শুধু নিজের parameter ও local দেখে (main-এর local নয়)।
- `return` ফাংশনের যেকোনো জায়গায় চলে; ফাংশন শেষে না গেলে `unit` রিটার্ন হয়।
- Top-level `return` = প্রোগ্রাম শেষ।

### লুপ

```vortex
for i in 1..5 { show(i); }      // 1,2,3,4 — শেষের সংখ্যা বাদ
let i = 0;
while (i < 5) { i = i + 1; }
```

for-লুপের `i` শুধু লুপের ভেতরে থাকে; bounds একবারই মূল্যায়ন হয়।

## 5. ত্রুটি (Errors)

তিন ধাপে ত্রুটি ধরা পড়ে, প্রতিটিতে ভুল জায়গা (লাইন/কলাম) দেখায়:

1. **Lexer**: অজানা অক্ষর, unterminated string
2. **Parser**: syntax ভুল — `';' প্রত্যাশিত...`
3. **Compiler**: undefined variable, unknown function, arity mismatch, duplicate declaration, nested fn
4. **Runtime**: division by zero, overflow, ভুল টাইপের অপারেশন, stack overflow (call depth 10,000)

## 6. উদাহরণ

```vortex
// একই লজিক তিন ভাষায়:
if (n % 2 == 0) { show("even"); } else { show("odd"); }
যদি (n % 2 == 0) { দেখাও("জোড়"); } নহলে { দেখাও("বিজোড়"); }
যদি (n % 2 == 0) { দেখুৱাওক("যুগ্ম"); } নহলে { দেখুৱাওক("অযুগ্ম"); }
```

## 7. v0.2-এ যা আসছে (planned)

Static type system (`let x: int = 5`), arrays ও maps, string methods, `break`/`continue`, error handling (`try`/`catch`), modules ও imports, generics, struct/class। Custom keyword aliases user-configurable হবে — যেকোনো ভারতীয় ভাষায় VORTEX লেখা সম্ভব হবে।
