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
}

impl Builtin {
    pub fn name(self) -> &'static str {
        match self {
            Builtin::Show => "show",
            Builtin::Len => "len",
            Builtin::Str => "str",
            Builtin::Num => "num",
        }
    }

    pub fn arity(self) -> usize {
        1
    }
}

pub fn lookup_builtin(s: &str) -> Option<Builtin> {
    match s {
        "show" | "দেখাও" | "দেখুৱাওক" => Some(Builtin::Show),
        "len" | "দৈৰ্ঘ্য" | "দৈর্ঘ্য" => Some(Builtin::Len),
        "str" | "স্ট্রিং" | "স্ট্ৰিং" => Some(Builtin::Str),
        "num" | "সংখ্যা" => Some(Builtin::Num),
        _ => None,
    }
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
];

/// Built-in ফাংশন টেবিল (নাম, aliases, বর্ণনা)
pub const BUILTIN_TABLE: &[(&str, &[&str], &str)] = &[
    (
        "show",
        &["show", "দেখাও", "দেখুৱাওক"],
        "মান প্রিন্ট করে (prints a value)",
    ),
    (
        "len",
        &["len", "দৈৰ্ঘ্য", "দৈর্ঘ্য"],
        "string-এর দৈর্ঘ্য (string length)",
    ),
    (
        "str",
        &["str", "স্ট্রিং", "স্ট্ৰিং"],
        "যেকোনো মানকে string-এ রূপান্তর (to string)",
    ),
    (
        "num",
        &["num", "সংখ্যা"],
        "string-কে সংখ্যায় রূপান্তর (to number)",
    ),
];
