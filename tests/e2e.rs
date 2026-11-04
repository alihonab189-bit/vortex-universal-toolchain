//! End-to-end টেস্ট: Source → Lexer → Parser → Compiler → VIR → VM

use vortexc::run_source;
use vortexc::value::Value;
use vortexc::vir;
use vortexc::{compiler, parser, Stage};


fn out(src: &str) -> Vec<String> {
    match run_source(src) {
        Ok((_, o, _)) => o,
        Err(e) => panic!("run failed: {}\nsource: {}", e, src),
    }
}

fn err_msg(src: &str) -> String {
    match run_source(src) {
        Ok(_) => panic!("expected error, got success: {}", src),
        Err(e) => e.msg,
    }
}

#[test]
fn hello() {
    assert_eq!(out("show(\"Hello, VORTEX!\");"), vec!["Hello, VORTEX!"]);
}

#[test]
fn precedence() {
    assert_eq!(
        out("show(2 + 3 * 4); show((2 + 3) * 4); show(10 % 3);"),
        vec!["14", "20", "1"]
    );
}

#[test]
fn string_concat_numbers() {
    assert_eq!(out("show(\"বয়স: \" + 21);"), vec!["বয়স: 21"]);
}

#[test]
fn float_division() {
    assert_eq!(out("show(7 / 2); show(2.5 + 1.5);"), vec!["3.5", "4.0"]);
}

#[test]
fn if_else_both_branches() {
    let code = r#"
        let x = 10;
        if (x > 5) { show("big"); } else { show("small"); }
        let y = 3;
        if (y > 5) { show("big"); } else { show("small"); }
    "#;
    assert_eq!(out(code), vec!["big", "small"]);
}

#[test]
fn else_if_chain() {
    let code = r#"
        let n = 2;
        if (n == 1) { show("one"); }
        else if (n == 2) { show("two"); }
        else { show("many"); }
    "#;
    assert_eq!(out(code), vec!["two"]);
}

#[test]
fn while_sum() {
    let code = "let s = 0; let i = 1; while (i <= 10) { s = s + i; i = i + 1; } show(s);";
    assert_eq!(out(code), vec!["55"]);
}

#[test]
fn for_range() {
    assert_eq!(out("for i in 1..4 { show(i); }"), vec!["1", "2", "3"]);
}

#[test]
fn recursion_fib() {
    let code = r#"
        fn fib(n) {
            if (n < 2) { return n; }
            return fib(n - 1) + fib(n - 2);
        }
        show(fib(10));
    "#;
    assert_eq!(out(code), vec!["55"]);
}

#[test]
fn mutual_recursion() {
    let code = r#"
        fn is_even(n) { if (n == 0) { return true; } return is_odd(n - 1); }
        fn is_odd(n) { if (n == 0) { return false; } return is_even(n - 1); }
        show(is_even(10)); show(is_odd(7));
    "#;
    assert_eq!(out(code), vec!["true", "true"]);
}

#[test]
fn assamese_keywords() {
    let code = r#"
        ধৰো নাম = "অসম";
        ধৰো বয়স = 50;
        যদি (বয়স >= 18) {
            দেখুৱাওক(নাম + " — ভোট দিব পাৰে");
        } নহলে {
            দেখুৱাওক("সৰু");
        }
        লুপ i ভিতৰত 1..4 {
            দেখুৱাওক("ঘূৰণ " + i);
        }
        কাম যোগ(a, b) { দিয়ক a + b; }
        দেখুৱাওক(যোগ(20, 22));
    "#;
    assert_eq!(
        out(code),
        vec!["অসম — ভোট দিব পাৰে", "ঘূৰণ 1", "ঘূৰণ 2", "ঘূৰণ 3", "42"]
    );
}

#[test]
fn bengali_keywords() {
    let code = r#"
        ধরো দাম = 250;
        ধরো সংখ্যা = 4;
        ধরো মোট = দাম * সংখ্যা;
        যদি (মোট > 900) {
            দেখাও("মোট: " + মোট + " — বেশি!");
        } নাহলে {
            দেখাও("মোট: " + মোট);
        }
    "#;
    assert_eq!(out(code), vec!["মোট: 1000 — বেশি!"]);
}

#[test]
fn short_circuit_and() {
    // x == 0 হলে 10 / x মূল্যায়ন হবে না — তাই error আসবে না
    let code = r#"
        fn safe(x) { return x != 0 && 10 / x > 1; }
        show(safe(0));
        show(safe(5));
    "#;
    assert_eq!(out(code), vec!["false", "true"]);
}

