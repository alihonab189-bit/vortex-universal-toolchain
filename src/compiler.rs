//! Compiler — AST থেকে VIR (Vortex Intermediate Representation) bytecode তৈরি করে।
//!
//! - প্রতিটি ফাংশন আলাদা করে compile হয়; top-level কোড `main` ফাংশন হয়।
//! - Compile time-এই undefined variable / unknown function / arity mismatch ধরা পড়ে।
//! - && ও || সংক্ষিপ্ত মূল্যায়ন (short-circuit) হয় jump দিয়ে।

use std::collections::HashMap;

use crate::ast::{Expr, Program, Stmt};
use crate::keywords::lookup_builtin;
use crate::value::Value;
use crate::vir::{Func, Instr, VProgram};

#[derive(Debug)]
pub struct CompileError {
    pub msg: String,
}

/// একটি ফাংশন কম্পাইল করার সময়ের অবস্থা (scope + slot allocation + code emission)
struct FnCtx {
    code: Vec<Instr>,
    scopes: Vec<HashMap<String, usize>>,
    nslots: usize,
}

impl FnCtx {
    fn new() -> Self {
        FnCtx {
            code: Vec::new(),
            scopes: Vec::new(),
            nslots: 0,
        }
    }

    fn enter(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str) -> Result<usize, CompileError> {
        let scope = self.scopes.last_mut().expect("no scope");
        if scope.contains_key(name) {
            return Err(CompileError {
                msg: format!(
                    "'{}' এই scope-এ ইতিমধ্যে ঘোষিত (already declared in this scope)",
                    name
                ),
            });
        }
        let s = self.nslots;
        self.nslots += 1;
        scope.insert(name.to_string(), s);
        Ok(s)
    }

    fn fresh_slot(&mut self) -> usize {
        let s = self.nslots;
        self.nslots += 1;
        s
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        for sc in self.scopes.iter().rev() {
            if let Some(&s) = sc.get(name) {
                return Some(s);
            }
        }
        None
    }

    fn emit(&mut self, i: Instr) -> usize {
        self.code.push(i);
        self.code.len() - 1
    }

    fn patch(&mut self, at: usize, target: usize) {
        match self.code[at] {
            Instr::Jmp(ref mut t) | Instr::JmpIfFalse(ref mut t) | Instr::JmpIfTrue(ref mut t) => {
                *t = target;
            }
            _ => {}
        }
    }

    fn last_is_ret(&self) -> bool {
        matches!(self.code.last(), Some(Instr::Ret))
    }
}

/// ফাংশন টেবিল: নাম → (funcs-এ index, arity)
type FnTable = HashMap<String, (usize, usize)>;

pub fn compile(program: &Program) -> Result<VProgram, CompileError> {
    // Pass 1: সব top-level ফাংশনের signature সংগ্রহ (hoisting —
    // তাই ঘোষণার আগে কল করলেও চলবে, recursion ও mutual recursion-ও চলে)
    let mut fn_table: FnTable = HashMap::new();
    let mut fn_asts: Vec<(&str, &Vec<String>, &Vec<Stmt>)> = Vec::new();
    for st in &program.stmts {
        if let Stmt::FnDecl { name, params, body } = st {
            if name == "main" {
                return Err(CompileError {
                    msg: "'main' নামটি সংরক্ষিত (reserved)".into(),
                });
            }
            if fn_table.contains_key(name) {
                return Err(CompileError {
                    msg: format!("ফাংশন '{}' দুবার ঘোষিত (duplicate function)", name),
                });
            }
            fn_table.insert(name.clone(), (fn_asts.len(), params.len()));
            fn_asts.push((name.as_str(), params, body));
        }
    }

    let mut funcs: Vec<Func> = fn_asts
        .iter()
        .map(|(n, p, _)| Func {
            name: n.to_string(),
            params: p.to_vec(),
            nslots: p.len(),
            code: Vec::new(),
        })
        .collect();

    // Pass 2a: main (top-level কোড)
    let mut mc = FnCtx::new();
    mc.enter();
    for st in &program.stmts {
        if let Stmt::FnDecl { .. } = st {
            continue;
        }
        compile_stmt(&mut mc, st, &fn_table)?;
    }
    mc.exit();
    if !mc.last_is_ret() {
        mc.emit(Instr::Const(Value::Unit));
        mc.emit(Instr::Ret);
    }
    funcs.push(Func {
        name: "main".into(),
        params: Vec::new(),
        nslots: mc.nslots,
        code: mc.code,
    });
    let entry = funcs.len() - 1;

    // Pass 2b: প্রতিটি ফাংশনের body
    for (idx, (name, params, body)) in fn_asts.into_iter().enumerate() {
        let mut fc = FnCtx::new();
        fc.enter();
        for p in params {
            fc.declare(p)?;
        }
        for st in body {
            compile_stmt(&mut fc, st, &fn_table)?;
        }
        fc.exit();
        if !fc.last_is_ret() {
            fc.emit(Instr::Const(Value::Unit));
            fc.emit(Instr::Ret);
        }
        funcs[idx] = Func {
            name: name.to_string(),
            params: params.to_vec(),
            nslots: fc.nslots,
            code: fc.code,
        };
    }

    Ok(VProgram { funcs, entry })
}

