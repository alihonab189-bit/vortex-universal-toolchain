//! Formatter — AST থেকে পরিচ্ছন্ন, একরূপ ফরম্যাটের VORTEX source বানায়।
//!
//! `vortexc fmt file.vx` — parse → pretty-print। নিজস্ব নিয়ম:
//! - 4 স্পেস ইনডেন্ট
//! - `{` আগে স্পেস, statement-এর পর খালি লাইন নিয়ন্ত্রণ
//! - `fmt(fmt(x)) == fmt(x)` — idempotent (টেস্টে প্রমাণিত)

use crate::ast::{dump, Expr, ForIter, Program, Stmt};

/// পুরো প্রোগ্রাম ফরম্যাট করে
pub fn format_source(src: &str) -> Result<String, String> {
    let p = crate::parser::parse(src).map_err(|e| {
        format!(
            "parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
            e.line, e.col, e.msg
        )
    })?;
    Ok(format_program(&p))
}

pub fn format_program(p: &Program) -> String {
    let mut out = String::new();
    let mut prev_block = false;
    for (i, st) in p.stmts.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            // block শেষে হলে (fn/if-এর body) খালি লাইন — পড়তে সুবিধা
            if prev_block {
                out.push('\n');
            }
        }
        prev_block = matches!(st, Stmt::FnDecl { .. });
        fmt_stmt(st, 0, &mut out);
    }
    // শেষে ঠিক একটি newline
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn pad(level: usize) -> String {
    "    ".repeat(level)
}

fn fmt_stmt(st: &Stmt, level: usize, out: &mut String) {
    let p = pad(level);
    match st {
        Stmt::Let(name, e) => {
            out.push_str(&format!("{}let {} = {};\n", p, name, fmt_expr(e)));
        }
        Stmt::Assign(name, e, _) => {
            out.push_str(&format!("{}{} = {};\n", p, name, fmt_expr(e)));
        }
        Stmt::IndexAssign(obj, idx, val, _) => {
            out.push_str(&format!(
                "{}{}[{}] = {};\n",
                p,
                fmt_expr(obj),
                fmt_expr(idx),
                fmt_expr(val)
            ));
        }
        Stmt::If {
            cond,
            then_b,
            else_b,
        } => {
            out.push_str(&format!("{}if {} {{\n", p, fmt_expr(cond)));
            for s in then_b {
                fmt_stmt(s, level + 1, out);
            }
            match else_b {
                // else if চেইন — একই লাইনে জোড়া থাকে
                Some(eb) if eb.len() == 1 && matches!(eb[0], Stmt::If { .. }) => {
                    out.push_str(&format!("{}}} else ", p));
                    fmt_stmt(&eb[0], level, out);
                }
                Some(eb) => {
                    out.push_str(&format!("{}}} else {{\n", p));
                    for s in eb {
                        fmt_stmt(s, level + 1, out);
                    }
                    out.push_str(&format!("{}}}\n", p));
                }
                None => out.push_str(&format!("{}}}\n", p)),
            }
        }
        Stmt::While { cond, body } => {
            out.push_str(&format!("{}while {} {{\n", p, fmt_expr(cond)));
            for s in body {
                fmt_stmt(s, level + 1, out);
            }
            out.push_str(&format!("{}}}\n", p));
        }
        Stmt::For { var, iter, body } => {
            match iter {
                ForIter::Range(from, to) => {
                    out.push_str(&format!(
                        "{}for {} in {}..{} {{\n",
                        p,
                        var,
                        fmt_expr(from),
                        fmt_expr(to)
                    ));
                }
                ForIter::Coll(c) => {
                    out.push_str(&format!(
                        "{}for {} in {} {{\n",
                        p,
                        var,
                        fmt_expr(c)
                    ));
                }
            }
            for s in body {
                fmt_stmt(s, level + 1, out);
            }
            out.push_str(&format!("{}}}\n", p));
        }
        Stmt::FnDecl { name, params, body } => {
            out.push_str(&format!(
                "{}fn {}({}) {{\n",
                p,
                name,
                params.join(", ")
            ));
            for s in body {
                fmt_stmt(s, level + 1, out);
            }
            out.push_str(&format!("{}}}\n", p));
        }
        Stmt::Return(e) => {
            match e {
                Some(x) => out.push_str(&format!("{}return {};\n", p, fmt_expr(x))),
                None => out.push_str(&format!("{}return;\n", p)),
            }
        }
        Stmt::Break => out.push_str(&format!("{}break;\n", p)),
        Stmt::Continue => out.push_str(&format!("{}continue;\n", p)),
        Stmt::ExprStmt(e) => {
            out.push_str(&format!("{}{};\n", p, fmt_expr(e)));
        }
    }
}

/// Binary operator-এর চারপাশে স্পেস রাখে; unary তে রাখে না
fn fmt_expr(e: &Expr) -> String {
    match e {
        Expr::Int(v) => v.to_string(),
        Expr::Float(v) => format!("{:?}", v),
        Expr::Str(s) => format!("{:?}", s),
        Expr::Bool(v) => v.to_string(),
        Expr::Ident(s) => s.clone(),
        Expr::Unary(op, x) => format!("{}{}", op, fmt_expr(x)),
        Expr::Binary(op, l, r) => format!("{} {} {}", fmt_expr(l), op, fmt_expr(r)),
        Expr::Call(name, args, _) => format!(
            "{}({})",
            name,
            args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        Expr::Array(items) => format!(
            "[{}]",
            items.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
        Expr::MapLit(pairs) => format!(
            "{{{}}}",
            pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", fmt_expr(k), fmt_expr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Index(o, i, _) => format!("{}[{}]", fmt_expr(o), fmt_expr(i)),
        Expr::Method(recv, name, args, _) => format!(
            "{}.{}({})",
            recv_as_str(recv),
            name,
            args.iter().map(fmt_expr).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn recv_as_str(recv: &Expr) -> String {
    fmt_expr(recv)
}

/// AST dump-ও পাওয়া যায় (পুরনো `ast` কমান্ডের জন্য re-export)
pub fn ast_dump(p: &Program) -> String {
    dump(p)
}
