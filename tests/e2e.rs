//! End-to-end টেস্ট: Source → Lexer → Parser → Compiler → VIR → VM

use vortexc::run_source;
use vortexc::vir;

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