#[test]
fn short_circuit_or() {
    let code = "show(true || 1 / 0 == 1);";
    assert_eq!(out(code), vec!["true"]);
}

#[test]
fn builtins() {
    let code = r#"
        show(len("hello"));
        show(str(99) + "!");
        show(num("42") + 8);
    "#;
    assert_eq!(out(code), vec!["5", "99!", "50"]);
}

#[test]
fn unary_ops() {
    assert_eq!(out("show(-5 + 3); show(!false);"), vec!["-2", "true"]);
}

#[test]
fn comments_ignored() {
    let code = "// লাইন কমেন্ট\n/* block\ncomment */ show(\"ok\");";
    assert_eq!(out(code), vec!["ok"]);
}

#[test]
fn logical_mix() {
    let code = "show(true && false); show(false || true); show(1 < 2 && 2 <= 2);";
    assert_eq!(out(code), vec!["false", "true", "true"]);
}

#[test]
fn string_compare() {
    let code = "show(\"apple\" < \"banana\"); show(\"a\" == \"a\");";
    assert_eq!(out(code), vec!["true", "true"]);
}

// ---------- ত্রুটি কেস (error cases) ----------

#[test]
fn err_div_by_zero() {
    let m = err_msg("show(1 / 0);");
    assert!(m.contains("division by zero"), "got: {}", m);
}

#[test]
fn err_undefined_variable() {
    let m = err_msg("show(xyz);");
    assert!(m.contains("unknown variable"), "got: {}", m);
}

#[test]
fn err_unknown_function() {
    let m = err_msg("nope(1);");
    assert!(m.contains("unknown function"), "got: {}", m);
}

#[test]
fn err_arity_mismatch() {
    let m = err_msg("fn f(a) { return a; } show(f(1, 2));");
    assert!(m.contains("expects 1 arguments"), "got: {}", m);
}

#[test]
fn err_condition_not_bool() {
    let m = err_msg("if (5) { show(1); }");
    assert!(m.contains("condition must be"), "got: {}", m);
}

#[test]
fn err_syntax() {
    assert!(run_source("let = 5;").is_err());
    assert!(run_source("show(1)").is_err()); // missing ;
}

// ---------- VIR roundtrip ----------

#[test]
fn vir_roundtrip() {
    let src = "let x = 40; show(x + 2); fn twice(n) { return n * 2; } show(twice(21));";
    let (_, o1, text) = run_source(src).expect("run");
    let vp = vir::parse_text(&text).expect("parse VIR");
    let r = vortexc::vm::execute(&vp).expect("exec VIR");
    assert_eq!(r.output, o1);
    assert_eq!(r.output, vec!["42", "42"]);
}

#[test]
fn vir_string_escape_roundtrip() {
    let src = "show(\"line1\\nline2\"); show(\"trailing space: \");";
    let (_, o1, text) = run_source(src).expect("run");
    let vp = vir::parse_text(&text).expect("parse VIR");
    let r = vortexc::vm::execute(&vp).expect("exec VIR");
    assert_eq!(r.output, o1);
    assert_eq!(r.output, vec!["line1\nline2", "trailing space: "]);
}

// ---------- lexer: unicode identifiers ----------

#[test]
fn unicode_identifiers() {
    // ৰ (U+09F0) ও ৱ (U+09F1) সহ অসমীয়া identifier
    let code = "ধৰো ৰাম = 5; ৰাম = ৰাম + 10; show(ৰাম);";
    assert_eq!(out(code), vec!["15"]);
}

// ==================== v0.2 — Arrays, Maps, Methods, Control ====================

#[test]
fn v02_arrays() {
    let src = r#"
        let a = [10, 20, 30];
        push(a, 40);
        show(a);
        show(a[0] + a[-1]);
        show(len(a));
        let last = pop(a);
        show(last);
        show(len(a));
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec!["[10, 20, 30, 40]", "50", "4", "40", "3"]);
}

#[test]
fn v02_maps() {
    let src = r#"
        let m = {"নাম": "ৰাম", "বয়স": 25};
        show(m["নাম"]);
        m["শহৰ"] = "গুৱাহাটী";
        show(has(m, "শহৰ"));
        show(keys(m));
        show(values(m));
        show(len(m));
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out[0], "ৰাম");
    assert_eq!(out[1], "true");
    assert_eq!(out[2], "[\"নাম\", \"বয়স\", \"শহৰ\"]");
    assert_eq!(out[3], "[\"ৰাম\", 25, \"গুৱাহাটী\"]");
    assert_eq!(out[4], "3");
}

