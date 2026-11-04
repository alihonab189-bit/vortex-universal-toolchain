//! Linter — কোডের সমস্যা compile হওয়ার আগেই ধরে।
//!
//! নিয়ম (v0.2):
//!   L001  ঘোষিত কিন্তু ব্যবহার হয়নি এমন variable
//!   L002  ঘোষিত কিন্তু কোথাও কল হয়নি এমন ফাংশন
//!   L003  built-in-এর নামে variable ছায়া ফেলে (shadowing)
//!   L004  খালি block
//!   L005  return-এর পরে পৌঁছানো যায় না এমন কোড
//!   L006  while(true) — সম্ভাব্য অসীম লুপ (break নেই)

use crate::ast::{Expr, ForIter, Program, Stmt};
use crate::keywords::lookup_builtin;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Warning {
    pub code: &'static str,
    pub msg: String,
    /// আনুমানিক লাইন (token scan থেকে; 0 = অজানা)
    pub line: usize,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(f, "{} [লাইন {}]: {}", self.code, self.line, self.msg)
        } else {
            write!(f, "{}: {}", self.code, self.msg)
        }
    }
}

/// নাম → প্রথম ঘোষণার আনুমানিক লাইন (token scan দিয়ে)
fn find_lines(src: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    if let Ok(toks) = crate::lexer::lex(src) {
        let mut prev_let = false;
        let mut prev_fn = false;
        for t in &toks {
            if let crate::token::Tok::Ident(name) = &t.tok {
                if prev_let || prev_fn {
                    out.push((name.clone(), t.line));
                }
            }
            prev_let = matches!(t.tok, crate::token::Tok::Let);
            prev_fn = matches!(t.tok, crate::token::Tok::Fn);
        }
    }
    out
}

pub fn lint(src: &str, p: &Program) -> Vec<Warning> {
    let mut w = Vec::new();
    let lines = find_lines(src);

    // সব ব্যবহৃত নাম (Ident/Call/Method/Assign) সংগ্রহ
    let mut used: HashSet<String> = HashSet::new();
    let mut declared_vars: Vec<String> = Vec::new();
    let mut declared_fns: Vec<String> = Vec::new();
    collect(p, &mut used, &mut declared_vars, &mut declared_fns);

    for name in &declared_vars {
        if !used.contains(name) {
            let line = lines
                .iter()
                .find(|(n, _)| n == name)
                .map_or(0, |(_, l)| *l);
            w.push(Warning {
                code: "L001",
                msg: format!(
                    "variable '{}' ঘোষিত কিন্তু কোথাও ব্যবহার হয়নি (declared but never used)",
                    name
                ),
                line,
            });
        }
    }
    for name in &declared_fns {
        if !used.contains(name) && name != "main" {
            let line = lines
                .iter()
                .find(|(n, _)| n == name)
                .map_or(0, |(_, l)| *l);
            w.push(Warning {
                code: "L002",
                msg: format!(
                    "ফাংশন '{}' কোথাও কল হয়নি (function never called)",
                    name
                ),
                line,
            });
        }
    }

    // built-in shadowing + খালি block + return-পরবর্তী কোড + অসীম লুপ
    for st in &p.stmts {
        check_stmt(st, &mut w, &lines);
    }

    w
}

fn line_of(name: &str, lines: &[(String, usize)]) -> usize {
    lines.iter().find(|(n, _)| n == name).map_or(0, |(_, l)| *l)
}

