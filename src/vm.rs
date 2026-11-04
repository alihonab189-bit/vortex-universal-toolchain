//! VORTEX Virtual Machine — VIR bytecode চালায়।
//!
//! Stack machine: value stack + call frames। প্রতিটি frame-এর নিজস্ব
//! slot array আছে (variable)। ত্রুটিগুলো দ্বিভাষিক (বাংলা + English)।

use crate::keywords::Builtin;
use crate::value::Value;
use crate::vir::{Instr, VProgram};

use std::cell::RefCell;
use std::rc::Rc;

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

pub struct Frame {
    pub fidx: usize,
    pub ip: usize,
    pub slots: Vec<Value>,
}

/// এক step-এর ফলাফল — চলতে থাকবে না প্রোগ্রাম শেষ
pub enum Step {
    Running,
    Finished(Value),
}

/// দৃশ্যমান (instrumented) VM — debugger ও test runner দুটোই এটা ব্যবহার করে।
/// প্রতিটি step() কল ঠিক একটি VIR instruction চালায়।
pub struct Machine<'p> {
    p: &'p VProgram,
    pub out: Vec<String>,
    pub stack: Vec<Value>,
    pub frames: Vec<Frame>,
    steps: u64,
}

impl<'p> Machine<'p> {
    pub fn new(p: &'p VProgram) -> Self {
        let n0 = p.funcs[p.entry].nslots;
        Machine {
            p,
            out: Vec::new(),
            stack: Vec::new(),
            frames: vec![Frame {
                fidx: p.entry,
                ip: 0,
                slots: vec![Value::Unit; n0],
            }],
            steps: 0,
        }
    }

    pub fn cur_func(&self) -> usize {
        self.frames.last().map(|f| f.fidx).unwrap_or(self.p.entry)
    }

    pub fn cur_ip(&self) -> usize {
        self.frames.last().map(|f| f.ip).unwrap_or(0)
    }

    pub fn func_name(&self, fidx: usize) -> &str {
        &self.p.funcs[fidx].name
    }

    /// ঠিক একটি instruction চালায়। Err মানে runtime ত্রুটি।
    pub fn step(&mut self) -> Result<Step, VmError> {
        let p = self.p;
        let stack = &mut self.stack;
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(VmError::new(
                "VM instruction সীমা ছাড়িয়ে গেছে (instruction limit exceeded)",
            ));
        }

        let cur = self.frames.last().unwrap().fidx;
        let ip = self.frames.last().unwrap().ip;
        let instr = match p.funcs[cur].code.get(ip) {
            Some(i) => i.clone(),
            None => {
                // code শেষ — চুপচাপ unit রিটার্ন (compiler সাধারণত RET বসায়)
                let rv = stack.pop().unwrap_or(Value::Unit);
                self.frames.pop();
                return match self.frames.last_mut() {
                    None => Ok(Step::Finished(rv)),
                    Some(_) => {
                        stack.push(rv);
                        Ok(Step::Running)
                    }
                };
            }
        };
        self.frames.last_mut().unwrap().ip += 1;

