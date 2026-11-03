//! vortexc — VORTEX Universal Toolchain CLI
//!
//! Commands:
//!   run <file.vx>        চালাও
//!   ir <file.vx>         VIR bytecode দেখাও
//!   ast <file.vx>        AST দেখাও
//!   tokens <file.vx>     token list দেখাও
//!   build <file.vx>      .vxc (VIR artifact) বানাও
//!   exec <file.vxc>      .vxc artifact চালাও
//!   repl                 interactive REPL
//!   keywords             keyword ও built-in টেবিল
//!   version | help

use std::fs;
use std::io::{self, BufRead, Write};

use vortexc::ast::{self, Expr, Stmt};
use vortexc::keywords::{BUILTIN_TABLE, KEYWORD_TABLE, Builtin};
use vortexc::token::Tok;
use vortexc::value::Value;
use vortexc::vir;
use vortexc::{compiler, lexer, parser, vm, VERSION};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code);
}

fn run(args: &[String]) -> i32 {
    if args.is_empty() {
        usage();
        return 0;
    }
    match args[0].as_str() {
        "run" => with_file(args, |src| match vortexc::run_source(src) {
            Ok((_, out, _)) => {
                print_output(&out);
                0
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        }),
        "ir" => with_file(args, |src| match parse_compile(src) {
            Ok(vp) => {
                println!("{}", vir::write_text(&vp));
                0
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        }),
        "ast" => with_file(args, |src| match parser::parse(src) {
            Ok(p) => {
                print!("{}", ast::dump(&p));
                0
            }
            Err(e) => {
                eprintln!("parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}", e.line, e.col, e.msg);
                1
            }
        }),
        "tokens" => with_file(args, |src| match lexer::lex(src) {
            Ok(ts) => {
                for t in &ts {
                    println!("{:>4}:{:<4}  {}", t.line, t.col, tok_label(&t.tok));
                }
                0
            }
            Err(e) => {
                eprintln!("lexer ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}", e.line, e.col, e.msg);
                1
            }
        }),
        "build" => {
            if args.len() < 2 {
                eprintln!("ব্যবহার (usage): vortexc build <file.vx> [-o output.vxc]");
                return 2;
            }
            let path = &args[1];
            let out = if args.len() >= 4 && args[2] == "-o" {
                args[3].clone()
            } else {
                std::path::Path::new(path)
                    .with_extension("vxc")
                    .to_string_lossy()
                    .to_string()
            };
            match fs::read_to_string(path) {
                Ok(src) => match parse_compile(&src) {
                    Ok(vp) => {
                        let text = vir::write_text(&vp);
                        match fs::write(&out, text) {
                            Ok(_) => {
                                println!("VIR artifact তৈরি হয়েছে (built): {}", out);
                                0
                            }
                            Err(e) => {
                                eprintln!("লেখা যায়নি (cannot write) {}: {}", out, e);
                                1
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                },
                Err(e) => {
                    eprintln!("ফাইল পড়া যায়নি (cannot read) {}: {}", path, e);
                    1
                }
            }
        }
        "exec" => with_file(args, |src| match vir::parse_text(src) {
            Ok(vp) => match vm::execute(&vp) {
                Ok(r) => {
                    print_output(&r.output);
                    0
                }
                Err(e) => {
                    eprintln!("[runtime] ত্রুটি (error): {}", e.msg);
                    1
                }
            },
            Err(e) => {
                eprintln!("[vir] ত্রুটি (error): {}", e);
                1
            }
        }),
        "repl" => {
            repl();
            0
        }
        "keywords" => {
            print_keywords();
            0
        }
        "version" | "--version" | "-V" => {
            println!("vortexc {} — VORTEX Universal Toolchain", VERSION);
            0
        }
        "help" | "--help" | "-h" => {
            usage();
            0
        }
        other => {
            eprintln!("অজানা কমান্ড (unknown command): {}", other);
            usage();
            2
        }
    }
}

fn with_file(args: &[String], f: impl FnOnce(&str) -> i32) -> i32 {
    if args.len() < 2 {
        eprintln!("ব্যবহার (usage): vortexc {} <file>", args[0]);
        return 2;
    }
    match fs::read_to_string(&args[1]) {
        Ok(src) => f(&src),
        Err(e) => {
            eprintln!("ফাইল পড়া যায়নি (cannot read) {}: {}", args[1], e);
            1
        }
    }
}

fn parse_compile(src: &str) -> Result<vortexc::vir::VProgram, String> {
    let program = parser::parse(src).map_err(|e| {
        format!(
            "parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
            e.line, e.col, e.msg
        )
    })?;
    compiler::compile(&program).map_err(|e| format!("compiler ত্রুটি (error): {}", e.msg))
}

fn print_output(out: &[String]) {
    for line in out {
        println!("{}", line);
    }
}

fn tok_label(t: &Tok) -> String {
    match t {
        Tok::Int(v) => format!("INT      {}", v),
        Tok::Float(v) => format!("FLOAT    {:?}", v),
        Tok::Str(v) => format!("STR      {:?}", v),
        Tok::Ident(s) => format!("IDENT    {}", s),
        other => format!("{:?}", other),
    }
}

fn print_keywords() {
    println!("VORTEX {} — Keyword টেবিল (English • অসমীয়া • বাংলা)", VERSION);
    println!("=========================================================");
    for (name, aliases) in KEYWORD_TABLE {
        println!("  {:<8} →  {}", name, aliases.join("  |  "));
    }
    println!();
    println!("Built-in ফাংশন (built-in functions):");
    for (name, aliases, doc) in BUILTIN_TABLE {
        println!("  {:<8} →  {:<28}  {}", name, aliases.join("  |  "), doc);
    }
}

fn usage() {
    println!("VORTEX Universal Toolchain — vortexc {}", VERSION);
    println!();
    println!("ব্যবহার (usage): vortexc <কমান্ড (command)> [arguments]");
    println!();
    println!("কমান্ড (commands):");
    println!("  run <file.vx>        প্রোগ্রাম চালাও (run a program)");
    println!("  ir <file.vx>         VIR bytecode দেখাও (show compiled IR)");
    println!("  ast <file.vx>        AST দেখাও (show syntax tree)");
    println!("  tokens <file.vx>     token list দেখাও (show tokens)");
    println!("  build <file.vx> [-o out.vxc]   VIR artifact বানাও (compile to file)");
    println!("  exec <file.vxc>      artifact চালাও (run compiled artifact)");
    println!("  repl                 interactive REPL শুরু করো");
    println!("  keywords             সব keyword ও built-in দেখাও");
    println!("  version | help");
}

// ---------------- REPL ----------------

fn unbalanced(lines: &[String]) -> bool {
    let mut net: i64 = 0;
    let mut in_str = false;
    let mut esc = false;
    for ch in lines.join("\n").chars() {
        if in_str {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' | '(' => net += 1,
            '}' | ')' => net -= 1,
            '/' => {}
            _ => {}
        }
    }
    net > 0
}

fn parse_single(src: &str) -> Option<Stmt> {
    let p = parser::parse(src).ok()?;
    if p.stmts.len() == 1 {
        p.stmts.into_iter().next()
    } else {
        None
    }
}

fn stmt_name(st: &Stmt) -> Option<&str> {
    match st {
        Stmt::Let(n, _) | Stmt::FnDecl { name: n, .. } => Some(n),
        _ => None,
    }
}

fn is_show_call(e: &Expr) -> bool {
    matches!(e, Expr::Call(n, _, _) if vortexc::keywords::lookup_builtin(n) == Some(Builtin::Show))
}

/// পুরনো buffer-এ একই নামের let/fn আছে কি না খুঁজে বাদ দেয়।
fn replace_in_buf(buf: &mut Vec<String>, name: &str) {
    buf.retain(|entry| {
        let keep = match parse_single(entry) {
            Some(st) => stmt_name(&st) != Some(name),
            None => true,
        };
        keep
    });
}

fn repl() {
    println!("VORTEX {} REPL — নিজের ভাষায় কোড লেখো ✨", VERSION);
    println!("(:exit দিয়ে বেরোও | :reset | :help)");
    let stdin = io::stdin();
    let mut buf: Vec<String> = Vec::new(); // persistent let/fn source lines
    let mut pending: Vec<String> = Vec::new(); // multi-line accumulation

    loop {
        let prompt = if pending.is_empty() { "VORTEX> " } else { "......> " };
        print!("{}", prompt);
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            break;
        }
        let t = line.trim_end().to_string();
        match t.trim() {
            ":exit" | ":quit" | ":q" => break,
            ":reset" => {
                buf.clear();
                pending.clear();
                println!("REPL রিসেট হয়েছে (reset)");
                continue;
            }
            ":help" => {
                println!("কমান্ড: :exit :reset :help");
                println!("বৈশিষ্ট্য: let/fn লাইন persist হয়; এক্সপ্রেশন লিখলে মান প্রিন্ট হয়");
                continue;
            }
            _ => {}
        }
        if t.trim().is_empty() && pending.is_empty() {
            continue;
        }
        pending.push(t);

        let mut input = pending.join("\n");
        let full_src0 = buf.join("\n") + "\n" + &input;

        // completeness check — ';' ছাড়া এক্সপ্রেশন হলে শেষে ';' জুড়ে আবার চেষ্টা
        let mut parsed = parser::parse(&full_src0);
        if parsed.is_err() && !unbalanced(&pending) && !input.ends_with(';') {
            let input2 = input.clone() + ";";
            let full_src2 = buf.join("\n") + "\n" + &input2;
            if let Ok(p) = parser::parse(&full_src2) {
                input = input2;
                parsed = Ok(p);
            }
        }

        match parsed {
            Err(e) => {
                if unbalanced(&pending) {
                    continue; // আরও লাইন দাও
                }
                eprintln!(
                    "ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
                    e.line, e.col, e.msg
                );
                pending.clear();
            }
            Ok(_) => {
                pending.clear();
                let full_src = buf.join("\n") + "\n" + &input;
                let single = parse_single(&input);
                match single {
                    Some(Stmt::Let(name, _)) | Some(Stmt::FnDecl { name, .. }) => {
                        let mut nb = buf.clone();
                        replace_in_buf(&mut nb, &name);
                        nb.push(input.clone());
                        let src2 = nb.join("\n");
                        let built: Option<vortexc::vir::VProgram> = match parser::parse(&src2) {
                            Ok(p) => match compiler::compile(&p) {
                                Ok(vp) => Some(vp),
                                Err(e) => {
                                    eprintln!("compiler ত্রুটি (error): {}", e.msg);
                                    None
                                }
                            },
                            Err(e) => {
                                eprintln!(
                                    "parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
                                    e.line, e.col, e.msg
                                );
                                None
                            }
                        };
                        if let Some(vp) = built {
                            match vm::execute(&vp) {
                                Ok(_) => {
                                    buf = nb;
                                    println!("✓ {}", name);
                                }
                                Err(e) => eprintln!("[runtime] ত্রুটি (error): {}", e.msg),
                            }
                        }
                    }
                    Some(Stmt::ExprStmt(e)) => {
                        if is_show_call(&e) {
                            run_prog(&full_src, false);
                        } else {
                            // মান দেখানোর জন্য Return যোগ করে চালাও
                            let mut p2 = parser::parse(&full_src).expect("checked");
                            p2.stmts.push(Stmt::Return(Some(e)));
                            match compiler::compile(&p2) {
                                Ok(vp) => match vm::execute(&vp) {
                                    Ok(r) => {
                                        print_output(&r.output);
                                        if r.value != Value::Unit {
                                            println!("= {}", r.value);
                                        }
                                    }
                                    Err(e) => eprintln!("[runtime] ত্রুটি (error): {}", e.msg),
                                },
                                Err(e) => eprintln!("compiler ত্রুটি (error): {}", e.msg),
                            }
                        }
                    }
                    _ => {
                        run_prog(&full_src, false);
                    }
                }
            }
        }
    }
}

fn run_prog(src: &str, print_value: bool) {
    let program = match parser::parse(src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
                e.line, e.col, e.msg
            );
            return;
        }
    };
    let vp = match compiler::compile(&program) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("compiler ত্রুটি (error): {}", e.msg);
            return;
        }
    };
    match vm::execute(&vp) {
        Ok(r) => {
            print_output(&r.output);
            if print_value && r.value != Value::Unit {
                println!("= {}", r.value);
            }
        }
        Err(e) => eprintln!("[runtime] ত্রুটি (error): {}", e.msg),
    }
}