fn check_stmt(st: &Stmt, w: &mut Vec<Warning>, lines: &[(String, usize)]) {
    match st {
        Stmt::Let(name, e) => {
            if lookup_builtin(name).is_some() {
                w.push(Warning {
                    code: "L003",
                    msg: format!(
                        "'{}' built-in-এর নাম — variable হিসেবে ব্যবহার built-in-কে ছায়া ফেলে (shadows a built-in)",
                        name
                    ),
                    line: line_of(name, lines),
                });
            }
            check_expr(e, w);
        }
        Stmt::Assign(_, e, _) => check_expr(e, w),
        Stmt::IndexAssign(o, i, v, _) => {
            check_expr(o, w);
            check_expr(i, w);
            check_expr(v, w);
        }
        Stmt::If {
            cond,
            then_b,
            else_b,
        } => {
            check_expr(cond, w);
            if then_b.is_empty() {
                w.push(Warning {
                    code: "L004",
                    msg: "খালি then-block (empty then-block)".into(),
                    line: 0,
                });
            }
            for s in then_b {
                check_stmt(s, w, lines);
            }
            if let Some(eb) = else_b {
                for s in eb {
                    check_stmt(s, w, lines);
                }
            }
        }
        Stmt::While { cond, body } => {
            check_expr(cond, w);
            let has_break = block_has_break(body);
            if is_true_lit(cond) && !has_break {
                w.push(Warning {
                    code: "L006",
                    msg: "'while true' লুপে break নেই — অসীম লুপ হতে পারে (possible infinite loop)".to_string(),
                    line: 0,
                });
            }
            for s in body {
                check_stmt(s, w, lines);
            }
        }
        Stmt::For { iter, body, .. } => {
            match iter {
                ForIter::Range(a, b) => {
                    check_expr(a, w);
                    check_expr(b, w);
                }
                ForIter::Coll(c) => check_expr(c, w),
            }
            for s in body {
                check_stmt(s, w, lines);
            }
        }
        Stmt::FnDecl { name, body, .. } => {
            if body.is_empty() {
                w.push(Warning {
                    code: "L004",
                    msg: format!("ফাংশন '{}'-এর খালি body (empty function body)", name),
                    line: line_of(name, lines),
                });
            }
            // return-এর পরে পৌঁছানো যায় না এমন কোড
            if let Some(pos) = body.iter().position(|s| matches!(s, Stmt::Return(_))) {
                if pos + 1 < body.len() {
                    w.push(Warning {
                        code: "L005",
                        msg: format!(
                            "ফাংশন '{}': return-এর পরের কোড কখনো চলে না (unreachable code after return)",
                            name
                        ),
                        line: line_of(name, lines),
                    });
                }
            }
            for s in body {
                check_stmt(s, w, lines);
            }
        }
        Stmt::Return(e) => {
            if let Some(x) = e {
                check_expr(x, w);
            }
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::ExprStmt(e) => check_expr(e, w),
    }
}

fn is_true_lit(e: &Expr) -> bool {
    matches!(e, Expr::Bool(true))
}

fn block_has_break(body: &[Stmt]) -> bool {
    body.iter().any(|s| match s {
        Stmt::Break => true,
        Stmt::If { then_b, else_b, .. } => {
            block_has_break(then_b)
                || else_b.as_ref().map_or(false, |eb| block_has_break(eb))
        }
        _ => false,
    })
}

fn check_expr(e: &Expr, w: &mut Vec<Warning>) {
    match e {
        Expr::Binary(_, l, r) | Expr::Index(l, r, _) => {
            check_expr(l, w);
            check_expr(r, w);
        }
        Expr::Unary(_, x) => check_expr(x, w),
        Expr::Call(_, args, _) => {
            for a in args {
                check_expr(a, w);
            }
        }
        Expr::Array(items) => {
            for a in items {
                check_expr(a, w);
            }
        }
        Expr::MapLit(pairs) => {
            for (k, v) in pairs {
                check_expr(k, w);
                check_expr(v, w);
            }
        }
        Expr::Method(recv, _, args, _) => {
            check_expr(recv, w);
            for a in args {
                check_expr(a, w);
            }
        }
        _ => {}
    }
}

fn collect(
    p: &Program,
    used: &mut HashSet<String>,
    vars: &mut Vec<String>,
    fns: &mut Vec<String>,
) {
    // প্রথমে সব ফাংশন নাম (hoisting-এর কারণে আগে থেকেই কল হতে পারে)
    for st in &p.stmts {
        if let Stmt::FnDecl { name, body, .. } = st {
            fns.push(name.clone());
            collect_stmt(body, used);
        }
    }
    for st in &p.stmts {
        collect_stmt_top(st, used, vars);
    }
}

fn collect_stmt_top(st: &Stmt, used: &mut HashSet<String>, vars: &mut Vec<String>) {
    match st {
        Stmt::FnDecl { .. } => {}
        Stmt::Let(name, e) => {
            collect_expr(e, used);
            vars.push(name.clone());
        }
        other => collect_stmt(std::slice::from_ref(other), used),
    }
}

fn collect_stmt(body: &[Stmt], used: &mut HashSet<String>) {
    for st in body {
        match st {
            Stmt::Let(_, e) => collect_expr(e, used),
            Stmt::Assign(name, e, _) => {
                used.insert(name.clone());
                collect_expr(e, used);
            }
            Stmt::IndexAssign(o, i, v, _) => {
                collect_expr(o, used);
                collect_expr(i, used);
                collect_expr(v, used);
            }
            Stmt::If {
                cond,
                then_b,
                else_b,
            } => {
                collect_expr(cond, used);
                collect_stmt(then_b, used);
                if let Some(eb) = else_b {
                    collect_stmt(eb, used);
                }
            }
            Stmt::While { cond, body } => {
                collect_expr(cond, used);
                collect_stmt(body, used);
            }
            Stmt::For { iter, body, .. } => {
                match iter {
                    ForIter::Range(a, b) => {
                        collect_expr(a, used);
                        collect_expr(b, used);
                    }
                    ForIter::Coll(c) => collect_expr(c, used),
                }
                collect_stmt(body, used);
            }
            Stmt::FnDecl { .. } => {}
            Stmt::Return(e) => {
                if let Some(x) = e {
                    collect_expr(x, used);
                }
            }
            Stmt::Break | Stmt::Continue => {}
            Stmt::ExprStmt(e) => collect_expr(e, used),
        }
    }
}

fn collect_expr(e: &Expr, used: &mut HashSet<String>) {
    match e {
        Expr::Ident(s) => {
            used.insert(s.clone());
        }
        Expr::Call(s, args, _) => {
            used.insert(s.clone());
            for a in args {
                collect_expr(a, used);
            }
        }
        Expr::Unary(_, x) => collect_expr(x, used),
        Expr::Binary(_, l, r) => {
            collect_expr(l, used);
            collect_expr(r, used);
        }
        Expr::Array(items) => {
            for a in items {
                collect_expr(a, used);
            }
        }
        Expr::MapLit(pairs) => {
            for (k, v) in pairs {
                collect_expr(k, used);
                collect_expr(v, used);
            }
        }
        Expr::Index(o, i, _) => {
            collect_expr(o, used);
            collect_expr(i, used);
        }
        Expr::Method(recv, _, args, _) => {
            collect_expr(recv, used);
            for a in args {
                collect_expr(a, used);
            }
        }
        _ => {}
    }
}