        match instr {
            Instr::Const(v) => stack.push(v),
            Instr::Load(s) => {
                let fr = self.frames.last().unwrap();
                match fr.slots.get(s) {
                    Some(v) => stack.push(v.clone()),
                    None => return Err(VmError::new("internal: bad slot (load)")),
                }
            }
            Instr::Store(s) => {
                let v = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow (store)"))?;
                let fr = self.frames.last_mut().unwrap();
                match fr.slots.get_mut(s) {
                    Some(dst) => *dst = v,
                    None => return Err(VmError::new("internal: bad slot (store)")),
                }
            }
            Instr::Add => binop(stack, "+")?,
            Instr::Sub => binop(stack, "-")?,
            Instr::Mul => binop(stack, "*")?,
            Instr::Div => binop(stack, "/")?,
            Instr::Mod => binop(stack, "%")?,
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
                let (b, a) = two(stack)?;
                stack.push(Value::Bool(values_eq(&a, &b)));
            }
            Instr::Ne => {
                let (b, a) = two(stack)?;
                stack.push(Value::Bool(!values_eq(&a, &b)));
            }
            Instr::Lt => {
                let (b, a) = two(stack)?;
                stack.push(Value::Bool(values_cmp("<", &a, &b)?));
            }
            Instr::Gt => {
                let (b, a) = two(stack)?;
                stack.push(Value::Bool(values_cmp(">", &a, &b)?));
            }
            Instr::Le => {
                let (b, a) = two(stack)?;
                stack.push(Value::Bool(values_cmp("<=", &a, &b)?));
            }
            Instr::Ge => {
                let (b, a) = two(stack)?;
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
            Instr::Jmp(t) => self.frames.last_mut().unwrap().ip = t,
            Instr::JmpIfFalse(t) => {
                let c = stack
                    .pop()
                    .ok_or_else(|| VmError::new("internal: stack underflow (branch)"))?;
                match c {
                    Value::Bool(false) => self.frames.last_mut().unwrap().ip = t,
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
                    Value::Bool(true) => self.frames.last_mut().unwrap().ip = t,
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
                if self.frames.len() >= MAX_CALL_DEPTH {
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
                self.frames.push(Frame { fidx, ip: 0, slots });
            }
            Instr::CallB(b, argc) => {
                if stack.len() < argc {
                    return Err(VmError::new("internal: stack underflow (builtin call)"));
                }
                let args: Vec<Value> = stack.split_off(stack.len() - argc);
                let out = &mut self.out;
                let r = exec_builtin(b, args, out)?;
                stack.push(r);
            }
            Instr::IndexGet => {
                if stack.len() < 2 {
                    return Err(VmError::new("internal: stack underflow (index)"));
                }
                let idx = stack.pop().unwrap();
                let obj = stack.pop().unwrap();
                let v = index_get(&obj, &idx)?;
                stack.push(v);
            }
            Instr::IndexSet => {
                if stack.len() < 3 {
                    return Err(VmError::new("internal: stack underflow (index set)"));
                }
                let val = stack.pop().unwrap();
                let idx = stack.pop().unwrap();
                let obj = stack.pop().unwrap();
                index_set(&obj, &idx, val)?;
            }
            Instr::MakeArr(n) => {
                if stack.len() < n {
                    return Err(VmError::new("internal: stack underflow (array literal)"));
                }
                let items: Vec<Value> = stack.split_off(stack.len() - n);
                stack.push(Value::Arr(Rc::new(RefCell::new(items))));
            }
            Instr::MakeMap(n) => {
                if stack.len() < n * 2 {
                    return Err(VmError::new("internal: stack underflow (map literal)"));
                }
                let flat: Vec<Value> = stack.split_off(stack.len() - n * 2);
                let mut pairs: Vec<(Value, Value)> = Vec::with_capacity(n);
                let mut it = flat.into_iter();
                while let (Some(k), Some(v)) = (it.next(), it.next()) {
                    pairs.push((k, v));
                }
                stack.push(Value::Map(Rc::new(RefCell::new(pairs))));
            }
            Instr::Ret => {
                let rv = stack.pop().unwrap_or(Value::Unit);
                self.frames.pop();
                return match self.frames.last_mut() {
                    None => Ok(Step::Finished(rv)),
                    Some(_) => {
                        stack.push(rv);
                        Ok(Step::Running)
                    }
                };
            }
        }
        Ok(Step::Running)
    }
}

/// প্রোগ্রামের entry (main) ফাংশন শেষ পর্যন্ত চালায়।
pub fn execute(p: &VProgram) -> Result<RunResult, VmError> {
    let mut m = Machine::new(p);
    loop {
        match m.step()? {
            Step::Running => {}
            Step::Finished(v) => {
                return Ok(RunResult {
                    value: v,
                    output: std::mem::take(&mut m.out),
                })
            }
        }
    }
}

/// নির্দিষ্ট ফাংশনকে entry করে চালায় — test runner-এর জন্য
pub fn execute_fn(p: &VProgram, name: &str) -> Result<RunResult, VmError> {
    let fidx = p
        .funcs
        .iter()
        .position(|f| f.name == name)
        .ok_or_else(|| VmError::new(format!("ফাংশন নেই (no such function): {}", name)))?;
    let mut m = Machine::new(p);
    m.frames[0] = Frame {
        fidx,
        ip: 0,
        slots: vec![Value::Unit; p.funcs[fidx].nslots],
    };
    loop {
        match m.step()? {
            Step::Running => {}
            Step::Finished(v) => {
                return Ok(RunResult {
                    value: v,
                    output: std::mem::take(&mut m.out),
                })
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
        (Value::Arr(x), Value::Arr(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_eq(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter()
                        .find(|(k2, _)| values_eq(k, k2))
                        .map_or(false, |(_, v2)| values_eq(v, v2))
                })
        }
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

fn index_get(obj: &Value, idx: &Value) -> Result<Value, VmError> {
    match (obj, idx) {
        (Value::Arr(a), Value::Int(i)) => {
            let len = a.borrow().len() as i64;
            let i = norm_index(*i, len)?;
            Ok(a.borrow()[i as usize].clone())
        }
        (Value::Str(s), Value::Int(i)) => {
            let chars: Vec<char> = s.chars().collect();
            let i = norm_index(*i, chars.len() as i64)?;
            Ok(Value::Str(chars[i as usize].to_string()))
        }
        (Value::Map(m), k) => {
            let m = m.borrow();
            for (mk, mv) in m.iter() {
                if values_eq(mk, k) {
                    return Ok(mv.clone());
                }
            }
            Err(VmError::new(format!(
                "map-এ চাবি নেই (key not found): {}",
                k.repr()
            )))
        }
        (o, i) => Err(VmError::new(format!(
            "{}-এর উপর index চলে না, index-এর ধরন {} (cannot index {} with {})",
            o.type_name(),
            i.type_name(),
            o.type_name(),
            i.type_name()
        ))),
    }
}

fn index_set(obj: &Value, idx: &Value, val: Value) -> Result<(), VmError> {
    match (obj, idx) {
        (Value::Arr(a), Value::Int(i)) => {
            let len = a.borrow().len() as i64;
            let i = norm_index(*i, len)?;
            a.borrow_mut()[i as usize] = val;
            Ok(())
        }
        (Value::Map(m), k) => {
            let mut m = m.borrow_mut();
            for (mk, mv) in m.iter_mut() {
                if values_eq(mk, k) {
                    *mv = val;
                    return Ok(());
                }
            }
            m.push((k.clone(), val));
            Ok(())
        }
        (o, i) => Err(VmError::new(format!(
            "{}-এর index-এ লেখা যায় না (cannot assign into index of {}) পাওয়া গেছে (got): {}",
            o.type_name(),
            o.type_name(),
            i.type_name()
        ))),
    }
}

/// ঋণাত্মক index সমর্থিত: -1 = শেষ উপাদান
fn norm_index(i: i64, len: i64) -> Result<i64, VmError> {
    let i = if i < 0 { i + len } else { i };
    if i < 0 || i >= len {
        Err(VmError::new(format!(
            "index সীমার বাইরে (index out of bounds): {} (দৈর্ঘ্য {})",
            i, len
        )))
    } else {
        Ok(i)
    }
}

fn exec_builtin(b: Builtin, mut args: Vec<Value>, out: &mut Vec<String>) -> Result<Value, VmError> {
    match b {
        Builtin::Show => {
            // variadic: show("a", x, 42) → "a x 42"
            let parts: Vec<String> = args.iter().map(|v| v.to_string()).collect();
            out.push(parts.join(" "));
            Ok(Value::Unit)
        }
        Builtin::Len => match args.remove(0) {
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            Value::Arr(a) => Ok(Value::Int(a.borrow().len() as i64)),
            Value::Map(m) => Ok(Value::Int(m.borrow().len() as i64)),
            other => Err(VmError::new(format!(
                "len() string/array/map নেয় (len() needs a string/array/map), পাওয়া গেছে (got): {}",
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
        Builtin::Push => {
            let v = args.remove(1);
            match args.remove(0) {
                Value::Arr(a) => {
                    a.borrow_mut().push(v);
                    Ok(Value::Unit)
                }
                other => Err(VmError::new(format!(
                    "push() শুধু array নেয় (push() needs an array), পাওয়া গেছে (got): {}",
                    other.type_name()
                ))),
            }
        }
        Builtin::Pop => match args.remove(0) {
            Value::Arr(a) => match a.borrow_mut().pop() {
                Some(v) => Ok(v),
                None => Err(VmError::new("খালি array থেকে pop করা যায় না (pop from empty array)")),
            },
            other => Err(VmError::new(format!(
                "pop() শুধু array নেয় (pop() needs an array), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Keys => match args.remove(0) {
            Value::Map(m) => Ok(Value::Arr(Rc::new(RefCell::new(
                m.borrow().iter().map(|(k, _)| k.clone()).collect(),
            )))),
            other => Err(VmError::new(format!(
                "keys() শুধু map নেয় (keys() needs a map), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Values => match args.remove(0) {
            Value::Map(m) => Ok(Value::Arr(Rc::new(RefCell::new(
                m.borrow().iter().map(|(_, v)| v.clone()).collect(),
            )))),
            other => Err(VmError::new(format!(
                "values() শুধু map নেয় (values() needs a map), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Has => {
            let k = args.remove(1);
            match args.remove(0) {
                Value::Map(m) => {
                    let found = m.borrow().iter().any(|(mk, _)| values_eq(mk, &k));
                    Ok(Value::Bool(found))
                }
                other => Err(VmError::new(format!(
                    "has() শুধু map নেয় (has() needs a map), পাওয়া গেছে (got): {}",
                    other.type_name()
                ))),
            }
        }
        Builtin::TypeOf => Ok(Value::Str(args.remove(0).type_name().to_string())),
        Builtin::Iter => match args.remove(0) {
            v @ Value::Arr(_) => Ok(v),
            Value::Map(m) => Ok(Value::Arr(Rc::new(RefCell::new(
                m.borrow().iter().map(|(k, _)| k.clone()).collect(),
            )))),
            Value::Str(s) => Ok(Value::Arr(Rc::new(RefCell::new(
                s.chars().map(|c| Value::Str(c.to_string())).collect(),
            )))),
            other => Err(VmError::new(format!(
                "এই ধরনের উপর for-in চলে না (not iterable): {}",
                other.type_name()
            ))),
        },
        Builtin::Floor => match args.remove(0) {
            Value::Float(x) => Ok(Value::Int(x.floor() as i64)),
            Value::Int(i) => Ok(Value::Int(i)),
            other => Err(VmError::new(format!(
                "floor() সংখ্যা নেয় (floor() needs a number), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Ceil => match args.remove(0) {
            Value::Float(x) => Ok(Value::Int(x.ceil() as i64)),
            Value::Int(i) => Ok(Value::Int(i)),
            other => Err(VmError::new(format!(
                "ceil() সংখ্যা নেয় (ceil() needs a number), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Abs => match args.remove(0) {
            Value::Int(i) => i.checked_abs().map(Value::Int).ok_or_else(overflow_err),
            Value::Float(x) => Ok(Value::Float(x.abs())),
            other => Err(VmError::new(format!(
                "abs() সংখ্যা নেয় (abs() needs a number), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Min => {
            let b = args.remove(1);
            let a = args.remove(0);
            let ord = num_cmp(&a, &b)?;
            Ok(if ord <= 0 { a } else { b })
        }
        Builtin::Max => {
            let b = args.remove(1);
            let a = args.remove(0);
            let ord = num_cmp(&a, &b)?;
            Ok(if ord >= 0 { a } else { b })
        }
        Builtin::Sqrt => match args.remove(0) {
            Value::Int(i) => Ok(Value::Float((i as f64).sqrt())),
            Value::Float(x) => Ok(Value::Float(x.sqrt())),
            other => Err(VmError::new(format!(
                "sqrt() সংখ্যা নেয় (sqrt() needs a number), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Pow => {
            let e = args.remove(1);
            let b = args.remove(0);
            match (b, e) {
                (Value::Int(x), Value::Int(y)) => {
                    if y < 0 {
                        Ok(Value::Float((x as f64).powi(y as i32)))
                    } else {
                        x.checked_pow(y as u32).map(Value::Int).ok_or_else(overflow_err)
                    }
                }
                (Value::Int(x), Value::Float(y)) => Ok(Value::Float((x as f64).powf(y))),
                (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x.powi(y as i32))),
                (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x.powf(y))),
                (a2, e2) => Err(type_err("^", &a2, &e2)),
            }
        }
        Builtin::Assert => {
            let v = args.remove(0);
            match v {
                Value::Bool(true) => Ok(Value::Unit),
                Value::Bool(false) => Err(VmError::new("assert ব্যর্থ (assertion failed)")),
                other => Err(VmError::new(format!(
                    "assert() true/false নেয়, পাওয়া গেছে (got): {}",
                    other.type_name()
                ))),
            }
        }
        Builtin::AssertEq => {
            let b = args.remove(1);
            let a = args.remove(0);
            if values_eq(&a, &b) {
                Ok(Value::Unit)
            } else {
                Err(VmError::new(format!(
                    "assert_eq ব্যর্থ (assertion failed): {} != {}",
                    a.repr(),
                    b.repr()
                )))
            }
        }
        Builtin::Upper => match args.remove(0) {
            Value::Str(s) => Ok(Value::Str(s.to_uppercase())),
            other => Err(VmError::new(format!(
                ".upper() শুধু string-এ চলে (.upper() needs a string), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Lower => match args.remove(0) {
            Value::Str(s) => Ok(Value::Str(s.to_lowercase())),
            other => Err(VmError::new(format!(
                ".lower() শুধু string-এ চলে (.lower() needs a string), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Trim => match args.remove(0) {
            Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
            other => Err(VmError::new(format!(
                ".trim() শুধু string-এ চলে (.trim() needs a string), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Split => {
            let sep = args.remove(1);
            match (args.remove(0), sep) {
                (Value::Str(s), Value::Str(sep)) => Ok(Value::Arr(Rc::new(RefCell::new(
                    s.split(&sep as &str)
                        .map(|p| Value::Str(p.to_string()))
                        .collect(),
                )))),
                (o, s2) => Err(VmError::new(format!(
                    ".split() string নেয় (needs strings), পাওয়া গেছে (got): {}, {}",
                    o.type_name(),
                    s2.type_name()
                ))),
            }
        }
        Builtin::Contains => {
            let needle = args.remove(1);
            match (args.remove(0), needle) {
                (Value::Str(s), Value::Str(n)) => Ok(Value::Bool(s.contains(&n))),
                (Value::Arr(a), v) => {
                    let found = a.borrow().iter().any(|x| values_eq(x, &v));
                    Ok(Value::Bool(found))
                }
                (o, n) => Err(VmError::new(format!(
                    ".contains() string/array নেয় (needs string/array), পাওয়া গেছে (got): {}, {}",
                    o.type_name(),
                    n.type_name()
                ))),
            }
        }
        Builtin::Replace => {
            let rep = args.remove(2);
            let old = args.remove(1);
            match (args.remove(0), old, rep) {
                (Value::Str(s), Value::Str(old), Value::Str(rep)) => {
                    Ok(Value::Str(s.replace(&old as &str, &rep)))
                }
                (o, o2, r) => Err(VmError::new(format!(
                    ".replace() string নেয় (needs strings), পাওয়া গেছে (got): {}, {}, {}",
                    o.type_name(),
                    o2.type_name(),
                    r.type_name()
                ))),
            }
        }
        Builtin::Chars => match args.remove(0) {
            Value::Str(s) => Ok(Value::Arr(Rc::new(RefCell::new(
                s.chars().map(|c| Value::Str(c.to_string())).collect(),
            )))),
            other => Err(VmError::new(format!(
                ".chars() শুধু string-এ চলে (.chars() needs a string), পাওয়া গেছে (got): {}",
                other.type_name()
            ))),
        },
        Builtin::Join => {
            let sep = args.remove(1);
            match (args.remove(0), sep) {
                (Value::Arr(a), Value::Str(sep)) => {
                    let parts: Vec<String> = a.borrow().iter().map(|v| v.to_string()).collect();
                    Ok(Value::Str(parts.join(&sep)))
                }
                (o, s2) => Err(VmError::new(format!(
                    ".join() array + string নেয় (needs array + string), পাওয়া গেছে (got): {}, {}",
                    o.type_name(),
                    s2.type_name()
                ))),
            }
        }
        Builtin::Repeat => {
            let n = args.remove(1);
            match (args.remove(0), n) {
                (Value::Str(s), Value::Int(n)) => {
                    if n < 0 {
                        return Err(VmError::new("সংখ্যা ঋণাত্মক হতে পারে না (negative count)"));
                    }
                    Ok(Value::Str(s.repeat(n as usize)))
                }
                (o, n2) => Err(VmError::new(format!(
                    ".repeat() string + int নেয় (needs string + int), পাওয়া গেছে (got): {}, {}",
                    o.type_name(),
                    n2.type_name()
                ))),
            }
        }
        Builtin::Sub => {
            let len = args.remove(2);
            let from = args.remove(1);
            match (args.remove(0), from, len) {
                (Value::Str(s), Value::Int(from), Value::Int(len)) => {
                    let chars: Vec<char> = s.chars().collect();
                    let from = if from < 0 { from + chars.len() as i64 } else { from };
                    if from < 0 || from > chars.len() as i64 {
                        return Err(VmError::new(format!(
                            "শুরু index সীমার বাইরে (start index out of bounds): {}",
                            from
                        )));
                    }
                    let end = (from + len).min(chars.len() as i64);
                    Ok(Value::Str(chars[from as usize..end as usize].iter().collect()))
                }
                (o, f2, l2) => Err(VmError::new(format!(
                    ".sub() string + int + int নেয় (needs string + int + int), পাওয়া গেছে (got): {}, {}, {}",
                    o.type_name(),
                    f2.type_name(),
                    l2.type_name()
                ))),
            }
        }
    }
}

/// min/max-এর জন্য সংখ্যা তুলনা (int/float mix সহ)
fn num_cmp(a: &Value, b: &Value) -> Result<i64, VmError> {
    let ord = match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => {
            x.partial_cmp(y).ok_or_else(|| VmError::new("NaN তুলনা করা যায় না (cannot compare NaN)"))?
        }
        (Value::Int(x), Value::Float(y)) => (*x as f64)
            .partial_cmp(y)
            .ok_or_else(|| VmError::new("NaN তুলনা করা যায় না (cannot compare NaN)"))?,
        (Value::Float(x), Value::Int(y)) => x
            .partial_cmp(&(*y as f64))
            .ok_or_else(|| VmError::new("NaN তুলনা করা যায় না (cannot compare NaN)"))?,
        _ => {
            return Err(VmError::new(format!(
                "min/max সংখ্যা নেয় (min/max need numbers), পাওয়া গেছে (got): {}, {}",
                a.type_name(),
                b.type_name()
            )))
        }
    };
    Ok(match ord {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    })
}
