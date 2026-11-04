//! Recursive-descent Parser — token list থেকে AST তৈরি করে।
//!
//! Grammar (EBNF):
//!   program  = stmt*
//!   stmt     = "let" IDENT "=" expr ";"
//!            | IDENT "=" expr ";"
//!            | "if" expr block ("else" (block | if_stmt))?
//!            | "while" expr block
//!            | "for" IDENT "in" expr ".." expr block
//!            | "fn" IDENT "(" params? ")" block
//!            | "return" expr? ";"
//!            | expr ";"
//!   expr     = or_expr ; সবচেয়ে কম precedence: ||, তারপর &&, তারপর comparison,
//!              additive, multiplicative, unary, primary

use crate::ast::{Expr, ForIter, Program, Stmt};
use crate::lexer::lex;
use crate::token::{Tok, Token};

#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

pub fn parse(src: &str) -> Result<Program, ParseError> {
    let toks = lex(src).map_err(|e| ParseError {
        msg: e.msg,
        line: e.line,
        col: e.col,
    })?;
    let mut p = Parser { t: toks, i: 0 };
    let stmts = p.stmts_until_eof()?;
    Ok(Program { stmts })
}

struct Parser {
    t: Vec<Token>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.t[self.i]
    }

    fn peek2(&self) -> &Token {
        &self.t[(self.i + 1).min(self.t.len() - 1)]
    }

    fn next(&mut self) -> Token {
        let tk = self.t[self.i].clone();
        if self.i < self.t.len() - 1 {
            self.i += 1;
        }
        tk
    }

    fn at(&self, t: &Tok) -> bool {
        std::mem::discriminant(&self.peek().tok) == std::mem::discriminant(t)
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        let tk = self.peek();
        ParseError {
            msg: msg.into(),
            line: tk.line,
            col: tk.col,
        }
    }

    fn expect(&mut self, t: &Tok, what: &str) -> Result<Token, ParseError> {
        if self.at(t) {
            Ok(self.next())
        } else {
            Err(self.err(format!(
                "{} প্রত্যাশিত (expected {}), কিন্তু পাওয়া গেছে {}",
                what,
                what,
                self.peek().describe()
            )))
        }
    }

    fn stmts_until_eof(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut v = Vec::new();
        while !self.at(&Tok::Eof) {
            v.push(self.stmt()?);
        }
        Ok(v)
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect(&Tok::LBrace, "'{'")?;
        let mut v = Vec::new();
        while !self.at(&Tok::RBrace) {
            if self.at(&Tok::Eof) {
                return Err(self.err("বন্ধ না হওয়া block (unterminated block): '}' প্রত্যাশিত"));
            }
            v.push(self.stmt()?);
        }
        self.next(); // consume '}'
        Ok(v)
    }

    fn stmt(&mut self) -> Result<Stmt, ParseError> {
        let tk = self.peek().clone();
        match tk.tok {
            Tok::Let => {
                self.next();
                let name = self.ident()?;
                self.expect(&Tok::Eq, "'='")?;
                let e = self.expr()?;
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Let(name, e))
            }
            Tok::Fn => {
                self.next();
                let name = self.ident()?;
                self.expect(&Tok::LParen, "'('")?;
                let mut params = Vec::new();
                if !self.at(&Tok::RParen) {
                    loop {
                        params.push(self.ident()?);
                        if self.at(&Tok::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RParen, "')'")?;
                let body = self.block()?;
                Ok(Stmt::FnDecl { name, params, body })
            }
            Tok::If => self.if_stmt(),
            Tok::While => {
                self.next();
                let cond = self.expr()?;
                let body = self.block()?;
                Ok(Stmt::While { cond, body })
            }
            Tok::For => {
                self.next();
                let var = self.ident()?;
                self.expect(&Tok::In, "'in' (ভিতৰত/মধ্যে)")?;
                let first = self.expr()?;
                let iter = if self.at(&Tok::DotDot) {
                    self.next();
                    let to = self.expr()?;
                    ForIter::Range(Box::new(first), Box::new(to))
                } else {
                    ForIter::Coll(Box::new(first))
                };
                let body = self.block()?;
                Ok(Stmt::For { var, iter, body })
            }
            Tok::Break => {
                self.next();
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Break)
            }
            Tok::Continue => {
                self.next();
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Continue)
            }
            Tok::Return => {
                self.next();
                let e = if self.at(&Tok::Semi) {
                    None
                } else {
                    Some(self.expr()?)
                };
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Return(e))
            }
            Tok::Ident(_) if matches!(&self.peek2().tok, Tok::Eq) => {
                let name = self.ident()?;
                self.next(); // '='
                let e = self.expr()?;
                self.expect(&Tok::Semi, "';'")?;
                Ok(Stmt::Assign(name, e, tk.line as u32))
            }
            // a[0] = v / m["ক"] = v — postfix expression দিয়ে শুরু হয় '=' পেলে
            _ => {
                let start = self.i;
                let e = self.expr()?;
                if self.at(&Tok::Eq) {
                    match e {
                        Expr::Index(obj, idx, line) => {
                            self.next(); // '='
                            let val = self.expr()?;
                            self.expect(&Tok::Semi, "';'")?;
                            Ok(Stmt::IndexAssign(obj, idx, Box::new(val), line))
                        }
                        _ => {
                            // রোলব্যাক নেই — ভুল বার্তা পরিষ্কার দিই
                            let _ = start;
                            Err(self.err(
                                "এখানে '=' চলে না — শুধু variable বা index-এ assign করা যায় (cannot assign to this expression)",
                            ))
                        }
                    }
                } else {
                    self.expect(&Tok::Semi, "';'")?;
                    Ok(Stmt::ExprStmt(e))
                }
            }
        }
    }

    fn if_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // 'if'
        let cond = self.expr()?;
        let then_b = self.block()?;
        let else_b = if self.at(&Tok::Else) {
            self.next();
            if self.at(&Tok::If) {
                Some(vec![self.if_stmt()?])
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_b,
            else_b,
        })
    }

    fn ident(&mut self) -> Result<String, ParseError> {
        if let Tok::Ident(s) = &self.peek().tok {
            let s = s.clone();
            self.next();
            Ok(s)
        } else {
            Err(self.err("identifier প্রত্যাশিত (expected identifier)"))
        }
    }

    // ---------- expressions ----------

    fn expr(&mut self) -> Result<Expr, ParseError> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.and_expr()?;
        while self.at(&Tok::OrOr) {
            self.next();
            let r = self.and_expr()?;
            l = Expr::Binary("||".into(), Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.cmp_expr()?;
        while self.at(&Tok::AndAnd) {
            self.next();
            let r = self.cmp_expr()?;
            l = Expr::Binary("&&".into(), Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn cmp_expr(&mut self) -> Result<Expr, ParseError> {
        let l = self.add_expr()?;
        let op = if self.at(&Tok::EqEq) {
            "=="
        } else if self.at(&Tok::NotEq) {
            "!="
        } else if self.at(&Tok::Lt) {
            "<"
        } else if self.at(&Tok::Gt) {
            ">"
        } else if self.at(&Tok::Le) {
            "<="
        } else if self.at(&Tok::Ge) {
            ">="
        } else {
            return Ok(l);
        };
        self.next();
        let r = self.add_expr()?;
        Ok(Expr::Binary(op.into(), Box::new(l), Box::new(r)))
    }

    fn add_expr(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.mul_expr()?;
        loop {
            let op = if self.at(&Tok::Plus) {
                "+"
            } else if self.at(&Tok::Minus) {
                "-"
            } else {
                break;
            };
            self.next();
            let r = self.mul_expr()?;
            l = Expr::Binary(op.into(), Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn mul_expr(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.unary()?;
        loop {
            let op = if self.at(&Tok::Star) {
                "*"
            } else if self.at(&Tok::Slash) {
                "/"
            } else if self.at(&Tok::Percent) {
                "%"
            } else {
                break;
            };
            self.next();
            let r = self.unary()?;
            l = Expr::Binary(op.into(), Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.at(&Tok::Minus) {
            self.next();
            let e = self.unary()?;
            return Ok(Expr::Unary("-".into(), Box::new(e)));
        }
        if self.at(&Tok::Not) {
            self.next();
            let e = self.unary()?;
            return Ok(Expr::Unary("!".into(), Box::new(e)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.primary_base()?;
        // postfix chain: a[0], m["key"], s.upper(), a.push(x) — চেইন করা যায়
        loop {
            if self.at(&Tok::LBracket) {
                self.next();
                let idx = self.expr()?;
                self.expect(&Tok::RBracket, "']'")?;
                e = Expr::Index(Box::new(e), Box::new(idx), 0);
            } else if self.at(&Tok::Dot) {
                let tk = self.peek().clone();
                self.next();
                let name = match &self.peek().tok {
                    Tok::Ident(s) => s.clone(),
                    other => {
                        return Err(ParseError {
                            msg: format!(
                                "'.'-এর পরে method-এর নাম দরকার (expected method name after '.'), পাওয়া গেছে {}",
                                Token { tok: other.clone(), line: tk.line, col: tk.col }.describe()
                            ),
                            line: tk.line,
                            col: tk.col,
                        })
                    }
                };
                self.next();
                self.expect(&Tok::LParen, "'('")?;
                let mut args = Vec::new();
                if !self.at(&Tok::RParen) {
                    loop {
                        args.push(self.expr()?);
                        if self.at(&Tok::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RParen, "')'")?;
                e = Expr::Method(Box::new(e), name, args, tk.line as u32);
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn primary_base(&mut self) -> Result<Expr, ParseError> {
        let tk = self.peek().clone();
        match tk.tok {
            Tok::Int(v) => {
                self.next();
                Ok(Expr::Int(v))
            }
            Tok::Float(v) => {
                self.next();
                Ok(Expr::Float(v))
            }
            Tok::Str(v) => {
                self.next();
                Ok(Expr::Str(v))
            }
            Tok::True => {
                self.next();
                Ok(Expr::Bool(true))
            }
            Tok::False => {
                self.next();
                Ok(Expr::Bool(false))
            }
            Tok::LParen => {
                self.next();
                let e = self.expr()?;
                self.expect(&Tok::RParen, "')'")?;
                Ok(e)
            }
            Tok::LBracket => {
                self.next();
                let mut items = Vec::new();
                if !self.at(&Tok::RBracket) {
                    loop {
                        items.push(self.expr()?);
                        if self.at(&Tok::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RBracket, "']'")?;
                Ok(Expr::Array(items))
            }
            Tok::LBrace => {
                self.next();
                let mut pairs = Vec::new();
                if !self.at(&Tok::RBrace) {
                    loop {
                        let k = self.expr()?;
                        self.expect(&Tok::Colon, "':'")?;
                        let v = self.expr()?;
                        pairs.push((k, v));
                        if self.at(&Tok::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RBrace, "'}'")?;
                Ok(Expr::MapLit(pairs))
            }
            Tok::Ident(name) => {
                if matches!(&self.peek2().tok, Tok::LParen) {
                    self.next(); // ident
                    self.next(); // '('
                    let mut args = Vec::new();
                    if !self.at(&Tok::RParen) {
                        loop {
                            args.push(self.expr()?);
                            if self.at(&Tok::Comma) {
                                self.next();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&Tok::RParen, "')'")?;
                    Ok(Expr::Call(name, args, tk.line as u32))
                } else {
                    self.next();
                    Ok(Expr::Ident(name))
                }
            }
            Tok::DotDot => Err(self.err(
                "'..' শুধু for-লুপের ভেতরে ব্যবহার করা যায় ('..' is only valid inside a for loop)",
            )),
            _ => Err(self.err(format!(
                "অপ্রত্যাশিত token (unexpected token): {}",
                tk.describe()
            ))),
        }
    }
}