fn compile_stmt(
    c: &mut FnCtx,
    st: &Stmt,
    ft: &FnTable,
) -> Result<(), CompileError> {
    match st {
        Stmt::Let(name, e) => {
            let slot = c.declare(name)?;
            compile_expr(c, e, ft)?;
            c.emit(Instr::Store(slot));
        }
        Stmt::Assign(name, e, _) => {
            let slot = c.resolve(name).ok_or_else(|| CompileError {
                msg: format!(
                    "অজানা ভ্যারিয়েবল '{}' (unknown variable — আগে 'let' দিয়ে ঘোষণা করো)",
                    name
                ),
            })?;
            compile_expr(c, e, ft)?;
            c.emit(Instr::Store(slot));
        }
        Stmt::If {
            cond,
            then_b,
            else_b,
        } => {
            compile_expr(c, cond, ft)?;
            let jf = c.emit(Instr::JmpIfFalse(0));
            for s in then_b {
                compile_stmt(c, s, ft)?;
            }
            let je = c.emit(Instr::Jmp(0));
            c.patch(jf, c.code.len());
            if let Some(eb) = else_b {
                for s in eb {
                    compile_stmt(c, s, ft)?;
                }
            }
            c.patch(je, c.code.len());
        }
        Stmt::While { cond, body } => {
            let lstart = c.code.len();
            compile_expr(c, cond, ft)?;
            let jf = c.emit(Instr::JmpIfFalse(0));
            for s in body {
                compile_stmt(c, s, ft)?;
            }
            c.emit(Instr::Jmp(lstart));
            c.patch(jf, c.code.len());
        }
        Stmt::For {
            var,
            from,
            to,
            body,
        } => {
            c.enter();
            let islot = c.declare(var)?;
            compile_expr(c, from, ft)?;
            c.emit(Instr::Store(islot));
            let eslot = c.fresh_slot();
            compile_expr(c, to, ft)?;
            c.emit(Instr::Store(eslot));
            let lstart = c.code.len();
            c.emit(Instr::Load(islot));
            c.emit(Instr::Load(eslot));
            c.emit(Instr::Lt);
            let jf = c.emit(Instr::JmpIfFalse(0));
            for s in body {
                compile_stmt(c, s, ft)?;
            }
            // i = i + 1
            c.emit(Instr::Load(islot));
            c.emit(Instr::Const(Value::Int(1)));
            c.emit(Instr::Add);
            c.emit(Instr::Store(islot));
            c.emit(Instr::Jmp(lstart));
            c.patch(jf, c.code.len());
            c.exit();
        }
        Stmt::FnDecl { name, .. } => {
            return Err(CompileError {
                msg: format!(
                    "ফাংশন '{}' শুধু top level-এ ঘোষণা করা যায় (functions can only be declared at the top level)",
                    name
                ),
            })
        }
        Stmt::Return(e) => {
            match e {
                Some(x) => compile_expr(c, x, ft)?,
                None => {
                    c.emit(Instr::Const(Value::Unit));
                }
            }
            c.emit(Instr::Ret);
        }
        Stmt::ExprStmt(e) => {
            compile_expr(c, e, ft)?;
            c.emit(Instr::Pop);
        }
    }
    Ok(())
}

