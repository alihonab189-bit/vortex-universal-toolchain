//! VORTEX AI Engine — প্রাকৃতিক ভাষা → VORTEX কোড (100% নিজস্ব, rule-based)
//!
//! অসমীয়া / বাংলা / English বাক্য বুঝে VORTEX প্রোগ্রাম তৈরি করে।
//! কোনো external API/LLM নেই — নিজস্ব intent-matching engine:
//!   1. বাক্যের লিপি চেনে (অসমীয়া ৰ/ৱ, বাংলা, English)
//!   2. সংখ্যা বের করে (ASCII + ভারতীয় অংক ০-৯)
//!   3. intent টেবিল থেকে মিলিয়ে কোড generate করে — বাক্যের লিপিতেই
//!
//! কমান্ড:
//!   vortexc ai "১ থেকে ১০ পর্যন্ত যোগফল দেখাও"        → কোড
//!   vortexc ai "..." --run                            → কোড + চালিয়ে আউটপুট
//!   vortexc explain file.vx                           → কোডের ব্যাখ্যা

use crate::ast::{Expr, ForIter, Program, Stmt};

// ---------------- লিপি শনাক্তকরণ ----------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Script {
    Assamese,
    Bengali,
    English,
}

pub fn detect_script(text: &str) -> Script {
    let has_ra = text.contains('ৰ'); // U+09F0 — শুধু অসমীয়ায়
    let has_wa = text.contains('ৱ'); // U+09F1 — শুধু অসমীয়ায়
    let has_indic = text.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c));
    if has_ra || has_wa {
        Script::Assamese
    } else if has_indic {
        Script::Bengali
    } else {
        Script::English
    }
}

// ---------------- সংখ্যা পার্সিং (ভারতীয় অংক সহ) ----------------

fn digit_val(c: char) -> Option<u32> {
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        '\u{09E6}'..='\u{09EF}' => Some(c as u32 - '\u{09E6}' as u32), // ০-৯
        _ => None,
    }
}

/// বাক্য থেকে শেষ সংখ্যা ("১ থেকে ১০" → 10)
pub fn extract_number_last(text: &str) -> Option<i64> {
    let mut last: Option<i64> = None;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if digit_val(chars[i]).is_some() {
            let mut n: i64 = 0;
            while i < chars.len() {
                match digit_val(chars[i]) {
                    Some(d) => {
                        n = n * 10 + d as i64;
                        i += 1;
                    }
                    None => break,
                }
            }
            last = Some(n);
        } else {
            i += 1;
        }
    }
    last
}

/// বাক্য থেকে প্রথম সংখ্যা (০-৯ বা ০-৯)
pub fn extract_number(text: &str) -> Option<i64> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if digit_val(chars[i]).is_some() {
            let mut n: i64 = 0;
            while i < chars.len() {
                match digit_val(chars[i]) {
                    Some(d) => {
                        n = n * 10 + d as i64;
                        i += 1;
                    }
                    None => break,
                }
            }
            return Some(n);
        }
        i += 1;
    }
    None
}

// ---------------- কীওয়ার্ড সেট (লিপি অনুযায়ী) ----------------

#[allow(dead_code)] // fn_/return_ ভবিষ্যৎ AI টেমপ্লেটে ব্যবহার হবে
struct Kw {
    let_: &'static str,
    fn_: &'static str,
    if_: &'static str,
    else_: &'static str,
    for_: &'static str,
    in_: &'static str,
    return_: &'static str,
    show: &'static str,
}

fn kw_for(script: Script) -> Kw {
    match script {
        Script::Assamese => Kw {
            let_: "ধৰো",
            fn_: "কাজ",
            if_: "যদি",
            else_: "নহলে",
            for_: "লুপ",
            in_: "ভিতৰত",
            return_: "দিয়ক",
            show: "দেখুৱাওক",
        },
        Script::Bengali => Kw {
            let_: "ধরো",
            fn_: "কাম",
            if_: "যদি",
            else_: "নাহলে",
            for_: "লুপ",
            in_: "মধ্যে",
            return_: "দিয়ক",
            show: "দেখাও",
        },
        Script::English => Kw {
            let_: "let",
            fn_: "fn",
            if_: "if",
            else_: "else",
            for_: "for",
            in_: "in",
            return_: "return",
            show: "show",
        },
    }
}

