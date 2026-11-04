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
//!   wasm <file.vx>       নিজস্ব backend → .wasm বাইনারি (--wat দেখাও)
//!   ai "<বাক্য>"         প্রাকৃতিক ভাষা → VORTEX কোড (--run সহ)
//!   explain <file.vx>    কোডের ব্যাখ্যা
//!   pkg <cmd>            নিজস্ব package manager (init/build/pack/install/list/tree)
//!   debug <file.vx>      VIR-level interactive debugger
//!   fmt <file.vx>        কোড ফরম্যাট (-w: জায়গাতেই লেখো)
//!   lint <file.vx>       সমস্যা খুঁজে বের করো
//!   test <file.vx>       test_* ফাংশন চালাও
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
        "wasm" => cmd_wasm(&args),
        "ai" => cmd_ai(&args),
        "explain" => with_file(args, |src| match parser::parse(src) {
            Ok(p) => {
                print!("{}", vortexc::ai::explain(src, &p));
                0
            }
            Err(e) => {
                eprintln!(
                    "parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
                    e.line, e.col, e.msg
                );
                1
            }
        }),
        "pkg" => cmd_pkg(&args[1..]),
        "debug" => with_file(args, |src| match parse_compile(src) {
            Ok(vp) => vortexc::debug::debug_program(&vp),
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        }),
        "fmt" => cmd_fmt(&args),
        "lint" => with_file(args, |src| match parser::parse(src) {
            Ok(p) => {
                let ws = vortexc::lint::lint(src, &p);
                if ws.is_empty() {
                    println!("✓ সমস্যা নেই (no issues)");
                }
                for w in &ws {
                    println!("⚠ {}", w);
                }
                if ws.is_empty() { 0 } else { 1 }
            }
            Err(e) => {
                eprintln!(
                    "parser ত্রুটি (error) [লাইন (line) {}, কলাম (col) {}]: {}",
                    e.line, e.col, e.msg
                );
                1
            }
        }),
        "test" => cmd_test(&args),
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
    println!("ভাষা (language):");
    println!("  run <file.vx>        প্রোগ্রাম চালাও (run a program)");
    println!("  ir <file.vx>         VIR bytecode দেখাও (show compiled IR)");
    println!("  ast <file.vx>        AST দেখাও (show syntax tree)");
    println!("  tokens <file.vx>     token list দেখাও (show tokens)");
    println!("  build <file.vx> [-o out.vxc]   VIR artifact বানাও (compile to file)");
    println!("  exec <file.vxc>      artifact চালাও (run compiled artifact)");
    println!("  repl                 interactive REPL শুরু করো");
    println!("  keywords             সব keyword ও built-in দেখাও");
    println!();
    println!("Backend ও টুল (backends & tools):");
    println!("  wasm <file.vx> [-o out.wasm]   নিজস্ব WebAssembly backend");
    println!("  ai \"<বাক্য>\" [--run]           প্রাকৃতিক ভাষা → VORTEX কোড");
    println!("  explain <file.vx>    কোডের ব্যাখ্যা (explain code)");
    println!("  pkg init|build|pack|install|list|tree   নিজস্ব package manager");
    println!("  debug <file.vx>      VIR-level interactive debugger");
    println!("  fmt <file.vx> [-w]   কোড ফরম্যাট করো (-w: লিখে দাও)");
    println!("  lint <file.vx>       সমস্যা খুঁজো (find issues)");
    println!("  test <file.vx>       test_* ফাংশন চালাও (run test functions)");
    println!("  version | help");
}

// ---------------- wasm ----------------

