//! Runtime value সংজ্ঞা — VM-এর ভেতরের ডেটা টাইপ।
//!
//! v0.2: Array ও Map যোগ হয়েছে। Rc<RefCell<...>> দিয়ে shared mutable
//! data — তাই `push(a, x)` করলে `a`-র ভেতরের মান বদলায়।
//! Map insertion-order ধরে রাখে (linear scan — v0.2-এ যথেষ্ট)।

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    /// তালিকা — shared mutable
    Arr(Rc<RefCell<Vec<Value>>>),
    /// কী-মান জোড়া — insertion-order সহ, shared mutable
    Map(Rc<RefCell<Vec<(Value, Value)>>>),
    Unit,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Bool(_) => "bool",
            Value::Arr(_) => "array",
            Value::Map(_) => "map",
            Value::Unit => "unit",
        }
    }

    /// নতুন খালি array
    pub fn new_arr() -> Value {
        Value::Arr(Rc::new(RefCell::new(Vec::new())))
    }

    /// নতুন খালি map
    pub fn new_map() -> Value {
        Value::Map(Rc::new(RefCell::new(Vec::new())))
    }

    /// কনটেইনারের ভেতরে string-কে উদ্ধৃতি চিহ্ন সহ দেখায় (repr)
    pub fn repr(&self) -> String {
        match self {
            Value::Str(s) => format!("\"{}\"", s),
            Value::Arr(a) => {
                let items: Vec<String> = a.borrow().iter().map(|v| v.repr()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Map(m) => {
                let pairs: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.repr()))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            other => other.to_string(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(x) => write!(f, "{:?}", x),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Arr(a) => {
                let items: Vec<String> = a.borrow().iter().map(|v| v.repr()).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Map(m) => {
                let pairs: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.repr()))
                    .collect();
                write!(f, "{{{}}}", pairs.join(", "))
            }
            Value::Unit => write!(f, "unit"),
        }
    }
}
