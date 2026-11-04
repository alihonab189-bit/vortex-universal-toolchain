//! VORTEX Debugger — VIR-level interactive debugging
//!
//! `vortexc debug file.vx` — নিজস্ব debugger, কোনো external tool নেই।
//!
//! কমান্ড:
//!   s / step      এক instruction এগোও
//!   n <k>         k টি instruction এগোও (default 1)
//!   c / continue  পরের breakpoint বা শেষ পর্যন্ত চালাও
//!   b <fn> <ip>   breakpoint বসাও (যেমন: b main 5)
//!   bl            সব breakpoint দেখাও
//!   db <fn> <ip>  breakpoint সরাও
//!   st            value stack দেখাও
//!   lc            বর্তমান frame-এর locals দেখাও
//!   fr            call frames দেখাও
//!   ir            বর্তমান instruction দেখাও
//!   q / quit      বেরোও

use crate::vir::{write_instr_text, VProgram};
use crate::vm::{Machine, Step};
use std::collections::BTreeSet;
use std::io::{BufRead, Write};

pub fn debug_program(p: &VProgram) -> i32 {
    println!(
        "VORTEX Debugger — '{}' ({} টি ফাংশন)",
        p.funcs[p.entry].name,
        p.funcs.len()
    );
    println!("'h' দিয়ে সাহায্য। breakpoint: b <fn> <ip>");
    let mut m = Machine::new(p);
    let mut bps: BTreeSet<(usize, usize)> = BTreeSet::new();
    let stdin = std::io::stdin();

    // প্রথম instruction দেখাই
    show_current(&m, p);

    loop {
        print!("(vdbg) ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            break;
        }
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            show_current(&m, p);
            continue;
        }
        match parts[0] {
            "q" | "quit" | "exit" => break,
            "h" | "help" => print_help(),
            "s" | "step" | "" => match m.step() {
                Ok(Step::Running) => show_current(&m, p),
                Ok(Step::Finished(v)) => {
                    finish(&m, v);
                    return 0;
                }
                Err(e) => {
                    eprintln!("[runtime] {}", e.msg);
                    return 1;
                }
            },
            "n" | "next" => {
                let k: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                for _ in 0..k {
                    match m.step() {
                        Ok(Step::Running) => {}
                        Ok(Step::Finished(v)) => {
                            finish(&m, v);
                            return 0;
                        }
                        Err(e) => {
                            eprintln!("[runtime] {}", e.msg);
                            return 1;
                        }
                    }
                }
                show_current(&m, p);
            }
            "c" | "continue" => loop {
                match m.step() {
                    Ok(Step::Running) => {
                        let key = (m.cur_func(), m.cur_ip());
                        if bps.contains(&key) {
                            println!("⏸ breakpoint ধরা পড়েছে: {} ip {}", m.func_name(key.0), key.1);
                            show_current(&m, p);
                            break;
                        }
                    }
                    Ok(Step::Finished(v)) => {
                        finish(&m, v);
                        return 0;
                    }
                    Err(e) => {
                        eprintln!("[runtime] {}", e.msg);
                        return 1;
                    }
                }
            },
            "b" => {
                if parts.len() < 3 {
                    eprintln!("ব্যবহার: b <fn> <ip>");
                    continue;
                }
                let fidx = match p.funcs.iter().position(|f| f.name == parts[1]) {
                    Some(i) => i,
                    None => {
                        eprintln!("ফাংশন নেই (no such function): {}", parts[1]);
                        continue;
                    }
                };
                let ip: usize = match parts[2].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        eprintln!("ip সংখ্যা হতে হবে: {}", parts[2]);
                        continue;
                    }
                };
                if fidx >= p.funcs.len() || ip >= p.funcs[fidx].code.len() {
                    eprintln!("ip সীমার বাইরে (out of range)");
                    continue;
                }
                bps.insert((fidx, ip));
                println!("✓ breakpoint: {} ip {}", p.funcs[fidx].name, ip);
            }
            "bl" => {
                if bps.is_empty() {
                    println!("(কোনো breakpoint নেই)");
                }
                for (f, ip) in &bps {
                    println!("  {} ip {}", p.funcs[*f].name, ip);
                }
            }
            "db" => {
                if parts.len() >= 3 {
                    if let (Some(fidx), Ok(ip)) = (
                        p.funcs.iter().position(|f| f.name == parts[1]),
                        parts[2].parse::<usize>(),
                    ) {
                        bps.remove(&(fidx, ip));
                        println!("✓ সরানো হয়েছে");
                    }
                }
            }
            "st" | "stack" => {
                if m.stack.is_empty() {
                    println!("(খালি stack)");
                }
                for (i, v) in m.stack.iter().enumerate() {
                    println!("  {:>3}: {}", i, v.repr());
                }
            }
            "lc" | "locals" => match m.frames.last() {
                None => println!("(frame নেই)"),
                Some(fr) => {
                    let fname = &p.funcs[fr.fidx].name;
                    println!("  {} ({})", fname, fr.slots.len());
                    for (i, v) in fr.slots.iter().enumerate() {
                        println!("  slot {}: {}", i, v.repr());
                    }
                }
            },
            "fr" | "frames" => {
                for (i, fr) in m.frames.iter().enumerate() {
                    println!(
                        "  #{} {} ip {} slots {}",
                        i,
                        p.funcs[fr.fidx].name,
                        fr.ip,
                        fr.slots.len()
                    );
                }
            }
            "ir" => show_current(&m, p),
            "out" => {
                for l in &m.out {
                    println!("| {}", l);
                }
            }
            other => {
                eprintln!("অজানা কমান্ড: {} — 'h' দেখুন", other);
            }
        }
    }
    0
}

fn show_current(m: &Machine, p: &VProgram) {
    let f = m.cur_func();
    let ip = m.cur_ip();
    let fname = m.func_name(f);
    match p.funcs[f].code.get(ip) {
        Some(i) => println!(
            "▶ {}:{}  {}",
            fname,
            ip,
            write_instr_text(i)
        ),
        None => println!("▶ {}:{}  (শেষ)", fname, ip),
    }
}

fn finish(m: &Machine, v: crate::value::Value) {
    println!("✔ প্রোগ্রাম শেষ (finished)");
    for l in &m.out {
        println!("{}", l);
    }
    if v != crate::value::Value::Unit {
        println!("= {}", v);
    }
}

fn print_help() {
    println!("s/step | n <k> | c/continue | b <fn> <ip> | bl | db <fn> <ip>");
    println!("st/stack | lc/locals | fr/frames | ir | out | h | q");
}