#[test]
fn v02_for_in_collections() {
    let src = r#"
        for x in [5, 6, 7] { show(x); }
        for c in "অসমীয়া" { show(c); }
        for k in {"k1": 1, "k2": 2} { show(k); }
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec!["5", "6", "7", "অ", "স", "ম", "ী", "য", "়", "া", "k1", "k2"]);
}

#[test]
fn v02_break_continue() {
    let src = r#"
        let i = 0;
        while true {
            i = i + 1;
            if i % 2 == 0 { continue; }
            if i > 7 { break; }
            show(i);
        }
        for n in 1..10 {
            if n > 3 { break; }
            show(n * 100);
        }
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec!["1", "3", "5", "7", "100", "200", "300"]);
}

#[test]
fn v02_string_methods() {
    let src = r#"
        let s = "  নমস্কাৰ  ";
        show(s.trim() + "|");
        show("hello".upper());
        show("WORLD".lower());
        show("a,b,c".split(","));
        show("hello".contains("ell"));
        show("abcd".replace("b", "X"));
        show("নমস্কাৰ".chars());
        show([1, 2, 3].join("-"));
        show("ab".repeat(3));
        show("নমস্কাৰ".sub(0, 3));
        show("hello".sub(-3, 2));
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec![
        "নমস্কাৰ|", "HELLO", "world",
        "[\"a\", \"b\", \"c\"]", "true", "aXcd",
        "[\"ন\", \"ম\", \"স\", \"্\", \"ক\", \"া\", \"ৰ\"]",
        "1-2-3", "ababab", "নমস",
        "ll",
    ]);
}

#[test]
fn v02_math_builtins() {
    let src = r#"
        show(floor(3.7));
        show(ceil(3.2));
        show(abs(-9));
        show(abs(-2.5));
        show(min(10, 3));
        show(max(10, 3));
        show(pow(2, 8));
        show(sqrt(81.0));
        show(num("42") + 1);
        show(type(5));
        show(type("x"));
        show(type([1]));
        show(type({"a": 1}));
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec![
        "3", "4", "9", "2.5", "3", "10", "256", "9.0",
        "43", "int", "string", "array", "map",
    ]);
}

#[test]
fn v02_index_assign() {
    let src = r#"
        let a = [1, 2, 3];
        a[1] = 99;
        show(a);
        a[-1] = 7;
        show(a);
        let m = {"x": 1};
        m["x"] = 5;
        m["y"] = 6;
        show(m);
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec!["[1, 99, 3]", "[1, 99, 7]", "{x: 5, y: 6}"]);
}

#[test]
fn v02_deep_equality() {
    let src = r#"
        show([1, [2, 3]] == [1, [2, 3]]);
        show([1, 2] != [1, 3]);
        show({"a": 1} == {"a": 1});
        show([1, 2] == [2, 1]);
    "#;
    let (_, out, _) = run_source(src).expect("run");
    assert_eq!(out, vec!["true", "true", "true", "false"]);
}

#[test]
fn v02_index_errors() {
    let src = "let a = [1]; show(a[5]);";
    let err = run_source(src).unwrap_err();
    assert_eq!(err.stage, Stage::Vm);
    assert!(err.msg.contains("সীমার বাইরে"));

    let src2 = "show({\"x\": 1}[\"missing\"]);";
    let err2 = run_source(src2).unwrap_err();
    assert!(err2.msg.contains("চাবি নেই"));
}

// ==================== v0.2 — AI Engine ====================

#[test]
fn v02_ai_sum_bengali() {
    let (code, intent) = vortexc::ai::generate("১ থেকে ১০ পর্যন্ত যোগফল দেখাও").expect("generate");
    assert_eq!(intent, "sum");
    let (_, out, _) = run_source(&code).expect("generated code runs");
    assert_eq!(out, vec!["যোগফল: 55"]);
}

#[test]
fn v02_ai_sum_english() {
    let (code, _) = vortexc::ai::generate("sum from 1 to 100").expect("generate");
    let (_, out, _) = run_source(&code).expect("runs");
    assert_eq!(out, vec!["sum: 5050"]);
}

