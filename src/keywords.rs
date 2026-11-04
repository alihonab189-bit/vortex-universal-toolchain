//! VORTEX multilingual keyword system
//!
//! VORTEX-এর মূল শক্তি: একই ভাষায় English, অসমীয়া (Assamese) এবং বাংলা (Bengali)
//! keyword ব্যবহার করা যায়। Lexer সবগুলোকে একই internal token-এ রূপান্তর করে।
//! ভবিষ্যতে এই টেবিল সম্পূর্ণ user-configurable হবে (Custom Keywords)।

use crate::token::Tok;

/// একটি word কোন token তৈরি করবে তা নির্ধারণ করে।
pub fn keyword(s: &str) -> Option<Tok> {
    match s {
        "let" | "ধরো" | "ধৰো" => Some(Tok::Let),
        "fn" | "কাজ" | "কাম" => Some(Tok::Fn),
        "if" | "যদি" => Some(Tok::If),
        "else" | "নহলে" | "নাহলে" => Some(Tok::Else),
        "while" | "যতক্ষণ" => Some(Tok::While),
        "for" | "লুপ" => Some(Tok::For),
        "in" | "ভিতৰত" | "মধ্যে" => Some(Tok::In),
        "return" | "দিয়া" | "দিয়ক" => Some(Tok::Return),
        "true" | "সঁচা" | "সত্য" => Some(Tok::True),
        "false" | "মিথ্যা" => Some(Tok::False),
        "break" | "বিৰাম" | "বিরতি" => Some(Tok::Break),
        "continue" | "আগুৱাই" | "এগিয়ে" => Some(Tok::Continue),
        _ => None,
    }
}

/// Built-in ফাংশনগুলো (ব্যবহারকারী নিজের ফাংশন দিয়ে এগুলোর নাম override করতে পারে)।
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Builtin {
    Show,
    Len,
    Str,
    Num,
    Push,
    Pop,
    Keys,
    Values,
    Has,
    TypeOf,
    Iter,
    Floor,
    Ceil,
    Abs,
    Min,
    Max,
    Sqrt,
    Pow,
    Upper,
    Lower,
    Trim,
    Split,
    Contains,
    Replace,
    Chars,
    Join,
    Repeat,
    Sub,
    Assert,
    AssertEq,
}

impl Builtin {
    pub fn name(self) -> &'static str {
        match self {
            Builtin::Show => "show",
            Builtin::Len => "len",
            Builtin::Str => "str",
            Builtin::Num => "num",
            Builtin::Push => "push",
            Builtin::Pop => "pop",
            Builtin::Keys => "keys",
            Builtin::Values => "values",
            Builtin::Has => "has",
            Builtin::TypeOf => "type",
            Builtin::Iter => "items",
            Builtin::Floor => "floor",
            Builtin::Ceil => "ceil",
            Builtin::Abs => "abs",
            Builtin::Min => "min",
            Builtin::Max => "max",
            Builtin::Sqrt => "sqrt",
            Builtin::Pow => "pow",
            Builtin::Upper => "upper",
            Builtin::Lower => "lower",
            Builtin::Trim => "trim",
            Builtin::Split => "split",
            Builtin::Contains => "contains",
            Builtin::Replace => "replace",
            Builtin::Chars => "chars",
            Builtin::Join => "join",
            Builtin::Repeat => "repeat",
            Builtin::Sub => "sub",
            Builtin::Assert => "assert",
            Builtin::AssertEq => "assert_eq",
        }
    }

    pub fn arity(self) -> usize {
        match self {
            Builtin::Show
            | Builtin::Len
            | Builtin::Str
            | Builtin::Num
            | Builtin::Pop
            | Builtin::Keys
            | Builtin::Values
            | Builtin::TypeOf
            | Builtin::Iter
            | Builtin::Floor
            | Builtin::Ceil
            | Builtin::Abs
            | Builtin::Sqrt
            | Builtin::Upper
            | Builtin::Lower
            | Builtin::Trim
            | Builtin::Split
            | Builtin::Chars
            | Builtin::Assert => 1,
            Builtin::Push
            | Builtin::Has
            | Builtin::Min
            | Builtin::Max
            | Builtin::Pow
            | Builtin::Contains
            | Builtin::Join
            | Builtin::Repeat
            | Builtin::AssertEq => 2,
            Builtin::Replace | Builtin::Sub => 3,
        }
    }
}