fn compile_expr(c: &mut FnCtx, e: &Expr, ft: &FnTable) -> Result<(), CompileError> {
    match e {
        Expr::Int(v) => {
            c.emit(Instr::Const(Value::Int(*v)));
        }
        Expr::Float(v) => {
            c.emit(Instr::Const(Value::Float(*v)));
        }
        Expr::Str(v) => {
            c.emit(Instr::Const(Value::Str(v.clone())));
        }
        Expr::Bool(v) => {
            c.emit(Instr::Const(Value::Bool(*v)));
        }
        Expr::Ident(name) => {
            let slot = c.resolve(name).ok_or_else(|| CompileError {
                msg: format!("অজানা ভ্যারিয়েবল '{}' (unknown variable)", name),
            })?;
            c.emit(Instr::Load(slot));
        }
        Expr::Unary(op, x) => {
            compile_expr(c, x, ft)?;
            if op == "-" {
                c.emit(Instr::Neg);
            } else {
                c.emit(Instr::Not);
            }
        }
        Expr::Binary(op, l, r) => {
            if op == "&&" {
                // short-circuit AND: a মিথ্যা হলে a-ই ফলাফল
                // [a] Dup JPF Lf Pop b Jmp Lend ; Lf: → a=false থেকে যায়
                compile_expr(c, l, ft)?;
                c.emit(Instr::Dup);
                let jf = c.emit(Instr::JmpIfFalse(0));
                c.emit(Instr::Pop);
                compile_expr(c, r, ft)?;
                let jend = c.emit(Instr::Jmp(0));
                c.patch(jf, c.code.len());
                c.patch(jend, c.code.len());
            } else if op == "||" {
                // short-circuit OR: a সত্য হলে a-ই ফলাফল
                compile_expr(c, l, ft)?;
                c.emit(Instr::Dup);
                let jt = c.emit(Instr::JmpIfTrue(0));
                c.emit(Instr::Pop);
                compile_expr(c, r, ft)?;
                let jend = c.emit(Instr::Jmp(0));
                c.patch(jt, c.code.len());
                c.patch(jend, c.code.len());
            } else {
                compile_expr(c, l, ft)?;
                compile_expr(c, r, ft)?;
                let instr = match op.as_str() {
                    "+" => Instr::Add,
                    "-" => Instr::Sub,
                    "*" => Instr::Mul,
                    "/" => Instr::Div,
                    "%" => Instr::Mod,
                    "==" => Instr::Eq,
                    "!=" => Instr::Ne,
                    "<" => Instr::Lt,
                    ">" => Instr::Gt,
                    "<=" => Instr::Le,
                    _ => Instr::Ge,
                };
                c.emit(instr);
            }
        }
        Expr::Call(name, args, _) => {
            let argc = args.len();
            if let Some(&(fidx, arity)) = ft.get(name) {
                if argc != arity {
                    return Err(CompileError {
                        msg: format!(
                            "ফাংশন '{}'-এ {}টি argument দরকার, {}টি দেওয়া হয়েছে ('{}' expects {} arguments, got {})",
                            name, arity, argc, name, arity, argc
                        ),
                    });
                }
                for a in args {
                    compile_expr(c, a, ft)?;
                }
                c.emit(Instr::CallFn(fidx, argc));
            } else if let Some(b) = lookup_builtin(name) {
                if argc != b.arity() {
                    return Err(CompileError {
                        msg: format!(
                            "built-in '{}'-এ {}টি argument দরকার, {}টি দেওয়া হয়েছে",
                            name,
                            b.arity(),
                            argc
                        ),
                    });
                }
                for a in args {
                    compile_expr(c, a, ft)?;
                }
                c.emit(Instr::CallB(b, argc));
            } else {
                return Err(CompileError {
                    msg: format!("অজানা ফাংশন '{}' (unknown function)", name),
                });
            }
        }
    }
    Ok(())
}

