//! VORTEX Virtual Machine — VIR bytecode চালায়।
//!
//! Stack machine: value stack + call frames। প্রতিটি frame-এর নিজস্ব
//! slot array আছে (variable)। ত্রুটিগুলো দ্বিভাষিক (বাংলা + English)।

use crate::keywords::Builtin;
use crate::value::Value;
use crate::vir::{Instr, VProgram};

#[derive(Debug)]
pub struct VmError {
    pub msg: String,
}

impl VmError {
    fn new(msg: impl Into<String>) -> Self {
        VmError { msg: msg.into() }
    }
}

pub struct RunResult {
    pub value: Value,
    pub output: Vec<String>,
}

const MAX_CALL_DEPTH: usize = 10_000;
const MAX_STEPS: u64 = 200_000_000;

struct Frame {
    fidx: usize,
    ip: usize,
    slots: Vec<Value>,
}

/// প্রোগ্রামের entry (main) ফাংশন চালায়।
pub fn execute(p: &VProgram) -> Result<RunResult, VmError> {
    let mut out: Vec<String> = Vec::new();
    let mut stack: Vec<Value> = Vec::new();
    let n0 = p.funcs[p.entry].nslots;
    let mut frames: Vec<Frame> = vec![Frame {
        fidx: p.entry,
        ip: 0,
        slots: vec![Value::Unit; n0],
    }];

    let mut steps: u64 = 0;

    loop {
        steps += 1;
        if steps > MAX_STEPS {
            return Err(VmError::new(
                "VM instruction সীমা ছাড়িয়ে গেছে (instruction limit exceeded)",
            ));
        }

        let cur = frames.last().unwrap().fidx;
        let ip = frames.last().unwrap().ip;
        let instr = match p.funcs[cur].code.get(ip) {
            Some(i) => i.clone(),
            None => {
                // code শেষ — চুপচাপ unit রিটার্ন (compiler সাধারণত RET বসায়)
                let rv = stack.pop().unwrap_or(Value::Unit);
                frames.pop();
                match frames.last_mut() {
                    None => return Ok(RunResult { value: rv, output: out }),
                    Some(_) => stack.push(rv),
                }
                continue;
            }
        };
        frames.last_mut().unwrap().ip += 1;

        match instr {
            Instr::Const(v) => stack.push(v),
            Instr::Load(s) => {
                let fr = frames.last().unwrap();
                match fr.slots.get(s) {
                    Some(v) => stack.push(v.clone()),
                    None => return Err(VmError::new("internal: bad slot (load)")),
                }
            }
            Instr::Store(s) => {
                let v = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow (store)"))?;
                let fr = frames.last_mut().unwrap();
                match fr.slots.get_mut(s) {
                    Some(dst) => *dst = v,
                    None => return Err(VmError::new("internal: bad slot (store)")),
                }
            }
            Instr::Add => binop(&mut stack, "+")?,
            Instr::Sub => binop(&mut stack, "-")?,
            Instr::Mul => binop(&mut stack, "*")?,
            Instr::Div => binop(&mut stack, "/")?,
            Instr::Mod => binop(&mut stack, "%")?,
            Instr::Neg => {
                let a = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow"))?;
                match a {
                    Value::Int(x) => match x.checked_neg() {
                        Some(v) => stack.push(Value::Int(v)),
                        None => return Err(VmError::new("সংখ্যা খুব বড় (integer overflow)")),
                    },
                    Value::Float(x) => stack.push(Value::Float(-x)),
                    other => {
                        return Err(VmError::new(format!(
                            "unary '-' এই টাইপে চলে না (unsupported type): {}",
                            other.type_name()
                        )))
                    }
                }
            }
            Instr::Not => {
                let a = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow"))?;
                match a {
                    Value::Bool(b) => stack.push(Value::Bool(!b)),
                    other => {
                        return Err(VmError::new(format!(
                            "'!' এই টাইপে চলে না (unsupported type): {}",
                            other.type_name()
                        )))
                    }
                }
            }
            Instr::Eq => {
                let (b, a) = two(&mut stack)?;
                stack.push(Value::Bool(values_eq(&a, &b)));
            }
            Instr::Ne => {
                let (b, a) = two(&mut stack)?;
                stack.push(Value::Bool(!values_eq(&a, &b)));
            }
            Instr::Lt => {
                let (b, a) = two(&mut stack)?;
                stack.push(Value::Bool(values_cmp("<", &a, &b)?));
            }
            Instr::Gt => {
                let (b, a) = two(&mut stack)?;
                stack.push(Value::Bool(values_cmp(">", &a, &b)?));
            }
            Instr::Le => {
                let (b, a) = two(&mut stack)?;
                stack.push(Value::Bool(values_cmp("<=", &a, &b)?));
            }
            Instr::Ge => {
                let (b, a) = two(&mut stack)?;
                stack.push(Value::Bool(values_cmp(">=", &a, &b)?));
            }
            Instr::Dup => {
                let v = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| VmError::new("internal: stack underflow (dup)"))?;
                stack.push(v);
            }
            Instr::Pop => {
                stack.pop();
            }
            Instr::Jmp(t) => frames.last_mut().unwrap().ip = t,
            Instr::JmpIfFalse(t) => {
                let c = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow (branch)"))?;
                match c {
                    Value::Bool(false) => frames.last_mut().unwrap().ip = t,
                    Value::Bool(true) => {}
                    other => {
                        return Err(VmError::new(format!(
                            "শর্ত অবশ্যই true/false হতে হবে (condition must be true/false), পাওয়া গেছে (got): {}",
                            other.type_name()
                        )))
                    }
                }
            }
            Instr::JmpIfTrue(t) => {
                let c = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow (branch)"))?;
                match c {
                    Value::Bool(true) => frames.last_mut().unwrap().ip = t,
                    Value::Bool(false) => {}
                    other => {
                        return Err(VmError::new(format!(
                            "শর্ত অবশ্যই true/false হতে হবে (condition must be true/false), পাওয়া গেছে (got): {}",
                            other.type_name()
                        )))
                    }
                }
            }
            Instr::CallFn(fidx, argc) => {
                if frames.len() >= MAX_CALL_DEPTH {
                    return Err(VmError::new(
                        "কল স্ট্যাক খুব গভীর (call stack overflow — recursion খুব গভীর হয়ে গেছে)",
                    ));
                }
                if stack.len() < argc {
                    return Err(VmError::new("internal: stack underflow (call)"));
                }
                let args: Vec<Value> = stack.split_off(stack.len() - argc);
                let n = p.funcs[fidx].nslots;
                let mut slots = vec![Value::Unit; n];
                for (i, v) in args.into_iter().enumerate() {
                    slots[i] = v;
                }
                frames.push(Frame { fidx, ip: 0, slots });
            }
            Instr::CallB(b, argc) => {
                if stack.len() < argc {
                    return Err(VmError::new("internal: stack underflow (builtin call)"));
                }
                let args: Vec<Value> = stack.split_off(stack.len() - argc);
                let r = exec_builtin(b, args, &mut out)?;
                stack.push(r);
            }
            Instr::Ret => {
                let rv = stack.pop().unwrap_or(Value::Unit);
                frames.pop();
                match frames.last_mut() {
                    None => return Ok(RunResult { value: rv, output: out }),
                    Some(_) => stack.push(rv),
                }
            }
        }
    }
}

