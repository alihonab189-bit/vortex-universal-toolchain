//! Lexer (Tokenizer) — source code-কে token-এ ভাঙে।
//!
//! Unicode-aware: English, অসমীয়া (Assamese) ও বাংলা (Bengali) — দুই ধরনের
//! identifier ও keyword সরাসরি চলে (ৰ ৱ সহ)।

use crate::keywords::keyword;
use crate::token::{Tok, Token};

#[derive(Debug)]
pub struct LexError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

struct Lexer {
    c: Vec<char>,
    i: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.c.get(self.i + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.c.get(self.i).copied();
        if let Some(c0) = ch {
            self.i += 1;
            if c0 == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    fn err(&self, msg: impl Into<String>) -> LexError {
        LexError {
            msg: msg.into(),
            line: self.line,
            col: self.col,
        }
    }
}

/// ভারতীয় লিপির combining marks (matra, hasanta/virama, nukta ইত্যাদি) —
/// এগুলো `is_alphanumeric()` false দেয় কিন্তু identifier-এর অংশ হতে পারে।
/// (Devanagari, Bengali/Assamese, Gurmukhi, Oriya, Telugu, Kannada, Malayalam)
fn is_indic_mark(c: char) -> bool {
    matches!(c,
        '\u{0900}'..='\u{0903}'
        | '\u{093A}'..='\u{094F}'
        | '\u{0951}'..='\u{0957}'
        | '\u{0962}'..='\u{0963}'
        | '\u{0981}'..='\u{0983}'
        | '\u{09BC}'
        | '\u{09BE}'..='\u{09CC}'
        | '\u{09CD}'
        | '\u{09D7}'
        | '\u{09E2}'..='\u{09E3}'
        | '\u{09FA}'..='\u{09FD}'
        | '\u{0A01}'..='\u{0A03}'
        | '\u{0A3C}'..='\u{0A4F}'
        | '\u{0A51}'
        | '\u{0A70}'..='\u{0A71}'
        | '\u{0A82}'..='\u{0A83}'
        | '\u{0B01}'..='\u{0B03}'
        | '\u{0B3C}'..='\u{0B4F}'
        | '\u{0B56}'..='\u{0B57}'
        | '\u{0B62}'..='\u{0B63}'
        | '\u{0C00}'..='\u{0C03}'
        | '\u{0C3E}'..='\u{0C4F}'
        | '\u{0D00}'..='\u{0D03}'
        | '\u{0D3B}'..='\u{0D4F}'
        | '\u{0D62}'..='\u{0D63}'
    )
}

fn is_ident_cont(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || is_indic_mark(c)
}

pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    let mut lx = Lexer {
        c: src.chars().collect(),
        i: 0,
        line: 1,
        col: 1,
    };
    let mut out: Vec<Token> = Vec::new();

    loop {
        // সাদা স্পেস ও comment স্কিপ
        loop {
            match lx.peek() {
                Some(ch) if ch.is_whitespace() => {
                    lx.bump();
                }
                Some('/') if lx.peek2() == Some('/') => {
                    while let Some(c0) = lx.peek() {
                        if c0 == '\n' {
                            break;
                        }
                        lx.bump();
                    }
                }
                Some('/') if lx.peek2() == Some('*') => {
                    let (sl, sc) = (lx.line, lx.col);
                    lx.bump();
                    lx.bump();
                    let mut closed = false;
                    while let Some(c0) = lx.bump() {
                        if c0 == '*' && lx.peek() == Some('/') {
                            lx.bump();
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        return Err(LexError {
                            msg: "বন্ধ না হওয়া block comment (unterminated block comment)".into(),
                            line: sl,
                            col: sc,
                        });
                    }
                }
                _ => break,
            }
        }

        let (line, col) = (lx.line, lx.col);
        let ch = match lx.peek() {
            None => {
                out.push(Token {
                    tok: Tok::Eof,
                    line,
                    col,
                });
                return Ok(out);
            }
            Some(c0) => c0,
        };

        // সংখ্যা — ASCII ও ভারতীয় অংক ০-৯ (U+09E6–U+09EF)
        if ch.is_ascii_digit() || ('\u{09E6}'..='\u{09EF}').contains(&ch) {
            let dv = |c: char| -> u32 {
                if c.is_ascii_digit() {
                    c as u32 - '0' as u32
                } else {
                    c as u32 - '\u{09E6}' as u32
                }
            };
            let mut s = String::new();
            while let Some(c0) = lx.peek() {
                if c0.is_ascii_digit() || ('\u{09E6}'..='\u{09EF}').contains(&c0) {
                    s.push(char::from_u32('0' as u32 + dv(c0)).unwrap());
                    lx.bump();
                } else {
                    break;
                }
            }
            let mut is_float = false;
            if lx.peek() == Some('.')
                && matches!(lx.peek2(), Some(d) if d.is_ascii_digit() || ('\u{09E6}'..='\u{09EF}').contains(&d))
            {
                is_float = true;
                s.push('.');
                lx.bump();
                while let Some(c0) = lx.peek() {
                    if c0.is_ascii_digit() || ('\u{09E6}'..='\u{09EF}').contains(&c0) {
                        s.push(char::from_u32('0' as u32 + dv(c0)).unwrap());
                        lx.bump();
                    } else {
                        break;
                    }
                }
            }
            let tok = if is_float {
                Tok::Float(s.parse::<f64>().map_err(|_| lx.err("invalid float"))?)
            } else {
                Tok::Int(s.parse::<i64>().map_err(|_| lx.err("invalid integer"))?)
            };
            out.push(Token { tok, line, col });
            continue;
        }

        // String literal
        if ch == '"' {
            lx.bump();
            let mut s = String::new();
            loop {
                match lx.bump() {
                    None => {
                        return Err(LexError {
                            msg: "বন্ধ না হওয়া string (unterminated string)".into(),
                            line,
                            col,
                        })
                    }
                    Some('"') => break,
                    Some('\n') => {
                        return Err(LexError {
                            msg: "string-এর মাঝে নতুন লাইন (newline inside string)".into(),
                            line: lx.line - 1,
                            col,
                        })
                    }
                    Some('\\') => match lx.bump() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('r') => s.push('\r'),
                        Some('0') => s.push('\0'),
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some(o) => {
                            return Err(LexError {
                                msg: format!("অজানা escape (unknown escape): \\{}", o),
                                line: lx.line,
                                col: lx.col,
                            })
                        }
                        None => {
                            return Err(LexError {
                                msg: "বন্ধ না হওয়া string (unterminated string)".into(),
                                line,
                                col,
                            })
                        }
                    },
                    Some(o) => s.push(o),
                }
            }
            out.push(Token {
                tok: Tok::Str(s),
                line,
                col,
            });
            continue;
        }

        // Identifier / keyword — Unicode সমর্থিত (অসমীয়া ৰ ৱ, matra/hasanta সহ)
        if ch.is_alphabetic() || ch == '_' {
            let mut s = String::new();
            while let Some(c0) = lx.peek() {
                if is_ident_cont(c0) {
                    s.push(c0);
                    lx.bump();
                } else {
                    break;
                }
            }
            let tok = keyword(&s).unwrap_or(Tok::Ident(s));
            out.push(Token { tok, line, col });
            continue;
        }

        // দুই-অক্ষরের symbol
        let two: Option<Tok> = match ch {
            '=' if lx.peek2() == Some('=') => Some(Tok::EqEq),
            '!' if lx.peek2() == Some('=') => Some(Tok::NotEq),
            '<' if lx.peek2() == Some('=') => Some(Tok::Le),
            '>' if lx.peek2() == Some('=') => Some(Tok::Ge),
            '&' if lx.peek2() == Some('&') => Some(Tok::AndAnd),
            '|' if lx.peek2() == Some('|') => Some(Tok::OrOr),
            '.' if lx.peek2() == Some('.') => Some(Tok::DotDot),
            _ => None,
        };
        if let Some(t) = two {
            lx.bump();
            lx.bump();
            out.push(Token { tok: t, line, col });
            continue;
        }

        // এক-অক্ষরের symbol
        let one = match ch {
            '+' => Tok::Plus,
            '-' => Tok::Minus,
            '*' => Tok::Star,
            '/' => Tok::Slash,
            '%' => Tok::Percent,
            '<' => Tok::Lt,
            '>' => Tok::Gt,
            '=' => Tok::Eq,
            '!' => Tok::Not,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '{' => Tok::LBrace,
            '}' => Tok::RBrace,
            ',' => Tok::Comma,
            ';' => Tok::Semi,
            '[' => Tok::LBracket,
            ']' => Tok::RBracket,
            ':' => Tok::Colon,
            '.' => Tok::Dot,
            _ => {
                return Err(LexError {
                    msg: format!("অপ্রত্যাশিত অক্ষর (unexpected character): '{}'", ch),
                    line,
                    col,
                })
            }
        };
        lx.bump();
        out.push(Token { tok: one, line, col });
    }
}