fn num_lit(n: i64, script: Script) -> String {
    // অসমীয়া/বাংলা লিপির কোডে অংকও ভারতীয় লিখি — পরিচয়টা পূর্ণ হয়
    match script {
        Script::English => n.to_string(),
        _ => {
            let digits: Vec<char> = "০১২৩৪৫৬৭৮৯".chars().collect();
            n.to_string()
                .chars()
                .map(|c| digits[c as usize - '0' as usize])
                .collect()
        }
    }
}

// ---------------- Intent ইঞ্জিন ----------------

/// trigger শব্দগুলো (তিন ভাষায়) — normalized টেক্সটে substring match
const TRIGGERS_SUM: &[&str] = &["sum", "যোগফল", "যোগ করো", "যোগাও", "total"];
const TRIGGERS_FACTORIAL: &[&str] = &["factorial", "ফেক্টৰিয়েল", "ফ্যাক্টরিয়াল", "গুণফল"];
const TRIGGERS_FIB: &[&str] = &["fibonacci", "ফিবোনাচি", "ফিবোনাচ্চি"];
const TRIGGERS_EVENODD: &[&str] = &["even", "odd", "জোৰ", "যুগ্ম", "জোড়", "বিজোড়", "বিজোড"];
const TRIGGERS_VOTE: &[&str] = &["vote", "ভোট"];
const TRIGGERS_TABLE: &[&str] = &["table", "নামতা", "গুণিতক"];
const TRIGGERS_REVERSE: &[&str] = &["reverse", "ওলোটা", "উল্টো"];
const TRIGGERS_AVERAGE: &[&str] = &["average", "গড়", "গড"];
const TRIGGERS_MAX: &[&str] = &["largest", "biggest", "সবচেয়ে বড়", "ডাঙৰ", "সৰ্বাধিক"];
const TRIGGERS_VOWELS: &[&str] = &["vowel", "স্বৰবৰ্ণ", "স্বরবর্ণ"];
const TRIGGERS_GREET: &[&str] = &["hello", "greet", "নমস্কাৰ", "নমস্কার", "নমস্কাৰ কওক"];
const TRIGGERS_PRINT: &[&str] = &["print", "দেখাও", "দেখুৱাওক", "প্রিন্ট", "প্ৰিন্ট"];
const TRIGGERS_SQUARE: &[&str] = &["square", "বৰ্গ", "বর্গ"];

fn has_trigger(text: &str, triggers: &[&str]) -> bool {
    triggers.iter().any(|t| text.contains(t))
}