pub fn lookup_builtin(s: &str) -> Option<Builtin> {
    match s {
        "show" | "দেখাও" | "দেখুৱাওক" => Some(Builtin::Show),
        "len" | "দৈৰ্ঘ্য" | "দৈর্ঘ্য" => Some(Builtin::Len),
        "str" | "স্ট্রিং" | "স্ট্ৰিং" => Some(Builtin::Str),
        "num" | "সংখ্যা" => Some(Builtin::Num),
        "push" | "যোগ" | "জোড়া" => Some(Builtin::Push),
        "pop" | "বাহৰ" | "বের করো" => Some(Builtin::Pop),
        "keys" | "চাবি" | "চাবিকাঠি" => Some(Builtin::Keys),
        "values" | "মানবোৰ" | "মান" => Some(Builtin::Values),
        "has" | "আছে" => Some(Builtin::Has),
        "type" | "ধৰণ" | "প্রকার" => Some(Builtin::TypeOf),
        "items" | "তালিকা" | "তালিকা_করো" => Some(Builtin::Iter),
        "floor" | "নিচৰ" | "নিচে" => Some(Builtin::Floor),
        "ceil" | "ওপৰৰ" | "ওপরে" => Some(Builtin::Ceil),
        "abs" | "পৰম" | "পরম" => Some(Builtin::Abs),
        "min" | "সৰু" | "ছোট" => Some(Builtin::Min),
        "max" | "ডাঙৰ" | "বড়" => Some(Builtin::Max),
        "sqrt" | "বৰ্গমূল" => Some(Builtin::Sqrt),
        "pow" | "ঘাত" => Some(Builtin::Pow),
        "assert" | "নিশ্চিত" => Some(Builtin::Assert),
        "assert_eq" | "সমান_নিশ্চিত" => Some(Builtin::AssertEq),
        _ => None,
    }
}

/// Method কল (`s.upper()`, `a.push(x)`, `m.keys()`) → built-in রূপান্তর।
/// রিটার্ন: built-in + মোট arity (receiver সহ)।
pub fn method_builtin(name: &str) -> Option<(Builtin, usize)> {
    let (b, extra) = match name {
        "upper" | "ডাঙৰ_হাতৰ" | "বড়_হাতের" => (Builtin::Upper, 0),
        "lower" | "সৰু_হাতৰ" | "ছোট_হাতের" => (Builtin::Lower, 0),
        "trim" | "পৰিষ্কাৰ" | "পরিষ্কার" => (Builtin::Trim, 0),
        "split" | "ভাগ" | "ভাঙো" => (Builtin::Split, 1),
        "contains" | "আছে_নে" | "ধারণ_করে" => (Builtin::Contains, 1),
        "replace" | "বদল" | "বদলাও" => (Builtin::Replace, 2),
        "chars" | "অক্ষৰ" | "অক্ষর" => (Builtin::Chars, 0),
        "join" | "জোৰা" | "জোড়াো" => (Builtin::Join, 1),
        "repeat" | "পুনৰাই" | "পুনরায়" => (Builtin::Repeat, 1),
        "sub" | "অংশ" => (Builtin::Sub, 2),
        "len" | "দৈৰ্ঘ্য" | "দৈর্ঘ্য" => (Builtin::Len, 0),
        "push" | "যোগ" | "জোড়া" => (Builtin::Push, 1),
        "pop" | "বাহৰ" | "বের করো" => (Builtin::Pop, 0),
        "keys" | "চাবি" | "চাবিকাঠি" => (Builtin::Keys, 0),
        "values" | "মানবোৰ" | "মান" => (Builtin::Values, 0),
        "has" | "আছে" => (Builtin::Has, 1),
        _ => return None,
    };
    Some((b, 1 + extra))
}