#[test]
fn v02_ai_table_and_fib() {
    let (code, intent) = vortexc::ai::generate("৭ এর নামতা").expect("generate");
    assert_eq!(intent, "multiplication_table");
    let (_, out, _) = run_source(&code).expect("runs");
    assert_eq!(out[0], "7");

    let (code2, intent2) = vortexc::ai::generate("fibonacci 8").expect("generate");
    assert_eq!(intent2, "fibonacci");
    let (_, out2, _) = run_source(&code2).expect("runs");
    assert_eq!(out2.len(), 8);

    let (code3, _) = vortexc::ai::generate("is 15 even or odd").expect("generate");
    let (_, out3, _) = run_source(&code3).expect("runs");
    assert_eq!(out3[0], "odd 15");
}

#[test]
fn v02_ai_unknown() {
    assert!(vortexc::ai::generate("রান্নাবাড়িতে আজ মাছ হবে").is_err());
}

#[test]
fn v02_explain() {
    let src = "let x = 5; show(x + 1);";
    let p = parser::parse(src).expect("parse");
    let text = vortexc::ai::explain(src, &p);
    assert!(text.contains("x"));
    assert!(text.contains("show"));
}

// ==================== v0.2 — Package Manager ====================

#[test]
fn v02_pkg_manifest_roundtrip() {
    let src = "[package]\nname = demo\nversion = 1.2.3\nentry = src/main.vx\n\n[deps]\nutils = 0.1.0\n";
    let m = vortexc::pkg::parse_manifest(src).expect("parse");
    assert_eq!(m.name, "demo");
    assert_eq!(m.version, "1.2.3");
    assert_eq!(m.deps, vec![("utils".to_string(), "0.1.0".to_string())]);
    let out = vortexc::pkg::write_manifest(&m);
    let m2 = vortexc::pkg::parse_manifest(&out).expect("re-parse");
    assert_eq!(m.name, m2.name);
    assert_eq!(m.deps, m2.deps);
}

#[test]
fn v02_pkg_manifest_errors() {
    assert!(vortexc::pkg::parse_manifest("name = x").is_err());
    assert!(vortexc::pkg::parse_manifest("[package]\n").is_err());
}

#[test]
fn v02_pkg_crc32() {
    assert_eq!(vortexc::pkg::crc32(b""), 0);
    assert_eq!(vortexc::pkg::crc32(b"a"), 0xE8B7BE43);
    assert_eq!(vortexc::pkg::crc32(b"123456789"), 0xCBF43926);
}

#[test]
fn v02_pkg_archive_roundtrip() {
    let entries = vec![
        vortexc::pkg::ArchiveEntry {
            name: "vortex.pkg".into(),
            data: b"[package]\nname = t".to_vec(),
            is_manifest: true,
        },
        vortexc::pkg::ArchiveEntry {
            name: "src/main.vx".into(),
            data: "show(\"নমস্কাৰ\");".as_bytes().to_vec(),
            is_manifest: false,
        },
    ];
    let packed = vortexc::pkg::pack(&entries);
    let unpacked = vortexc::pkg::unpack(&packed).expect("unpack");
    assert_eq!(unpacked.len(), 2);
    assert_eq!(unpacked[1].name, "src/main.vx");
    assert_eq!(unpacked[1].data, entries[1].data);
}

#[test]
fn v02_pkg_archive_corruption_detected() {
    let entries = vec![vortexc::pkg::ArchiveEntry {
        name: "a.vx".into(),
        data: b"show(1);".to_vec(),
        is_manifest: false,
    }];
    let mut packed = vortexc::pkg::pack(&entries);
    let n = packed.len();
    packed[n - 2] ^= 0xFF; // crc/data ভাঙো
    assert!(vortexc::pkg::unpack(&packed).is_err());
    let mut bad = b"XXXX".to_vec();
    bad.extend_from_slice(&packed[4..]);
    assert!(vortexc::pkg::unpack(&bad).is_err());
}

#[test]
fn v02_pkg_resolve_order_and_cycle() {
    let mut g = std::collections::HashMap::new();
    g.insert("app".to_string(), vec!["net".to_string(), "util".to_string()]);
    g.insert("net".to_string(), vec!["util".to_string()]);
    g.insert("util".to_string(), vec![]);
    let order = vortexc::pkg::resolve_order(&g).expect("resolve");
    assert_eq!(order, vec!["util", "net", "app"]);

    let mut cyc = std::collections::HashMap::new();
    cyc.insert("a".to_string(), vec!["b".to_string()]);
    cyc.insert("b".to_string(), vec!["a".to_string()]);
    assert!(vortexc::pkg::resolve_order(&cyc).is_err());
}