/// বাক্য থেকে কোড তৈরি। রিটার্ন: (কোড, intent-এর নাম)
pub fn generate(text: &str) -> Result<(String, &'static str), String> {
    let script = detect_script(text);
    let kw = kw_for(script);
    let normalized = text.to_lowercase();

    let n = extract_number(text);

    // আগে সুনির্দিষ্ট intent-গুলো, পরে সাধারণ print
    if has_trigger(&normalized, TRIGGERS_SUM) {
        let n = extract_number_last(text).or(n).unwrap_or(10);
        let code = format!(
            "// যোগফল: ১ থেকে {}\n{} মোট = ০;\n{} i {} ১..{} {{\n    মোট = মোট + i;\n}}\n{}(\"{}\", মোট);\n",
            num_lit(n, script),
            kw.let_,
            kw.for_,
            kw.in_,
            num_lit(n + 1, script),
            kw.show,
            label_sum(script),
        );
        return Ok((code, "sum"));
    }
    if has_trigger(&normalized, TRIGGERS_FACTORIAL) {
        let n = n.unwrap_or(5);
        let code = format!(
            "// ফেক্টৰিয়েল {}\n{} ফল = ১;\n{} i {} ২..{} {{\n    ফল = ফল * i;\n}}\n{}(\"{}\", ফল);\n",
            num_lit(n, script),
            kw.let_,
            kw.for_,
            kw.in_,
            num_lit(n + 1, script),
            kw.show,
            label_result(script),
        );
        return Ok((code, "factorial"));
    }
    if has_trigger(&normalized, TRIGGERS_FIB) {
        let n = n.map_or(10, |x| x.clamp(1, 20));
        let code = format!(
            "// ফিবোনাচি — প্ৰথম {} টি\n{} a = {};\n{} b = {};\n{} i {} {}..{} {{\n    {}(a);\n    {} t = a + b;\n    a = b;\n    b = t;\n}}\n",
            num_lit(n, script),
            kw.let_,
            num_lit(0, script),
            kw.let_,
            num_lit(1, script),
            kw.for_,
            kw.in_,
            num_lit(0, script),
            num_lit(n, script),
            kw.show,
            kw.let_,
        );
        return Ok((code, "fibonacci"));
    }
    if has_trigger(&normalized, TRIGGERS_EVENODD) {
        let n = n.unwrap_or(7);
        let code = format!(
            "{} ({}) % ২ == ০ {{\n    {}(\"{}\", {});\n}} {} {{\n    {}(\"{}\", {});\n}}\n",
            kw.if_,
            num_lit(n, script),
            kw.show,
            label_even(script),
            num_lit(n, script),
            kw.else_,
            kw.show,
            label_odd(script),
            num_lit(n, script),
        );
        return Ok((code, "even_odd"));
    }
    if has_trigger(&normalized, TRIGGERS_VOTE) {
        let n = n.unwrap_or(18);
        let code = format!(
            "{} {} >= ১৮ {{\n    {}(\"{}\");\n}} {} {{\n    {}(\"{}\");\n}}\n",
            kw.if_,
            num_lit(n, script),
            kw.show,
            label_can_vote(script),
            kw.else_,
            kw.show,
            label_cant_vote(script),
        );
        return Ok((code, "vote_eligibility"));
    }
    if has_trigger(&normalized, TRIGGERS_TABLE) {
        let n = n.unwrap_or(7);
        let code = format!(
            "// {} এর নামতা\n{} i {} ১..১১ {{\n    {}({} * i);\n}}\n",
            num_lit(n, script),
            kw.for_,
            kw.in_,
            kw.show,
            num_lit(n, script),
        );
        return Ok((code, "multiplication_table"));
    }
    if has_trigger(&normalized, TRIGGERS_REVERSE) {
        let word = extract_quoted(text).unwrap_or_else(|| "নমস্কাৰ".into());
        let code = format!(
            "{} শব্দ = \"{}\";\n{} ওলোটা = \"\";\n{} c {} শব্দ.{}() {{\n    ওলোটা = c + ওলোটা;\n}}\n{}(ওলোটা);\n",
            kw.let_,
            word,
            kw.let_,
            kw.for_,
            kw.in_,
            method_chars(script),
            kw.show,
        );
        return Ok((code, "reverse_string"));
    }
    if has_trigger(&normalized, TRIGGERS_AVERAGE) {
        let code = format!(
            "{} তালিকা = [১০, ২০, ৩০, ৪০];\n{} মোট = ০;\n{} x {} তালিকা {{\n    মোট = মোট + x;\n}}\n{}(\"{}\", মোট / তালিকা.{}());\n",
            kw.let_,
            kw.let_,
            kw.for_,
            kw.in_,
            kw.show,
            label_avg(script),
            method_len(script),
        );
        return Ok((code, "average"));
    }
    if has_trigger(&normalized, TRIGGERS_MAX) {
        let code = format!(
            "{} তালিকা = [১২, ৪৫, ৭, ৩৩];\n{} ডাঙৰ = তালিকা[০];\n{} x {} তালিকা {{\n    {} (x > ডাঙৰ) {{\n        ডাঙৰ = x;\n    }}\n}}\n{}(ডাঙৰ);\n",
            kw.let_,
            kw.let_,
            kw.for_,
            kw.in_,
            kw.if_,
            kw.show,
        );
        return Ok((code, "largest"));
    }
    if has_trigger(&normalized, TRIGGERS_VOWELS) {
        let code = format!(
            "{} শব্দ = \"অসমীয়া\";\n{} গণনা = ০;\n{} c {} শব্দ.{}() {{\n    {} \"অ\" == c || \"আ\" == c || \"ই\" == c || \"এ\" == c {{\n        গণনা = গণনা + ১;\n    }}\n}}\n{}(গণনা);\n",
            kw.let_,
            kw.let_,
            kw.for_,
            kw.in_,
            method_chars(script),
            kw.if_,
            kw.show,
        );
        return Ok((code, "count_vowels"));
    }
    if has_trigger(&normalized, TRIGGERS_SQUARE) {
        let n = n.unwrap_or(9);
        let code = format!(
            "{}(\"{}\", {} * {});\n",
            kw.show,
            label_square(script),
            num_lit(n, script),
            num_lit(n, script),
        );
        return Ok((code, "square"));
    }
    if has_trigger(&normalized, TRIGGERS_GREET)
        || has_trigger(&normalized, TRIGGERS_PRINT)
    {
        let msg = extract_quoted(text).unwrap_or_else(|| match script {
            Script::Assamese => "নমস্কাৰ, পৃথিৱী!".into(),
            Script::Bengali => "নমস্কার, পৃথিবী!".into(),
            Script::English => "Hello, VORTEX!".into(),
        });
        let code = format!("{}(\"{}\");\n", kw.show, msg);
        return Ok((code, "greet"));
    }

    Err(match script {
        Script::Assamese => "এই বাক্যটো এতিয়াও বুজিব পৰা নাই। উদাহৰণ: \"১ থেকে ১০ লৈ যোগফল দেখাও\", \"৭ এৰ নামতা\", \"ফিবোনাচি ৮\", \"১৫ জোৰ নে যুগ্ম\"".into(),
        Script::Bengali => "এই বাক্যটি এখনো বোঝা যায়নি। উদাহরণ: \"১ থেকে ১০ পর্যন্ত যোগফল দেখাও\", \"৭ এর নামতা\", \"ফিবোনাচি ৮\", \"১৫ জোড় না বিজোড়\"".into(),
        Script::English => "Sentence not understood yet. Examples: \"sum from 1 to 10\", \"multiplication table of 7\", \"fibonacci 8\", \"is 15 even or odd\", \"print hello\"".into(),
    })
}