/// `vortexc keywords` কমান্ডের জন্য প্রদর্শন-টেবিল (canonic নাম + aliases)
pub const KEYWORD_TABLE: &[(&str, &[&str])] = &[
    ("let", &["let", "ধরো", "ধৰো"]),
    ("fn", &["fn", "কাজ", "কাম"]),
    ("if", &["if", "যদি"]),
    ("else", &["else", "নহলে", "নাহলে"]),
    ("while", &["while", "যতক্ষণ"]),
    ("for", &["for", "লুপ"]),
    ("in", &["in", "ভিতৰত", "মধ্যে"]),
    ("return", &["return", "দিয়া", "দিয়ক"]),
    ("true", &["true", "সঁচা", "সত্য"]),
    ("false", &["false", "মিথ্যা"]),
    ("break", &["break", "বিৰাম", "বিরতি"]),
    ("continue", &["continue", "আগুৱাই", "এগিয়ে"]),
];

/// Built-in ফাংশন টেবিল (নাম, aliases, বর্ণনা)
pub const BUILTIN_TABLE: &[(&str, &[&str], &str)] = &[
    ("show", &["show", "দেখাও", "দেখুৱাওক"], "মান প্রিন্ট করে (prints a value)"),
    ("len", &["len", "দৈৰ্ঘ্য", "দৈর্ঘ্য"], "string/array/map-এর দৈর্ঘ্য (length)"),
    ("str", &["str", "স্ট্রিং", "স্ট্ৰিং"], "যেকোনো মানকে string-এ রূপান্তর (to string)"),
    ("num", &["num", "সংখ্যা"], "string-কে সংখ্যায় রূপান্তর (to number)"),
    ("push", &["push", "যোগ", "জোড়া"], "array-র শেষে মান যোগ (append to array)"),
    ("pop", &["pop", "বাহৰ", "বের করো"], "array-র শেষ মান বের করে (remove last)"),
    ("keys", &["keys", "চাবি", "চাবিকাঠি"], "map-এর সব চাবি (all keys of a map)"),
    ("values", &["values", "মানবোৰ", "মান"], "map-এর সব মান (all values of a map)"),
    ("has", &["has", "আছে"], "map-এ চাবি আছে কি না (key exists?)"),
    ("type", &["type", "ধৰণ", "প্রকার"], "মানের ধরনের নাম (type name)"),
    ("items", &["items", "তালিকা"], "for-in-এর জন্য তালিকা (iterable as array)"),
    ("floor", &["floor", "নিচৰ", "নিচে"], "নিচের পূর্ণসংখ্যা (round down)"),
    ("ceil", &["ceil", "ওপৰৰ", "ওপরে"], "ওপরের পূর্ণসংখ্যা (round up)"),
    ("abs", &["abs", "পৰম", "পরম"], "পরম মান (absolute value)"),
    ("min", &["min", "সৰু", "ছোট"], "ছোট মান (smaller of two)"),
    ("max", &["max", "ডাঙৰ", "বড়"], "বড় মান (larger of two)"),
    ("sqrt", &["sqrt", "বৰ্গমূল"], "বর্গমূল (square root)"),
    ("pow", &["pow", "ঘাত"], "ঘাত সংখ্যা (power)"),
    ("assert", &["assert", "নিশ্চিত"], "টেস্টে শর্ত যাচাই (test assertion)"),
    ("assert_eq", &["assert_eq", "সমান_নিশ্চিত"], "টেস্টে সমতা যাচাই (equality assertion)"),
    ("upper", &["method: .upper", "ডাঙৰ_হাতৰ"], "বড় হাতের অক্ষর (uppercase)"),
    ("lower", &["method: .lower", "সৰু_হাতৰ"], "ছোট হাতের অক্ষর (lowercase)"),
    ("trim", &["method: .trim", "পৰিষ্কাৰ"], "দুপাশের স্পেস কাটে (trim spaces)"),
    ("split", &["method: .split", "ভাগ"], "বিভাজকে ভাগ করে (split string)"),
    ("contains", &["method: .contains", "আছে_নে"], "অন্তর্ভুক্ত কি না (contains?)"),
    ("replace", &["method: .replace", "বদল"], "পুরনোকে নতুন দিয়ে বদল (replace)"),
    ("chars", &["method: .chars", "অক্ষৰ"], "অক্ষরের array (array of chars)"),
    ("join", &["method: .join", "জোৰা"], "array-কে জোড়া দেয় (join array)"),
    ("repeat", &["method: .repeat", "পুনৰাই"], "n বার পুনরাবৃত্তি (repeat n times)"),
    ("sub", &["method: .sub", "অংশ"], "অংশ string (substring)"),
];