fn two(stack: &mut Vec<Value>) -> Result<(Value, Value), VmError> {
    let b = stack
        .pop()
        .ok_or_else(|| VmError::new("internal: stack underflow"))?;
    let a = stack
        .pop()
        .ok_or_else(|| VmError::new("internal: stack underflow"))?;
    Ok((b, a))
}

fn binop(stack: &mut Vec<Value>, op: &str) -> Result<(), VmError> {
    let (b, a) = two(stack)?;
    stack.push(arith(op, a, b)?);
    Ok(())
}

fn overflow_err() -> VmError {
    VmError::new("সংখ্যা খুব বড় (integer overflow)")
}

fn div0_err() -> VmError {
    VmError::new("ভাগশূন্য (division by zero)")
}

fn type_err(op: &str, a: &Value, b: &Value) -> VmError {
    VmError::new(format!(
        "'{}' এই টাইপগুলোতে চলে না (unsupported types): {} এবং (and) {}",
        op,
        a.type_name(),
        b.type_name()
    ))
}

fn arith(op: &str, a: Value, b: Value) -> Result<Value, VmError> {
    use Value::*;
    match op {
        "+" => {
            // string থাকলে concat — "নাম: " + 42 লিখলেই কাজ হয়
            if matches!(a, Str(_)) || matches!(b, Str(_)) {
                let mut s = String::new();
                s.push_str(&a.to_string());
                s.push_str(&b.to_string());
                return Ok(Str(s));
            }
            match (a, b) {
                (Int(x), Int(y)) => x
                    .checked_add(y)
                    .map(Int)
                    .ok_or_else(overflow_err),
                (Float(x), Float(y)) => Ok(Float(x + y)),
                (Int(x), Float(y)) => Ok(Float(x as f64 + y)),
                (Float(x), Int(y)) => Ok(Float(x + y as f64)),
                (a2, b2) => Err(type_err("+", &a2, &b2)),
            }
        }
        "-" => match (a, b) {
            (Int(x), Int(y)) => x
                .checked_sub(y)
                .map(Int)
                .ok_or_else(overflow_err),
            (Float(x), Float(y)) => Ok(Float(x - y)),
            (Int(x), Float(y)) => Ok(Float(x as f64 - y)),
            (Float(x), Int(y)) => Ok(Float(x - y as f64)),
            (a2, b2) => Err(type_err("-", &a2, &b2)),
        },
        "*" => match (a, b) {
            (Int(x), Int(y)) => x
                .checked_mul(y)
                .map(Int)
                .ok_or_else(overflow_err),
            (Float(x), Float(y)) => Ok(Float(x * y)),
            (Int(x), Float(y)) => Ok(Float(x as f64 * y)),
            (Float(x), Int(y)) => Ok(Float(x * y as f64)),
            (a2, b2) => Err(type_err("*", &a2, &b2)),
        },
        "/" => match (a, b) {
            (Int(x), Int(y)) => {
                if y == 0 {
                    return Err(div0_err());
                }
                Ok(Float(x as f64 / y as f64))
            }
            (Float(x), Float(y)) => {
                if y == 0.0 {
                    return Err(div0_err());
                }
                Ok(Float(x / y))
            }
            (Int(x), Float(y)) => {
                if y == 0.0 {
                    return Err(div0_err());
                }
                Ok(Float(x as f64 / y))
            }
            (Float(x), Int(y)) => {
                if y == 0 {
                    return Err(div0_err());
                }
                Ok(Float(x / y as f64))
            }
            (a2, b2) => Err(type_err("/", &a2, &b2)),
        },
        "%" => match (a, b) {
            (Int(x), Int(y)) => {
                if y == 0 {
                    return Err(div0_err());
                }
                Ok(Int(x % y))
            }
            (Float(x), Float(y)) => {
                if y == 0.0 {
                    return Err(div0_err());
                }
                Ok(Float(x % y))
            }
            (Int(x), Float(y)) => {
                if y == 0.0 {
                    return Err(div0_err());
                }
                Ok(Float(x as f64 % y))
            }
            (Float(x), Int(y)) => {
                if y == 0 {
                    return Err(div0_err());
                }
                Ok(Float(x % y as f64))
            }
            (a2, b2) => Err(type_err("%", &a2, &b2)),
        },
        _ => Err(VmError::new("internal: unknown op")),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f64),
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Unit, Value::Unit) => true,
        _ => false,
    }
}