fn label_sum(script: Script) -> &'static str {
    match script {
        Script::Assamese => "যোগফল:",
        Script::Bengali => "যোগফল:",
        Script::English => "sum:",
    }
}

fn label_result(script: Script) -> &'static str {
    match script {
        Script::Assamese => "ফলাফল:",
        Script::Bengali => "ফলাফল:",
        Script::English => "result:",
    }
}

fn label_even(script: Script) -> &'static str {
    match script {
        Script::Assamese => "জোৰ (even)",
        Script::Bengali => "জোড় (even)",
        Script::English => "even",
    }
}

fn label_odd(script: Script) -> &'static str {
    match script {
        Script::Assamese => "অজোৰ (odd)",
        Script::Bengali => "বিজোড় (odd)",
        Script::English => "odd",
    }
}

fn label_can_vote(script: Script) -> &'static str {
    match script {
        Script::Assamese => "আপুনি ভোট দিব পাৰিব ✅",
        Script::Bengali => "আপনি ভোট দিতে পারবেন ✅",
        Script::English => "You can vote ✅",
    }
}

fn label_cant_vote(script: Script) -> &'static str {
    match script {
        Script::Assamese => "এতিয়াও সৰু — অপেক্ষা কৰক",
        Script::Bengali => "এখনো ছোট — অপেক্ষা করুন",
        Script::English => "Too young — please wait",
    }
}

fn label_avg(script: Script) -> &'static str {
    match script {
        Script::Assamese => "গড়:",
        Script::Bengali => "গড়:",
        Script::English => "average:",
    }
}

fn label_square(script: Script) -> &'static str {
    match script {
        Script::Assamese => "বৰ্গ:",
        Script::Bengali => "বর্গ:",
        Script::English => "square:",
    }
}

fn method_chars(script: Script) -> &'static str {
    match script {
        Script::Assamese => "অক্ষৰ",
        Script::Bengali => "অক্ষর",
        Script::English => "chars",
    }
}

fn method_len(script: Script) -> &'static str {
    match script {
        Script::Assamese => "দৈৰ্ঘ্য",
        Script::Bengali => "দৈর্ঘ্য",
        Script::English => "len",
    }
}

/// উদ্ধৃতিতে মোড়ানো টেক্সট বের করে
fn extract_quoted(text: &str) -> Option<String> {
    let start = text.find('"')?;
    let rest = &text[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// `--run` এর জন্য: তৈরি কোড সরাসরি চালায়
pub fn run_generated(code: &str) -> Result<Vec<String>, String> {
    match crate::run_source(code) {
        Ok((_, out, _)) => Ok(out),
        Err(e) => Err(e.to_string()),
    }
}

// ---------------- explain — কোড → প্রাকৃতিক ভাষা ----------------

/// AST পড়ে প্রতিটি statement-এর ব্যাখ্যা তৈরি করে (অসমীয়া+বাংলা+English মিশ্র)
pub fn explain(src: &str, p: &Program) -> String {
    let script = detect_script(src);
    let mut out = String::new();
    out.push_str("VORTEX explain — কোডের ব্যাখ্যা (explanation)\n");
    out.push_str("================================================\n");
    for (i, st) in p.stmts.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, explain_stmt(st, script)));
    }
    if p.stmts.is_empty() {
        out.push_str("(খালি প্রোগ্রাম — empty program)\n");
    }
    out
}

