# VORTEX Language Specification — v0.2

> VORTEX-এর একটি প্রোগ্রাম `.vx` ফাইলে থাকে, UTF-8 encoding-এ।
> English, অসমীয়া ও বাংলা — যেকোনো ভাষার keyword মেশানো যায়, একই প্রোগ্রামেও।
> সংখ্যাও দুই স্টাইলে লেখা যায়: `42` বা `৪২`।

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
| `break` | `break` | `বিৰাম` | `বিরতি` |
| `continue` | `continue` | `আগুৱাই` | `এগিয়ে` |

### Built-in ফাংশন (v0.2)

| Canonical | Aliases | কাজ |
|---|---|---|
| `show(...)` | `দেখাও`, `দেখুৱাওক` | একাধিক মান স্পেস দিয়ে প্রিন্ট (variadic) |
| `len(x)` | `দৈৰ্ঘ্য`, `দৈর্ঘ্য` | string/array/map-এর দৈর্ঘ্য |
| `str(x)` | `স্ট্রিং`, `স্ট্ৰিং` | যেকোনো মান → string |
| `num(s)` | `সংখ্যা` | string → int/float |
| `push(a, v)` | `যোগ`, `জোড়া` | array-র শেষে যোগ |
| `pop(a)` | `বাহৰ` | শেষ উপাদান বের করে |
| `keys(m)` | `চাবি`, `চাবিকাঠি` | map-এর সব চাবি (array) |
| `values(m)` | `মানবোৰ`, `মান` | map-এর সব মান (array) |
| `has(m, k)` | `আছে` | চাবি আছে কি না |
| `type(x)` | `ধৰণ`, `প্রকার` | টাইপের নাম: int/float/string/bool/array/map/unit |
| `items(x)` | `তালিকা` | for-in-এর জন্য array রূপে |
| `floor(x)` `ceil(x)` `abs(x)` `min(a,b)` `max(a,b)` `sqrt(x)` `pow(b,e)` | — | গণিত |
| `assert(c)` `assert_eq(a, b)` | `নিশ্চিত` | টেস্ট assertion |

## 2. মানের ধরন (Value types)

`int` (i64), `float` (f64), `string`, `bool`, `array`, `map`, `unit`।

- `+` string-এর সাথে দেখলে সবকিছু concat করে: `"মোট: " + ১০০০`
- `/` সবসময় float দেয়; `%` শুধু int-এ
- array/map হলো reference — `push` করলে মূলটাই বদলায়
- `==` array/map-এর উপর deep equality করে

## 3. নতুন সিনট্যাক্স (v0.2)

### Array

```vortex
let a = [১, ২, ৩];
push(a, ৪);
a[0] = ১০;
show(a[0]);      // ১০
show(a[-1]);     // ঋণাত্মক index = পেছন থেকে
```

### Map (insertion-order সহ)

```vortex
let m = {"নাম": "ৰাম", "বয়স": ২৫};
m["শহৰ"] = "গুৱাহাটী";      // নতুন key যোগ / update
show(m["নাম"]);
show(has(m, "বয়স"));        // true
```

### for-in তিন ধরনের সংগ্রহে চলে

```vortex
for i in ১..৫ { }            // সংখ্যা-পরিসর
for x in [১০, ২০] { }        // array
for c in "অসমীয়া" { }        // string (অক্ষর)
for k in {"a": ১} { }        // map → keys
```

### Method কল (string/array/map-এর উপর)

```vortex
"hello".upper();   "WORLD".lower();   "  নমস্কাৰ  ".trim();
"a,b".split(",");   "hello".contains("ell");
"abcd".replace("b", "X");   "নমস্কাৰ".chars();   "abc".sub(1, 2);
"ab".repeat(৩);   [১,২,৩].join("-");   a.len();   a.push(x);   a.pop();
m.keys();   m.values();   m.has(k);
```

## 4. পূর্ণ ব্যাকরণ (EBNF, v0.2)

```ebnf
program  = stmt*
stmt     = "let" IDENT "=" expr ";"
         | IDENT "=" expr ";"
         | expr_index "=" expr ";"        ; a[0] = v / m["k"] = v
         | "if" expr block ("else" (block | if_stmt))?
         | "while" expr block
         | "for" IDENT "in" (expr ".." expr | expr) block
         | "fn" IDENT "(" params? ")" block
         | "return" expr? ";"
         | "break" ";" | "continue" ";"
         | expr ";"
expr     = or_expr
or_expr  = and_expr { "||" and_expr }
and_expr = cmp_expr { "&&" cmp_expr }
cmp_expr = add_expr [ ("=="|"!="|"<"|">"|"<="|">=") add_expr ]
add_expr = mul_expr { ("+"|"-") mul_expr }
mul_expr = postfix { ("*"|"/"|"%") postfix }
postfix  = primary { "[" expr "]" | "." IDENT "(" args? ")" }
primary  = INT | FLOAT | STRING | "true" | "false" | IDENT
         | "(" expr ")" | "[" items? "]" | "{" pairs? "}"
         | IDENT "(" args? ")" | "-" postfix | "!" postfix
```

## 5. ত্রুটি বার্তা

সব error দ্বিভাষিক (বাংলা + English), stage সহ:
`[lexer|parser|compiler|runtime] ত্রুটি (error) [লাইন L, কলাম C]: ...`
