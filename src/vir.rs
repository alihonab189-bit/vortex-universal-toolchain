//! VIR — Vortex Intermediate Representation
//!
//! Stack-based bytecode. Source → AST → VIR → VM।
//! VIR একটি পাঠযোগ্য টেক্সট ফরম্যাটে serialize হয় — `vortexc build` দিয়ে
//! .vxc ফাইলে সংরক্ষণ করা যায় এবং `vortexc exec` দিয়ে সরাসরি চালানো যায়।

use crate::keywords::{lookup_builtin, Builtin};
use crate::value::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    Const(Value),
    Load(usize),
    Store(usize),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Not,
    Dup,
    Pop,
    Jmp(usize),
    JmpIfFalse(usize),
    JmpIfTrue(usize),
    CallFn(usize, usize),   // (function index, argument count)
    CallB(Builtin, usize),  // (built-in, argument count)
    /// [..., obj, idx] → [..., value] — array/string/map index read
    IndexGet,
    /// [..., obj, idx, val] → [...] — array/map index write
    IndexSet,
    /// স্ট্যাকের উপরের n মান পপ করে array বানায় (উল্লেখ্য: লেখার ক্রম উল্টো)
    MakeArr(usize),
    /// উপরের 2n মান (k,v জোড়া) পপ করে map বানায়
    MakeMap(usize),
    Ret,
}

#[derive(Clone, Debug)]
pub struct Func {
    pub name: String,
    pub params: Vec<String>,
    pub nslots: usize,
    pub code: Vec<Instr>,
}

#[derive(Clone, Debug)]
pub struct VProgram {
    pub funcs: Vec<Func>,
    /// entry ফাংশনের index (main)
    pub entry: usize,
}

// ---------------- টেক্সট ফরম্যাট ----------------

pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// একটি instruction-এর টেক্সট রূপ (debugger-এর জন্য public)
pub fn write_instr_text(i: &Instr) -> String {
    instr_line(i)
}

fn instr_line(i: &Instr) -> String {
    match i {
        Instr::Const(v) => match v {
            Value::Int(n) => format!("CONST I {}", n),
            Value::Float(x) => format!("CONST F {:?}", x),
            Value::Str(s) => format!("CONST S {}", escape(s)),
            Value::Bool(b) => format!("CONST B {}", b),
            Value::Unit => "CONST U".into(),
            // array/map runtime-এ MakeArr/MakeMap দিয়ে তৈরি হয় — constant হিসেবে আসে না
            Value::Arr(_) | Value::Map(_) => unreachable!("container constant in VIR"),
        },
        Instr::Load(s) => format!("LD {}", s),
        Instr::Store(s) => format!("ST {}", s),
        Instr::Add => "ADD".into(),
        Instr::Sub => "SUB".into(),
        Instr::Mul => "MUL".into(),
        Instr::Div => "DIV".into(),
        Instr::Mod => "MOD".into(),
        Instr::Neg => "NEG".into(),
        Instr::Eq => "EQ".into(),
        Instr::Ne => "NE".into(),
        Instr::Lt => "LT".into(),
        Instr::Gt => "GT".into(),
        Instr::Le => "LE".into(),
        Instr::Ge => "GE".into(),
        Instr::Not => "NOT".into(),
        Instr::Dup => "DUP".into(),
        Instr::Pop => "POP".into(),
        Instr::Jmp(t) => format!("JMP {}", t),
        Instr::JmpIfFalse(t) => format!("JPF {}", t),
        Instr::JmpIfTrue(t) => format!("JPT {}", t),
        Instr::CallFn(f, a) => format!("CALLF {} {}", f, a),
        Instr::CallB(b, a) => format!("CALLB {} {}", b.name(), a),
        Instr::IndexGet => "IDXGET".into(),
        Instr::IndexSet => "IDXSET".into(),
        Instr::MakeArr(n) => format!("ARR {}", n),
        Instr::MakeMap(n) => format!("MAP {}", n),
        Instr::Ret => "RET".into(),
    }
}

/// পুরো প্রোগ্রামকে পাঠযোগ্য VIR টেক্সটে রূপান্তর করে।
pub fn write_text(p: &VProgram) -> String {
    let mut s = String::from("# VORTEX VIR v1\n");
    for f in &p.funcs {
        s.push_str(&format!(
            "FUNC {} {} {}\n",
            f.name,
            f.params.len(),
            f.nslots
        ));
        for i in &f.code {
            s.push_str(&instr_line(i));
            s.push('\n');
        }
    }
    s
}