// ==================== v0.2 — Formatter / Linter / Tests ====================

#[test]
fn v02_fmt_idempotent() {
    let src = "let   x=1;  show( x +2 );\nfn f( a,b ) { if a>b { return a; } else { return b; } }\n";
    let once = vortexc::fmt::format_source(src).expect("fmt");
    let twice = vortexc::fmt::format_source(&once).expect("fmt2");
    assert_eq!(once, twice);
    assert!(once.contains("let x = 1;"));
    assert!(once.contains("} else {"));
}

#[test]
fn v02_fmt_else_if_chain() {
    let src = "if a { show(1); } else { if b { show(2); } else { show(3); } }";
    let out = vortexc::fmt::format_source(src).expect("fmt");
    assert!(out.contains("} else if b {"));
}

#[test]
fn v02_lint_catches_issues() {
    let src = "let unused_var = 5; let used = 10; show(used); fn never_called() { show(1); }";
    let p = parser::parse(src).expect("parse");
    let ws = vortexc::lint::lint(src, &p);
    let codes: Vec<&str> = ws.iter().map(|w| w.code).collect();
    assert!(codes.contains(&"L001"), "unused var: {:?}", ws);
    assert!(codes.contains(&"L002"), "unused fn: {:?}", ws);
}

#[test]
fn v02_lint_shadow_and_empty() {
    let src = "let len = 5; if true {}";
    let p = parser::parse(src).expect("parse");
    let ws = vortexc::lint::lint(src, &p);
    let codes: Vec<&str> = ws.iter().map(|w| w.code).collect();
    assert!(codes.contains(&"L003"), "shadow builtin: {:?}", ws);
    assert!(codes.contains(&"L004"), "empty block: {:?}", ws);
}

#[test]
fn v02_lint_clean_code() {
    let src = "let x = 5; show(x);";
    let p = parser::parse(src).expect("parse");
    let ws = vortexc::lint::lint(src, &p);
    assert!(ws.is_empty(), "{:?}", ws);
}

#[test]
fn v02_test_runner_executes_fns() {
    let src = r#"
        fn test_add() { assert_eq(1 + 1, 2); }
        fn test_fail() { assert_eq(1, 2); }
        fn helper() { return 1; }
    "#;
    let p = parser::parse(src).expect("parse");
    let vp = compiler::compile(&p).expect("compile");
    let r = vortexc::vm::execute_fn(&vp, "test_add").expect("test_add passes");
    assert_eq!(r.value, Value::Unit);
    let err = match vortexc::vm::execute_fn(&vp, "test_fail") {
        Ok(_) => panic!("test_fail should fail"),
        Err(e) => e,
    };
    assert!(err.msg.contains("assert_eq ব্যর্থ"));
}

// ==================== v0.2 — Debugger infrastructure ====================

#[test]
fn v02_vm_step_machine() {
    let (_, _, text) = run_source("let x = 3; show(x);").expect("run");
    let vp = vir::parse_text(&text).expect("vir");
    let mut m = vortexc::vm::Machine::new(&vp);
    let mut steps = 0;
    let mut finished = false;
    while steps < 1000 {
        match m.step().expect("step") {
            vortexc::vm::Step::Running => steps += 1,
            vortexc::vm::Step::Finished(_) => {
                finished = true;
                break;
            }
        }
    }
    assert!(finished);
    assert_eq!(m.out, vec!["3"]);
    assert!(steps > 5, "multiple steps: {}", steps);
}

#[test]
fn v02_execute_fn_by_name() {
    let (_, _, text) = run_source("fn maker() { return [10, 20]; } show(maker());").expect("run");
    let vp = vir::parse_text(&text).expect("vir");
    let r = vortexc::vm::execute_fn(&vp, "maker").expect("call maker");
    assert_eq!(r.value, Value::Arr(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Int(10), Value::Int(20)]))));
}

// ==================== v0.2 — Indic digits ====================

#[test]
fn v02_indic_digit_literals() {
    let (_, out, _) = run_source("show(১২ + ৩০); show(২.৫ * ২);").expect("run");
    assert_eq!(out, vec!["42", "5.0"]);
}

#[test]
fn v02_show_variadic() {
    let (_, out, _) = run_source("show(\"a\", 1, 2.5, true);").expect("run");
    assert_eq!(out, vec!["a 1 2.5 true"]);
}