fn explain_stmt(st: &Stmt, script: Script) -> String {
    match st {
        Stmt::Let(name, e) => match script {
            Script::English => format!("Variable '{}' gets the value {}", name, expr_str(e)),
            _ => format!("ভ্যারিয়েবল '{}' -এ মান রাখা হয়েছে {}", name, expr_str(e)),
        },
        Stmt::Assign(name, e, _) => match script {
            Script::English => format!("Variable '{}' is updated to {}", name, expr_str(e)),
            _ => format!("ভ্যারিয়েবল '{}' -এর মান বদলে {} করা হয়েছে", name, expr_str(e)),
        },
        Stmt::IndexAssign(o, i, v, _) => format!(
            "{}[{}] -এর মান {} করা হয়েছে (index assign)",
            expr_str(o),
            expr_str(i),
            expr_str(v)
        ),
        Stmt::If { cond, then_b, else_b } => {
            let mut s = format!("শর্ত যাচাই: যদি {} হয় → {} টি কাজ", expr_str(cond), then_b.len());
            match else_b {
                Some(eb) => s.push_str(&format!(", নহলে → {} টি কাজ", eb.len())),
                None => {}
            }
            s
        }
        Stmt::While { cond, body } => format!(
            "লুপ: যতক্ষণ {} থাকে, {} টি কাজ পুনরাবৃত্তি হয়",
            expr_str(cond),
            body.len()
        ),
        Stmt::For { var, iter, body } => match iter {
            ForIter::Range(a, b) => format!(
                "লুপ: '{}' হলে {} থেকে {} পর্যন্ত গুনে {} টি কাজ করে",
                var,
                expr_str(a),
                expr_str(b),
                body.len()
            ),
            ForIter::Coll(c) => format!(
                "লুপ: '{}' -এর প্রতিটি উপাদান নিয়ে {} টি কাজ করে",
                expr_str(c),
                body.len()
            ),
        },
        Stmt::FnDecl { name, params, body } => format!(
            "ফাংশন {}({}) — {} টি কাজ করে, দরকার হলে মান ফেরত দেয় (return)",
            name,
            params.join(", "),
            body.len()
        ),
        Stmt::Return(e) => format!(
            "ফাংশন শেষ — {} ফেরত দেয় (returns {})",
            e.as_ref().map(expr_str).unwrap_or_else(|| "unit".into()),
            e.as_ref().map(expr_str).unwrap_or_else(|| "unit".into())
        ),
        Stmt::Break => "লুপ এখনই থামে (break)".into(),
        Stmt::Continue => "লুপের পরের ধাপে যায় (continue)".into(),
        Stmt::ExprStmt(e) => format!("কাজ চালায়: {}", expr_str(e)),
    }
}

fn expr_str(e: &Expr) -> String {
    match e {
        Expr::Int(v) => v.to_string(),
        Expr::Float(v) => format!("{:?}", v),
        Expr::Str(s) => format!("{:?}", s),
        Expr::Bool(v) => v.to_string(),
        Expr::Ident(s) => s.clone(),
        Expr::Unary(op, x) => format!("{}{}", op, expr_str(x)),
        Expr::Binary(op, l, r) => format!("{} {} {}", expr_str(l), op, expr_str(r)),
        Expr::Call(name, args, _) => format!(
            "{}({})",
            name,
            args.iter().map(expr_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::Array(items) => format!(
            "[{}]",
            items.iter().map(expr_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::MapLit(pairs) => format!(
            "{{{}}}",
            pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", expr_str(k), expr_str(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Index(o, i, _) => format!("{}[{}]", expr_str(o), expr_str(i)),
        Expr::Method(recv, name, args, _) => format!(
            "{}.{}({})",
            expr_str(recv),
            name,
            args.iter().map(expr_str).collect::<Vec<_>>().join(", ")
        ),
    }
}
