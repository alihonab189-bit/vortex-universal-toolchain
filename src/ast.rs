//! Abstract Syntax Tree — Parser-এর আউটপুট, Compiler-এর ইনপুট।

#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Ident(String),
    /// op: "-" বা "!"
    Unary(String, Box<Expr>),
    /// op: + - * / % == != < > <= >= && ||
    Binary(String, Box<Expr>, Box<Expr>),
    /// ফাংশন কল: নাম, arguments, source line
    Call(String, Vec<Expr>, u32),
    /// Array literal: [1, 2, 3]
    Array(Vec<Expr>),
    /// Map literal: {"নাম": "ৰাম"}
    MapLit(Vec<(Expr, Expr)>),
    /// Index access: a[0], m["key"], s[2]
    Index(Box<Expr>, Box<Expr>, u32),
    /// Method call: recv.name(args) — compiler built-in-লৈ নামায়
    Method(Box<Expr>, String, Vec<Expr>, u32),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr, u32),
    /// obj[idx] = val
    IndexAssign(Box<Expr>, Box<Expr>, Box<Expr>, u32),
    If {
        cond: Expr,
        then_b: Vec<Stmt>,
        else_b: Option<Vec<Stmt>>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    /// for var in from..to { body }   বা   for var in coll { body }
    For {
        var: String,
        iter: ForIter,
        body: Vec<Stmt>,
    },
    FnDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    ExprStmt(Expr),
}

/// for-লুপের iterator: সংখ্যা-পরিসর বা সংগ্রহ (array/string/map-keys)
#[derive(Clone, Debug)]
pub enum ForIter {
    Range(Box<Expr>, Box<Expr>),
    Coll(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

/// `vortexc ast` কমান্ডের জন্য সুন্দর AST প্রিন্ট।
pub fn dump(p: &Program) -> String {
    let mut s = String::new();
    for st in &p.stmts {
        dump_stmt(st, 0, &mut s);
    }
    s
}

fn indent(level: usize) -> &'static str {
    const PAD: &str = "  ";
    // সর্বোচ্চ 8 level-এর জন্য static padding
    match level {
        0 => "",
        1 => PAD,
        2 => "    ",
        3 => "      ",
        4 => "        ",
        _ => "          ",
    }
}

fn pad(level: usize) -> String {
    let mut s = String::new();
    for _ in 0..level {
        s.push_str(indent(1));
    }
    s
}

fn dump_stmt(st: &Stmt, level: usize, out: &mut String) {
    let p = pad(level);
    match st {
        Stmt::Let(name, e) => {
            out.push_str(&format!("{}Let {} = {}\n", p, name, expr_str(e)));
        }
        Stmt::Assign(name, e, _) => {
            out.push_str(&format!("{}Assign {} = {}\n", p, name, expr_str(e)));
        }
        Stmt::IndexAssign(obj, idx, val, _) => {
            out.push_str(&format!(
                "{}IndexAssign {}[{}] = {}\n",
                p,
                expr_str(obj),
                expr_str(idx),
                expr_str(val)
            ));
        }
        Stmt::If {
            cond,
            then_b,
            else_b,
        } => {
            out.push_str(&format!("{}If {}\n", p, expr_str(cond)));
            for s in then_b {
                dump_stmt(s, level + 1, out);
            }
            if let Some(eb) = else_b {
                out.push_str(&format!("{}Else\n", p));
                for s in eb {
                    dump_stmt(s, level + 1, out);
                }
            }
        }
        Stmt::While { cond, body } => {
            out.push_str(&format!("{}While {}\n", p, expr_str(cond)));
            for s in body {
                dump_stmt(s, level + 1, out);
            }
        }
        Stmt::For { var, iter, body } => {
            match iter {
                ForIter::Range(from, to) => out.push_str(&format!(
                    "{}For {} in {}..{}\n",
                    p,
                    var,
                    expr_str(from),
                    expr_str(to)
                )),
                ForIter::Coll(c) => out.push_str(&format!(
                    "{}For {} in {}\n",
                    p,
                    var,
                    expr_str(c)
                )),
            }
            for s in body {
                dump_stmt(s, level + 1, out);
            }
        }
        Stmt::FnDecl { name, params, body } => {
            out.push_str(&format!("{}Fn {}({})\n", p, name, params.join(", ")));
            for s in body {
                dump_stmt(s, level + 1, out);
            }
        }
        Stmt::Return(e) => {
            out.push_str(&format!(
                "{}Return {}\n",
                p,
                e.as_ref().map(expr_str).unwrap_or_else(|| "unit".into())
            ));
        }
        Stmt::Break => out.push_str(&format!("{}Break\n", p)),
        Stmt::Continue => out.push_str(&format!("{}Continue\n", p)),
        Stmt::ExprStmt(e) => {
            out.push_str(&format!("{}Expr {}\n", p, expr_str(e)));
        }
    }
}

fn expr_str(e: &Expr) -> String {
    match e {
        Expr::Int(v) => v.to_string(),
        Expr::Float(v) => format!("{:?}", v),
        Expr::Str(v) => format!("{:?}", v),
        Expr::Bool(v) => v.to_string(),
        Expr::Ident(s) => s.clone(),
        Expr::Unary(op, x) => format!("({}{})", op, expr_str(x)),
        Expr::Binary(op, l, r) => format!("({} {} {})", expr_str(l), op, expr_str(r)),
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