/// VIR টেক্সট থেকে প্রোগ্রাম ফিরিয়ে আনে (build/exec roundtrip)।
pub fn parse_text(src: &str) -> Result<VProgram, String> {
    let mut funcs: Vec<Func> = Vec::new();
    let mut cur: Option<Func> = None;

    for raw in src.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let line = line.trim_start(); // শেষের space রাখতে হবে — CONST S string-এর অংশ
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        match op {
            "FUNC" => {
                if let Some(f) = cur.take() {
                    funcs.push(f);
                }
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() != 3 {
                    return Err(format!("ভুল FUNC লাইন (bad FUNC line): {}", line));
                }
                let arity: usize = parts[1]
                    .parse()
                    .map_err(|_| format!("ভুল arity: {}", line))?;
                let nslots: usize = parts[2]
                    .parse()
                    .map_err(|_| format!("ভুল nslots: {}", line))?;
                cur = Some(Func {
                    name: parts[0].to_string(),
                    params: (0..arity).map(|i| format!("p{}", i)).collect(),
                    nslots,
                    code: Vec::new(),
                });
            }
            _ => {
                let f = cur
                    .as_mut()
                    .ok_or_else(|| "FUNC-এর আগে instruction (instruction before FUNC)".to_string())?;
                f.code.push(parse_instr(op, rest)?);
            }
        }
    }
    if let Some(f) = cur.take() {
        funcs.push(f);
    }
    if funcs.is_empty() {
        return Err("খালি VIR (empty VIR)".into());
    }
    let entry = funcs
        .iter()
        .position(|f| f.name == "main")
        .ok_or_else(|| "VIR-এ main নেই (no main in VIR)".to_string())?;
    Ok(VProgram { funcs, entry })
}

fn parse_usize(s: &str) -> Result<usize, String> {
    s.parse::<usize>().map_err(|_| format!("ভুল সংখ্যা: {}", s))
}

fn parse_instr(op: &str, rest: &str) -> Result<Instr, String> {
    let bad = || format!("অজানা/ভুল VIR instruction: {} {}", op, rest);
    match op {
        "CONST" => {
            if rest.trim() == "U" {
                return Ok(Instr::Const(Value::Unit));
            }
            let (k, v) = rest.split_once(' ').ok_or_else(bad)?;
            Ok(match k {
                "I" => Instr::Const(Value::Int(v.parse::<i64>().map_err(|_| bad())?)),
                "F" => Instr::Const(Value::Float(v.parse::<f64>().map_err(|_| bad())?)),
                "S" => Instr::Const(Value::Str(unescape(v))),
                "B" => Instr::Const(Value::Bool(v == "true")),
                "U" => Instr::Const(Value::Unit),
                _ => return Err(bad()),
            })
        }
        "LD" => Ok(Instr::Load(parse_usize(rest)?)),
        "ST" => Ok(Instr::Store(parse_usize(rest)?)),
        "ADD" => Ok(Instr::Add),
        "SUB" => Ok(Instr::Sub),
        "MUL" => Ok(Instr::Mul),
        "DIV" => Ok(Instr::Div),
        "MOD" => Ok(Instr::Mod),
        "NEG" => Ok(Instr::Neg),
        "EQ" => Ok(Instr::Eq),
        "NE" => Ok(Instr::Ne),
        "LT" => Ok(Instr::Lt),
        "GT" => Ok(Instr::Gt),
        "LE" => Ok(Instr::Le),
        "GE" => Ok(Instr::Ge),
        "NOT" => Ok(Instr::Not),
        "DUP" => Ok(Instr::Dup),
        "POP" => Ok(Instr::Pop),
        "JMP" => Ok(Instr::Jmp(parse_usize(rest)?)),
        "JPF" => Ok(Instr::JmpIfFalse(parse_usize(rest)?)),
        "JPT" => Ok(Instr::JmpIfTrue(parse_usize(rest)?)),
        "CALLF" => {
            let (a, b) = rest.split_once(' ').ok_or_else(bad)?;
            Ok(Instr::CallFn(parse_usize(a)?, parse_usize(b)?))
        }
        "CALLB" => {
            let (name, argc) = rest.split_once(' ').ok_or_else(bad)?;
            let b = lookup_builtin(name).ok_or_else(bad)?;
            Ok(Instr::CallB(b, parse_usize(argc)?))
        }
        "IDXGET" => Ok(Instr::IndexGet),
        "IDXSET" => Ok(Instr::IndexSet),
        "ARR" => Ok(Instr::MakeArr(parse_usize(rest)?)),
        "MAP" => Ok(Instr::MakeMap(parse_usize(rest)?)),
        "RET" => Ok(Instr::Ret),
        _ => Err(bad()),
    }
}