fn values_cmp(op: &str, a: &Value, b: &Value) -> Result<bool, VmError> {
    use std::cmp::Ordering;
    let ord: Option<Ordering> = match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Int(x), Value::Float(y)) => (*x as f64).partial_cmp(y),
        (Value::Float(x), Value::Int(y)) => x.partial_cmp(&(*y as f64)),
        (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
        (a2, b2) => {
            return Err(VmError::new(format!(
                "'{}' এই টাইপগুলোর মধ্যে তুলনা করা যায় না (cannot compare types): {} এবং (and) {}",
                op,
                a2.type_name(),
                b2.type_name()
            )))
        }
    };
    Ok(match op {
        "<" => ord == Some(Ordering::Less),
        ">" => ord == Some(Ordering::Greater),
        "<=" => ord != Some(Ordering::Greater),
        _ => ord != Some(Ordering::Less),
    })
}

fn exec_builtin(b: Builtin, mut args: Vec<Value>, out: &mut Vec<String>) -> Result<Value, VmError> {
    match b {
        Builtin::Show => {
            let v = args.remove(0);
            out.push(v.to_string());
            Ok(Value::Unit)
        }
        Builtin::Len => match args.remove(0) {
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            other => Err(VmError::new(format!(
                "len() শুধু string নেয় (len() needs a string), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Str => Ok(Value::Str(args.remove(0).to_string())),
        Builtin::Num => match args.remove(0) {
            Value::Str(s) => {
                let t = s.trim();
                if let Ok(i) = t.parse::<i64>() {
                    Ok(Value::Int(i))
                } else if let Ok(f) = t.parse::<f64>() {
                    Ok(Value::Float(f))
                } else {
                    Err(VmError::new(format!(
                        "সংখ্যায় রূপান্তর করা যায়নি (cannot convert to number): \"{}\"",
                        t
                    )))
                }
            }
            Value::Int(i) => Ok(Value::Int(i)),
            Value::Float(f) => Ok(Value::Float(f)),
            other => Err(VmError::new(format!(
                "num() শুধু string/int/float নেয় (num() needs a string/int/float), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
    }
}