fn cmd_wasm(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("ব্যবহার (usage): vortexc wasm <file.vx> [-o out.wasm]");
        return 2;
    }
    let path = &args[1];
    let out = if args.len() >= 4 && args[2] == "-o" {
        args[3].clone()
    } else {
        std::path::Path::new(path)
            .with_extension("wasm")
            .to_string_lossy()
            .to_string()
    };
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ফাইল পড়া যায়নি (cannot read) {}: {}", path, e);
            return 1;
        }
    };
    match vortexc::wasm::compile_to_wasm(&src) {
        Ok(bytes) => match fs::write(&out, &bytes) {
            Ok(_) => {
                println!(
                    "✓ WebAssembly তৈরি (built): {} ({} bytes) — নিজস্ব backend, LLVM ছাড়া",
                    out,
                    bytes.len()
                );
                0
            }
            Err(e) => {
                eprintln!("লেখা যায়নি (cannot write) {}: {}", out, e);
                1
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

// ---------------- ai ----------------

fn cmd_ai(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("ব্যবহার (usage): vortexc ai \"<বাক্য>\" [--run]");
        eprintln!("উদাহরণ: vortexc ai \"১ থেকে ১০ পর্যন্ত যোগফল দেখাও\" --run");
        return 2;
    }
    let sentence = &args[1];
    let do_run = args.iter().any(|a| a == "--run");
    match vortexc::ai::generate(sentence) {
        Ok((code, intent)) => {
            println!("// AI Engine — intent: {}", intent);
            println!("{}", code);
            if do_run {
                println!("---- আউটপুট (output) ----");
                match vortexc::ai::run_generated(&code) {
                    Ok(lines) => {
                        for l in lines {
                            println!("{}", l);
                        }
                    }
                    Err(e) => {
                        eprintln!("[runtime] {}", e);
                        return 1;
                    }
                }
            }
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

// ---------------- pkg ----------------

fn cmd_pkg(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("ব্যবহার: vortexc pkg <init|build|pack|install|list|tree> ...");
        return 2;
    }
    let deps_dir = std::path::Path::new(".vortex/deps");
    match args[0].as_str() {
        "init" => {
            let name = args.get(1).cloned().unwrap_or_else(|| {
                std::env::current_dir()
                    .ok()
                    .and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_else(|| "mypkg".into())
            });
            match vortexc::pkg::cmd_init(std::path::Path::new("."), &name) {
                Ok(p) => {
                    println!("✓ package '{}' তৈরি (created): {}", name, p);
                    0
                }
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            }
        }
        "build" => match vortexc::pkg::cmd_build(std::path::Path::new(".")) {
            Ok(p) => {
                println!("✓ build: {}", p);
                0
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        "pack" => match vortexc::pkg::cmd_pack(std::path::Path::new(".")) {
            Ok(p) => {
                println!("✓ archive: {}", p);
                0
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        "install" => {
            if args.len() < 2 {
                eprintln!("ব্যবহার: vortexc pkg install <file.vpa>");
                return 2;
            }
            match vortexc::pkg::install(std::path::Path::new(&args[1]), deps_dir) {
                Ok(name) => {
                    println!(
                        "✓ ইনস্টল হয়েছে (installed): {} → {}",
                        name,
                        deps_dir.display()
                    );
                    0
                }
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            }
        }
        "list" => {
            let items = vortexc::pkg::cmd_list(deps_dir);
            if items.is_empty() {
                println!("(কোনো package নেই — no packages)");
            }
            for it in items {
                println!("  {}", it);
            }
            0
        }
        "tree" => match vortexc::pkg::build_graph(deps_dir) {
            Ok(g) => match vortexc::pkg::resolve_order(&g) {
                Ok(order) => {
                    if order.is_empty() {
                        println!("(কোনো dependency নেই — no dependencies)");
                    }
                    for (i, n) in order.iter().enumerate() {
                        println!("  {}. {}", i + 1, n);
                    }
                    0
                }
                Err(e) => {
                    eprintln!("✗ {}", e);
                    1
                }
            },
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        other => {
            eprintln!("অজানা pkg কমান্ড (unknown pkg command): {}", other);
            2
        }
    }
}

// ---------------- fmt ----------------

fn cmd_fmt(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("ব্যবহার: vortexc fmt <file.vx> [-w]");
        return 2;
    }
    let path = &args[1];
    let write_in_place = args.iter().any(|a| a == "-w");
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("পড়া যায়নি (cannot read) {}: {}", path, e);
            return 1;
        }
    };
    match vortexc::fmt::format_source(&src) {
        Ok(formatted) => {
            if write_in_place {
                if formatted != src {
                    let _ = fs::write(path, &formatted);
                    println!("✓ ফরম্যাট হয়েছে (formatted): {}", path);
                } else {
                    println!("✓ ইতিমধ্যে পরিষ্কার (already formatted)");
                }
            } else {
                print!("{}", formatted);
            }
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

// ---------------- test runner ----------------

fn cmd_test(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("ব্যবহার: vortexc test <file.vx>");
        return 2;
    }
    let src = match fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("পড়া যায়নি (cannot read) {}: {}", args[1], e);
            return 1;
        }
    };
    let program = match parser::parse(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parser ত্রুটি [লাইন {}, কলাম {}]: {}", e.line, e.col, e.msg);
            return 1;
        }
    };
    let vp = match compiler::compile(&program) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("compiler ত্রুটি (error): {}", e.msg);
            return 1;
        }
    };
    let mut tests: Vec<String> = Vec::new();
    for st in &program.stmts {
        if let Stmt::FnDecl { name, params, .. } = st {
            if name.starts_with("test_") && params.is_empty() {
                tests.push(name.clone());
            }
        }
    }
    if tests.is_empty() {
        println!("(কোনো test_* ফাংশন নেই — no test functions found)");
        return 0;
    }
    let mut passed = 0usize;
    let mut failed = 0usize;
    for t in &tests {
        match vm::execute_fn(&vp, t) {
            Ok(_) => {
                println!("✓ {}", t);
                passed += 1;
            }
            Err(e) => {
                println!("✗ {} — {}", t, e.msg);
                failed += 1;
            }
        }
    }
    println!();
    println!(
        "ফলাফল (results): {} পাস (passed), {} ব্যর্থ (failed) / {} মোট (total)",
        passed,
        failed,
        tests.len()
    );
    if failed == 0 { 0 } else { 1 }
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
