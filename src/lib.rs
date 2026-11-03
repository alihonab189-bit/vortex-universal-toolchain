//! VORTEX Universal Toolchain — Language Engine v0.1
//!
//! Source (.vx) → Lexer → Parser → AST → Compiler → VIR → VM
//!
//! সম্পূর্ণ নিজস্ব ইমপ্লিমেন্টেশন — কোনো third-party crate নেই (100% Rust std)।

pub mod ast;
pub mod compiler;
pub mod keywords;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod value;
pub mod vir;
pub mod vm;

pub const VERSION: &str = "0.1.0";

use value::Value;

/// কোন ধাপে ত্রুটি হয়েছে (which stage failed)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Lex,
    Parse,
    Compile,
    Vm,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Stage::Lex => "lexer",
            Stage::Parse => "parser",
            Stage::Compile => "compiler",
            Stage::Vm => "runtime",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug)]
pub struct RunError {
    pub stage: Stage,
    pub msg: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(
                f,
                "[{}] ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
                self.stage, self.line, self.col, self.msg
            )
        } else {
            write!(f, "[{}] ত্রুটি (error): {}", self.stage, self.msg)
        }
    }
}

/// এক কলে পুরো pipeline চালায়: Source → Lexer → Parser → Compiler → VM
/// রিটার্ন: (main-এর রিটার্ন মান, show() আউটপুট, VIR টেক্সট)
pub fn run_source(src: &str) -> Result<(Value, Vec<String>, String), RunError> {
    let program = parser::parse(src).map_err(|e| RunError {
        stage: Stage::Parse,
        msg: e.msg,
        line: e.line,
        col: e.col,
    })?;
    let vprog = compiler::compile(&program).map_err(|e| RunError {
        stage: Stage::Compile,
        msg: e.msg,
        line: 0,
        col: 0,
    })?;
    let vir_text = vir::write_text(&vprog);
    let result = vm::execute(&vprog).map_err(|e| RunError {
        stage: Stage::Vm,
        msg: e.msg,
        line: 0,
        col: 0,
    })?;
    Ok((result.value, result.output, vir_text))
}
