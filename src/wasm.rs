//! নিজস্ব WASM backend — AST → WebAssembly binary (LLVM ছাড়া, 100% হাতে লেখা)
//!
//! ডিজাইন (v0.2):
//! - প্রতিটি VORTEX মান মেমরিতে 16-বাইট box: {tag: i32, pad: i32, bits: i64}
//! - wasm স্ট্যাকে শুধু box-এর pointer (i32) থাকে — টাইপ-সেফ
//! - Tag: 0=int 1=float 2=string 3=bool 4=array 5=map 6=unit
//! - String: bits = ptr → {len: i32, bytes...} (bytes ptr+8 থেকে)
//! - Array:  bits = ptr → {len: i32, cap: i32, cells: i32}
//! - Map:    bits = ptr → {len: i32, cap: i32, entries: i32} (entry = 32B)
//! - নিজস্ব bump allocator + নিজস্ব output buffer
//! - Export: memory, vortex_main, vortex_output
//!
//! সীমাবদ্ধতা (v0.2, স্পষ্টভাবে ডকুমেন্টেড):
//! - .upper()/.lower()/.trim()/.split()/.replace()/.join()/.repeat()/.sub()
//!   wasm backend-এ এখনো নেই (VM-এ আছে) — compile error দেয়
//! - wasm-এ int অ্যারিথমেটিক wrap করে (VM-এ overflow trap)
//! - num() শুধু integer string পার্স করে

use crate::ast::{Expr, ForIter, Program, Stmt};

// ================= binary writer =================

fn uleb(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

fn sleb(out: &mut Vec<u8>, mut v: i64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        let sign = b & 0x40 != 0;
        if (v == 0 && !sign) || (v == -1 && sign) {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

fn section(out: &mut Vec<u8>, id: u8, payload: &[u8]) {
    out.push(id);
    uleb(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

fn vec_bytes(items: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    uleb(&mut v, items.len() as u64);
    for it in items {
        uleb(&mut v, it.len() as u64);
        v.extend_from_slice(it);
    }
    v
}

// ================= asm builder =================

#[derive(Clone, Copy, PartialEq)]
enum VT {
    I32,
    I64,
    F64,
}

impl VT {
    fn byte(self) -> u8 {
        match self {
            VT::I32 => 0x7F,
            VT::I64 => 0x7E,
            VT::F64 => 0x7C,
        }
    }
}

struct Asm(Vec<u8>);

impl Asm {
    fn new() -> Self {
        Asm(Vec::new())
    }
    fn o(&mut self, op: u8) {
        self.0.push(op);
    }
    fn raw(&mut self, bs: &[u8]) {
        self.0.extend_from_slice(bs);
    }
    fn idx(&mut self, op: u8, x: u32) {
        self.o(op);
        uleb(&mut self.0, x as u64);
    }
    fn i32c(&mut self, v: i32) {
        self.o(0x41);
        sleb(&mut self.0, v as i64);
    }
    fn i64c(&mut self, v: i64) {
        self.o(0x42);
        sleb(&mut self.0, v);
    }
    fn f64c(&mut self, v: f64) {
        self.o(0x44);
        self.raw(&v.to_le_bytes());
    }
    fn call(&mut self, f: u32) {
        self.idx(0x10, f);
    }
    fn lget(&mut self, l: u32) {
        self.idx(0x20, l);
    }
    fn lset(&mut self, l: u32) {
        self.idx(0x21, l);
    }
    fn ltee(&mut self, l: u32) {
        self.idx(0x22, l);
    }
    fn gget(&mut self, g: u32) {
        self.idx(0x23, g);
    }
    fn gset(&mut self, g: u32) {
        self.idx(0x24, g);
    }
    fn br(&mut self, d: u32) {
        self.idx(0x0C, d);
    }
    fn br_if(&mut self, d: u32) {
        self.idx(0x0D, d);
    }
    fn load(&mut self, op: u8, align: u32, offset: u32) {
        self.o(op);
        uleb(&mut self.0, align as u64);
        uleb(&mut self.0, offset as u64);
    }
    fn blk(&mut self, op: u8, bt: u8) {
        self.o(op);
        self.raw(&[bt]);
    }
    fn end(&mut self) {
        self.o(0x0B);
    }
    /// if (cond on stack) { body } — void
    fn if_void(&mut self, body: impl FnOnce(&mut Asm)) {
        self.blk(0x04, 0x40);
        body(self);
        self.end();
    }
}

// ================= module =================

struct FuncDef {
    type_idx: u32,
    locals: Vec<VT>,
    body: Vec<u8>,
}

struct Mod {
    types: Vec<(Vec<VT>, Vec<VT>)>,
    funcs: Vec<FuncDef>,
    data: Vec<u8>,
    str_map: std::collections::HashMap<String, u32>,
    g_alloc: u32,
    g_outp: u32,
    g_outl: u32,
    g_outc: u32,
}

impl Mod {
    fn new() -> Self {
        let mut m = Mod {
            types: Vec::new(),
            funcs: Vec::new(),
            data: Vec::new(),
            str_map: std::collections::HashMap::new(),
            g_alloc: 0,
            g_outp: 1,
            g_outl: 2,
            g_outc: 3,
        };
        for s in CONST_STRINGS {
            m.str_const(s);
        }
        m
    }

    fn type_idx(&mut self, params: &[VT], results: &[VT]) -> u32 {
        if let Some(i) = self
            .types
            .iter()
            .position(|t| t.0.as_slice() == params && t.1.as_slice() == results)
        {
            return i as u32;
        }
        self.types.push((params.to_vec(), results.to_vec()));
        (self.types.len() - 1) as u32
    }

    fn add_func(&mut self, params: &[VT], results: &[VT], locals: Vec<VT>, body: Vec<u8>) -> u32 {
        let ti = self.type_idx(params, results);
        let mut body = body;
        body.push(0x0B); // function body-র বাধ্যতামূলক শেষ opcode
        self.funcs.push(FuncDef {
            type_idx: ti,
            locals,
            body,
        });
        (self.funcs.len() - 1) as u32
    }

    fn str_const(&mut self, s: &str) -> u32 {
        if let Some(&a) = self.str_map.get(s) {
            return a;
        }
        let addr = self.data.len() as u32;
        let bytes = s.as_bytes();
        // header runtime string-এর মতোই 8 বাইট: len(4) + pad(4), bytes addr+8 থেকে
        self.data
            .extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.data.extend_from_slice(&[0, 0, 0, 0]);
        self.data.extend_from_slice(bytes);
        while self.data.len() % 8 != 0 {
            self.data.push(0);
        }
        self.str_map.insert(s.to_string(), addr);
        addr
    }
}

const CONST_STRINGS: &[&str] = &[
    "NaN", "inf", "-inf", "true", "false", "unit", "int", "float", "string", "bool", "array",
    "map", "[", "]", "{", "}", ", ", ": ", "\n", "assert ব্যর্থ (assertion failed)",
    "assert_eq ব্যর্থ (equality assertion failed)",
];

// helper function indices
const F_ALLOC: u32 = 0;
const F_MEMCPY: u32 = 1;
const F_BOX: u32 = 2;
const F_BOXI: u32 = 3;
const F_BOXF: u32 = 4;
const F_BOXB: u32 = 5;
const F_BOXU: u32 = 6;
const F_SBOX: u32 = 7;
const F_TAG: u32 = 8;
const F_UBITS: u32 = 9;
const F_SNEW: u32 = 10;
const F_SLEN: u32 = 11;
const F_SCAT: u32 = 12;
const F_SEQ: u32 = 13;
const F_SCMP: u32 = 14;
const F_I2S: u32 = 15;
const F_F2S: u32 = 16;
const F_B2S: u32 = 17;
const F_TOSTR: u32 = 18;
const F_AREPR: u32 = 19;
const F_MREPR: u32 = 20;
const F_TOF: u32 = 21;
const F_ADD: u32 = 22;
const F_SUB: u32 = 23;
const F_MUL: u32 = 24;
const F_DIV: u32 = 25;
const F_MOD: u32 = 26;
const F_NEG: u32 = 27;
const F_NOT: u32 = 28;
const F_TRUTHY: u32 = 29;
const F_LT: u32 = 30;
const F_GT: u32 = 31;
const F_LE: u32 = 32;
const F_GE: u32 = 33;
const F_VEQ: u32 = 34;
const F_ARRNEW: u32 = 35;
const F_APUSH: u32 = 36;
const F_ALEN_RAW: u32 = 37;
const F_AGET: u32 = 38;
const F_AGET_CELL: u32 = 39;
const F_ASET: u32 = 40;
const F_APOP: u32 = 41;
const F_MAPNEW: u32 = 42;
const F_MHDR_LEN: u32 = 43;
const F_MHDR_BASE: u32 = 44;
const F_MSET: u32 = 45;
const F_MGET: u32 = 46;
const F_MKEYS: u32 = 47;
const F_MVALS: u32 = 48;
const F_MHAS: u32 = 49;
const F_OUT: u32 = 50;
const F_SCONTAINS: u32 = 51;
const F_SCHARS: u32 = 52;
const F_S2I: u32 = 53;
const F_POW: u32 = 54;
const F_BLEN: u32 = 55;
const F_BTYPE: u32 = 56;
const F_BFLOOR: u32 = 57;
const F_BCEIL: u32 = 58;
const F_BABS: u32 = 59;
const F_BSQRT: u32 = 60;
const F_BMIN: u32 = 61;
const F_BMAX: u32 = 62;
const F_BCONTAINS: u32 = 63;
const F_BITER: u32 = 64;
const F_BINDEX: u32 = 65;
const F_BINDEX_SET: u32 = 66;
const F_BASSERT: u32 = 67;
const F_BASSERT_EQ: u32 = 68;
const F_FAIL: u32 = 69;
const N_HELPERS: u32 = 70;

// ================= helpers =================

fn build_helpers(m: &mut Mod) {
    let sc = |mm: &mut Mod, s: &str| mm.str_const(s);

    // ---- $alloc(n: i32) -> i32 ----
    {
        let mut a = Asm::new();
        a.gget(m.g_alloc);
        a.lset(1);
        a.gget(m.g_alloc);
        a.lget(0);
        a.raw(&[0x6A]);
        a.i32c(7);
        a.raw(&[0x6A]);
        a.i32c(-8);
        a.raw(&[0x71]);
        a.gset(m.g_alloc);
        a.o(0x3F);
        a.raw(&[0x00]);
        a.i32c(65536);
        a.raw(&[0x6C]);
        a.gget(m.g_alloc);
        a.raw(&[0x48]);
        a.if_void(|a| {
            a.gget(m.g_alloc);
            a.o(0x3F);
            a.raw(&[0x00]);
            a.i32c(65536);
            a.raw(&[0x6C]);
            a.raw(&[0x6B]);
            a.i32c(65535);
            a.raw(&[0x6A]);
            a.i32c(16);
            a.raw(&[0x76]);
            a.o(0x40);
            a.raw(&[0x00]);
            a.o(0x1A);
        });
        a.lget(1);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $memcpy(dst, src, n) ----
    {
        let mut a = Asm::new();
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(2);
        a.raw(&[0x45]);
        a.br_if(1);
        a.lget(0);
        a.lget(1);
        a.load(0x2D, 0, 0);
        a.load(0x3B, 0, 0);
        a.lget(0);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(0);
        a.lget(1);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(1);
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.lset(2);
        a.br(0);
        a.end();
        a.end();
        m.add_func(&[VT::I32, VT::I32, VT::I32], &[], vec![], a.0);
    }

    // ---- $box(tag, bits) -> ptr ----
    {
        let mut a = Asm::new();
        a.i32c(16);
        a.call(F_ALLOC);
        a.ltee(2); // p স্ট্যাকেই থাকে
        a.lget(2);
        a.lget(0);
        a.load(0x36, 2, 0);
        a.lget(2);
        a.lget(1);
        a.load(0x37, 3, 8);
        m.add_func(&[VT::I32, VT::I64], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $boxi / $boxf / $boxb / $boxu / $sbox ----
    {
        let mut a = Asm::new();
        a.i32c(0);
        a.lget(0);
        a.call(F_BOX);
        m.add_func(&[VT::I64], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.i32c(1);
        a.lget(0);
        a.raw(&[0xBD]);
        a.call(F_BOX);
        m.add_func(&[VT::F64], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.i32c(3);
        a.lget(0);
        a.call(F_BOX);
        m.add_func(&[VT::I64], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.i32c(6);
        a.i64c(0);
        a.call(F_BOX);
        m.add_func(&[], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.i32c(2);
        a.lget(0);
        a.raw(&[0xAD]);
        a.call(F_BOX);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $tag / $ubits ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.load(0x28, 2, 0);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.lget(0);
        a.load(0x29, 3, 8);
        m.add_func(&[VT::I32], &[VT::I64], vec![], a.0);
    }

    // ---- $snew(len) -> ptr ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.call(F_ALLOC);
        a.ltee(1); // p স্ট্যাকেই থাকে
        a.lget(1);
        a.lget(0);
        a.load(0x36, 2, 0);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $slen(p) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.load(0x28, 2, 0);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $scat(a, b) -> ptr ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_SLEN);
        a.lset(2);
        a.lget(1);
        a.call(F_SLEN);
        a.lset(3);
        a.lget(2);
        a.lget(3);
        a.raw(&[0x6A]);
        a.call(F_SNEW);
        a.lset(4); // p
        a.lget(4);
        a.i32c(8);
        a.raw(&[0x6A]); // dst = p + 8
        a.lget(0);
        a.i32c(8);
        a.raw(&[0x6A]); // src = a + 8
        a.lget(2);
        a.call(F_MEMCPY);
        a.lget(4);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.lget(2);
        a.raw(&[0x6A]); // dst = p + 8 + la
        a.lget(1);
        a.i32c(8);
        a.raw(&[0x6A]); // src = b + 8
        a.lget(3);
        a.call(F_MEMCPY);
        a.lget(4);
        m.add_func(
            &[VT::I32, VT::I32],
            &[VT::I32],
            vec![VT::I32, VT::I32, VT::I32],
            a.0,
        );
    }

    // ---- $seq(a, b) -> i64 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_SLEN);
        a.lset(2);
        a.lget(1);
        a.call(F_SLEN);
        a.lset(3);
        a.lget(2);
        a.lget(3);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.i64c(0);
            a.o(0x0F);
        });
        a.i32c(0);
        a.lset(4);
        a.i64c(1);
        a.lset(5);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(4);
        a.lget(2);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(0);
        a.lget(4);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.lget(1);
        a.lget(4);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.i64c(0);
            a.lset(5);
            a.br(1);
        });
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        a.lget(5);
        m.add_func(
            &[VT::I32, VT::I32],
            &[VT::I64],
            vec![VT::I32, VT::I32, VT::I32, VT::I64],
            a.0,
        );
    }

    // ---- $scmp(a, b) -> i64 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_SLEN);
        a.lset(2);
        a.lget(1);
        a.call(F_SLEN);
        a.lset(3);
        a.i32c(0);
        a.lset(4);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(4);
        a.lget(2);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(4);
        a.lget(3);
        a.raw(&[0x4E]);
        a.br_if(1);
        // bytes ভিন্ন কি না
        a.lget(0);
        a.lget(4);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.lget(1);
        a.lget(4);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.raw(&[0x46]);
        a.raw(&[0x45]); // eqz → ভিন্ন হলে 1
        a.if_void(|a| {
            a.lget(0);
            a.lget(4);
            a.raw(&[0x6A]);
            a.load(0x2D, 0, 8);
            a.lget(1);
            a.lget(4);
            a.raw(&[0x6A]);
            a.load(0x2D, 0, 8);
            a.raw(&[0x48]); // a<b?
            a.if_void(|a| {
                a.i64c(-1);
                a.o(0x0F);
            });
            a.i64c(1);
            a.o(0x0F);
        });
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        // সব সমান → length তুলনা
        a.lget(2);
        a.lget(3);
        a.raw(&[0x48]);
        a.if_void(|a| {
            a.i64c(-1);
            a.o(0x0F);
        });
        a.lget(2);
        a.lget(3);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.i64c(1);
            a.o(0x0F);
        });
        a.i64c(0);
        m.add_func(
            &[VT::I32, VT::I32],
            &[VT::I64],
            vec![VT::I32, VT::I32, VT::I32],
            a.0,
        );
    }

    // ---- $i2s(x: i64) -> ptr ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.i64c(0);
        a.raw(&[0x53]);
        a.lset(1);
        a.i64c(0);
        a.lget(0);
        a.raw(&[0x7D]);
        a.lget(0);
        a.lget(1);
        a.o(0x1B);
        a.lset(2);
        // zero
        a.blk(0x02, 0x40);
        a.lget(2);
        a.raw(&[0x50]);
        a.raw(&[0x45]);
        a.br_if(0);
        a.i32c(1);
        a.call(F_SNEW);
        a.lset(3);
        a.lget(3);
        a.i32c(48);
        a.load(0x3A, 0, 8);
        a.lget(3);
        a.o(0x0F);
        a.end();
        // digits
        a.i32c(21);
        a.call(F_SNEW);
        a.lset(3);
        a.i32c(20);
        a.lset(4);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(2);
        a.raw(&[0x50]);
        a.br_if(1);
        a.lget(3);
        a.lget(4);
        a.raw(&[0x6A]); // addr = p + i (offset 8 মিলবে)
        a.lget(2);
        a.i64c(10);
        a.raw(&[0x81]);
        a.o(0xA7);
        a.i32c(48);
        a.raw(&[0x6A]); // digit = '0' + u%10
        a.load(0x3A, 0, 8);
        a.lget(2);
        a.i64c(10);
        a.raw(&[0x80]);
        a.lset(2);
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        a.lget(1);
        a.if_void(|a| {
            a.lget(3);
            a.lget(4);
            a.raw(&[0x6A]); // addr = p + i
            a.i32c(45); // '-'
            a.load(0x3A, 0, 8);
            a.lget(4);
            a.i32c(1);
            a.raw(&[0x6B]);
            a.lset(4);
        });
        a.i32c(20);
        a.lget(4);
        a.raw(&[0x6B]);
        a.lset(5);
        a.lget(5);
        a.call(F_SNEW);
        a.lset(6);
        a.lget(6);
        a.i32c(8);
        a.raw(&[0x6A]); // dst = q + 8
        a.lget(3);
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.i32c(8); // src = p + i + 1 + 8
        a.raw(&[0x6A]);
        a.raw(&[0x6A]);
        a.lget(5);
        a.call(F_MEMCPY);
        a.lget(6);
        m.add_func(
            &[VT::I64],
            &[VT::I32],
            vec![VT::I32, VT::I64, VT::I32, VT::I32, VT::I32, VT::I32],
            a.0,
        );
    }

    // ---- $f2s(x: f64) -> ptr ----
    {
        let nan_a = sc(m, "NaN");
        let inf_a = sc(m, "inf");
        let ninf_a = sc(m, "-inf");
        let mut a = Asm::new();
        a.lget(0);
        a.lget(0);
        a.raw(&[0x62]); // f64.ne → NaN?
        a.if_void(|a| {
            a.i32c(nan_a as i32);
            a.o(0x0F);
        });
        a.lget(0);
        a.f64c(f64::INFINITY);
        a.raw(&[0x61]); // f64.eq
        a.if_void(|a| {
            a.lget(0);
            a.f64c(0.0);
            a.raw(&[0x63]); // f64.lt
            a.if_void(|a| {
                a.i32c(ninf_a as i32);
                a.o(0x0F);
            });
            a.i32c(inf_a as i32);
            a.o(0x0F);
        });
        a.lget(0);
        a.f64c(0.0);
        a.raw(&[0x63]); // f64.lt
        a.lset(1); // neg
        a.lget(0);
        a.raw(&[0x99]);
        a.lset(0); // |x|
        // ip = trunc(x)
        a.lget(0);
        a.raw(&[0xFC, 0x06]); // i64.trunc_sat_f64_s
        a.lset(2);
        // d = trunc((x - ip) * 1e6 + 0.5)
        a.lget(0);
        a.lget(2);
        a.raw(&[0xB9]);
        a.raw(&[0xA1]);
        a.f64c(1e6);
        a.raw(&[0xA2]);
        a.f64c(0.5);
        a.raw(&[0xA0]);
        a.raw(&[0xFC, 0x06]);
        a.lset(3); // d
        // carry
        a.lget(3);
        a.i64c(1_000_000);
        a.raw(&[0x55]);
        a.if_void(|a| {
            a.lget(2);
            a.i64c(1);
            a.raw(&[0x7C]);
            a.lset(2);
            a.lget(3);
            a.i64c(1_000_000);
            a.raw(&[0x7D]);
            a.lset(3);
        });
        // frac buffer
        a.i32c(6);
        a.call(F_SNEW);
        a.lset(5);
        a.i32c(6);
        a.lset(6); // end
        // d == 0 → "0"; নইলে ৬ অংক + trim
        a.lget(3);
        a.raw(&[0x50]);
        a.blk(0x04, 0x40);
        // d == 0 branch
        a.lget(5);
        a.i32c(48);
        a.load(0x3A, 0, 8);
        a.i32c(1);
        a.lset(6);
        a.o(0x05); // else
        // ডিফল্ট: buf = "000000"
        a.i32c(0);
        a.lset(7);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(7);
        a.i32c(6);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(5);
        a.lget(7);
        a.raw(&[0x6A]);
        a.i32c(48);
        a.load(0x3A, 0, 8);
        a.lget(7);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(7);
        a.br(0);
        a.end();
        a.end();
        // d-এর অংক ডান দিক থেকে
        a.i32c(5);
        a.lset(7);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(3);
        a.raw(&[0x50]);
        a.br_if(1);
        a.lget(5);
        a.lget(7);
        a.raw(&[0x6A]); // addr = scratch + j
        a.lget(3);
        a.i64c(10);
        a.raw(&[0x81]);
        a.o(0xA7);
        a.i32c(48);
        a.raw(&[0x6A]);
        a.load(0x3A, 0, 8);
        a.lget(3);
        a.i64c(10);
        a.raw(&[0x80]);
        a.lset(3);
        a.lget(7);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.lset(7);
        a.br(0);
        a.end();
        a.end();
        // trim
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(6);
        a.i32c(1);
        a.raw(&[0x4C]);
        a.br_if(1);
        a.lget(5);
        a.lget(6);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.i32c(48);
        a.raw(&[0x47]);
        a.br_if(1);
        a.lget(6);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.lset(6);
        a.br(0);
        a.end();
        a.end();
        a.end(); // if/else শেষ
        a.lget(2);
        a.call(F_I2S);
        a.lset(4); // ip_str
        a.lget(4);
        a.call(F_SLEN);
        a.lget(1);
        a.raw(&[0x6A]);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lget(6);
        a.raw(&[0x6A]);
        a.lset(8); // total
        a.lget(8);
        a.call(F_SNEW);
        a.lset(9); // q
        a.lget(1);
        a.if_void(|a| {
            a.lget(9);
            a.i32c(45);
            a.i32c(0);
            a.raw(&[0x6A]);
            a.load(0x3A, 0, 8);
        });
        a.lget(9);
        a.lget(1);
        a.raw(&[0x6A]);
        a.i32c(8);
        a.raw(&[0x6A]); // dst = q + neg + 8
        a.lget(4);
        a.i32c(8);
        a.raw(&[0x6A]); // src = ip_str + 8
        a.lget(4);
        a.call(F_SLEN);
        a.call(F_MEMCPY);
        // q[8+neg+slen_ip] = '.'
        a.lget(9);
        a.lget(1);
        a.raw(&[0x6A]);
        a.lget(4);
        a.call(F_SLEN);
        a.raw(&[0x6A]);
        a.i32c(46);
        a.load(0x3A, 0, 8); // memarg offset 8-ই যথেষ্ট
        // memcpy(q+8+neg+slen_ip+1, scratch+8, end)
        a.lget(9);
        a.lget(1);
        a.raw(&[0x6A]);
        a.lget(4);
        a.call(F_SLEN);
        a.raw(&[0x6A]);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.lget(5);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.lget(6);
        a.call(F_MEMCPY);
        a.lget(9);
        m.add_func(
            &[VT::F64],
            &[VT::I32],
            vec![VT::I32, VT::I64, VT::I64, VT::I32, VT::I32, VT::I32, VT::I32, VT::I32, VT::I32],
            a.0,
        );
    }

    // ---- $b2s(b: i64) -> ptr ----
    {
        let t = sc(m, "true");
        let f = sc(m, "false");
        let mut a = Asm::new();
        a.lget(0);
        a.raw(&[0x50]); // i64.eqz
        a.raw(&[0x45]); // i32.eqz → 1 যদি b != 0
        a.blk(0x04, 0x7F); // if result i32
        a.i32c(t as i32);
        a.o(0x05);
        a.i32c(f as i32);
        a.end();
        m.add_func(&[VT::I64], &[VT::I32], vec![], a.0);
    }

    // ---- $tostr(p) -> ptr ----
    {
        let unit_a = sc(m, "unit");
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(1);
        a.lget(1);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.call(F_I2S);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(1);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.raw(&[0xBF]);
            a.call(F_F2S);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(2);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.o(0xA7);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(3);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.call(F_B2S);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(4);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_AREPR);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(5);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_MREPR);
            a.o(0x0F);
        });
        a.i32c(unit_a as i32);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $arepr(a) -> ptr ----
    {
        let lb = sc(m, "[");
        let rb = sc(m, "]");
        let cm = sc(m, ", ");
        let mut a = Asm::new();
        a.i32c(lb as i32);
        a.lset(1);
        a.lget(0);
        a.call(F_ALEN_RAW);
        a.lset(3);
        a.i32c(0);
        a.lset(2);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(2);
        a.lget(3);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(2);
        a.raw(&[0x45]);
        a.raw(&[0x45]); // 1 যদি i != 0
        a.if_void(|a| {
            a.lget(1);
            a.i32c(cm as i32);
            a.call(F_SCAT);
            a.lset(1);
        });
        a.lget(0);
        a.lget(2);
        a.o(0xAD);
        a.call(F_AGET);
        a.lset(4);
        a.lget(1);
        a.lget(4);
        a.call(F_TOSTR);
        a.call(F_SCAT);
        a.lset(1);
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(2);
        a.br(0);
        a.end();
        a.end();
        a.lget(1);
        a.i32c(rb as i32);
        a.call(F_SCAT);
        m.add_func(
            &[VT::I32],
            &[VT::I32],
            vec![VT::I32, VT::I32, VT::I32, VT::I32],
            a.0,
        );
    }

    // ---- $mrepr(m) -> ptr ----
    {
        let lb = sc(m, "{");
        let rb = sc(m, "}");
        let cm = sc(m, ", ");
        let co = sc(m, ": ");
        let mut a = Asm::new();
        a.i32c(lb as i32);
        a.lset(1);
        a.lget(0);
        a.call(F_MHDR_LEN);
        a.lset(3);
        a.i32c(0);
        a.lset(2);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(2);
        a.lget(3);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(2);
        a.raw(&[0x45]);
        a.raw(&[0x45]); // 1 যদি i != 0
        a.if_void(|a| {
            a.lget(1);
            a.i32c(cm as i32);
            a.call(F_SCAT);
            a.lset(1);
        });
        a.lget(0);
        a.call(F_MHDR_BASE);
        a.lget(2);
        a.i32c(32);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.ltee(4);
        a.i32c(16);
        a.raw(&[0x6A]);
        a.lset(5);
        a.lget(1);
        a.lget(4);
        a.call(F_TOSTR);
        a.call(F_SCAT);
        a.lset(1);
        a.lget(1);
        a.i32c(co as i32);
        a.call(F_SCAT);
        a.lset(1);
        a.lget(1);
        a.lget(5);
        a.call(F_TOSTR);
        a.call(F_SCAT);
        a.lset(1);
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(2);
        a.br(0);
        a.end();
        a.end();
        a.lget(1);
        a.i32c(rb as i32);
        a.call(F_SCAT);
        m.add_func(
            &[VT::I32],
            &[VT::I32],
            vec![VT::I32, VT::I32, VT::I32, VT::I32, VT::I32],
            a.0,
        );
    }

    // ---- $tof(p) -> f64 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(1);
        a.lget(1);
        a.raw(&[0x45]);
        a.blk(0x04, 0x7C); // if result f64
        a.lget(0);
        a.call(F_UBITS);
        a.raw(&[0xB9]);
        a.o(0x05); // else
        a.lget(1);
        a.i32c(1);
        a.raw(&[0x46]);
        a.blk(0x04, 0x7C);
        a.lget(0);
        a.call(F_UBITS);
        a.raw(&[0xBF]);
        a.o(0x05);
        a.o(0x00); // unreachable
        a.end();
        a.end();
        m.add_func(&[VT::I32], &[VT::F64], vec![VT::I32], a.0);
    }

    // ---- $add / $sub / $mul ----
    for (_fidx, iop, fop) in [(F_ADD, 0x7Cu8, 0xA0u8), (F_SUB, 0x7Du8, 0xA1u8), (F_MUL, 0x7Eu8, 0xA2u8)] {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(2);
        a.lget(1);
        a.call(F_TAG);
        a.lset(3);
        // string?
        a.lget(2);
        a.i32c(2);
        a.raw(&[0x46]);
        a.lget(3);
        a.i32c(2);
        a.raw(&[0x46]);
        a.raw(&[0x72]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_TOSTR);
            a.lget(1);
            a.call(F_TOSTR);
            a.call(F_SCAT);
            a.call(F_SBOX);
            a.o(0x0F);
        });
        // both int?
        a.lget(2);
        a.raw(&[0x45]);
        a.lget(3);
        a.raw(&[0x45]);
        a.raw(&[0x71]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.lget(1);
            a.call(F_UBITS);
            a.raw(&[iop]);
            a.call(F_BOXI);
            a.o(0x0F);
        });
        a.lget(0);
        a.call(F_TOF);
        a.lget(1);
        a.call(F_TOF);
        a.raw(&[fop]);
        a.call(F_BOXF);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I32, VT::I32], a.0);
    }

    // ---- $div (float semantics; ভাগশূন্য → trap) ----
    {
        let mut a = Asm::new();
        a.lget(1);
        a.call(F_TOF);
        a.lset(4); // fb
        a.lget(4);
        a.f64c(0.0);
        a.raw(&[0x61]); // f64.eq → ভাগশূন্য হলে trap
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(0);
        a.call(F_TOF);
        a.lget(4);
        a.raw(&[0xA3]);
        a.call(F_BOXF);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I32, VT::I32, VT::F64], a.0);
    }

    // ---- $mod (শুধু int) ----
    {
        let mut a = Asm::new();
        a.lget(1);
        a.call(F_TAG);
        a.raw(&[0x45]);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(1);
        a.call(F_UBITS);
        a.raw(&[0x50]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(0);
        a.call(F_UBITS);
        a.lget(1);
        a.call(F_UBITS);
        a.raw(&[0x81]);
        a.call(F_BOXI);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $neg ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i64c(0);
            a.lget(0);
            a.call(F_UBITS);
            a.raw(&[0x7D]);
            a.call(F_BOXI);
            a.o(0x0F);
        });
        a.lget(0);
        a.call(F_TAG);
        a.i32c(1);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.raw(&[0xBF]);
            a.raw(&[0x9A]);
            a.call(F_BOXF);
            a.o(0x0F);
        });
        a.o(0x00);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $not / $truthy ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.i32c(3);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(0);
        a.call(F_UBITS);
        a.raw(&[0x50]); // i64.eqz → i32
        a.raw(&[0xAD]); // extend → i64
        a.call(F_BOXB);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.i32c(3);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(0);
        a.call(F_UBITS);
        a.raw(&[0x50]);
        a.raw(&[0x45]);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $lt / $gt / $le / $ge ----
    for (_fidx, iop, fop, mode) in [
        (F_LT, 0x53u8, 0x63u8, 0u8),
        (F_GT, 0x55u8, 0x64u8, 1u8),
        (F_LE, 0x57u8, 0x65u8, 2u8),
        (F_GE, 0x59u8, 0x66u8, 3u8),
    ] {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(2);
        a.lget(1);
        a.call(F_TAG);
        a.lset(3);
        // both string?
        a.lget(2);
        a.i32c(2);
        a.raw(&[0x46]);
        a.lget(3);
        a.i32c(2);
        a.raw(&[0x46]);
        a.raw(&[0x71]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.o(0xA7);
            a.lget(1);
            a.call(F_UBITS);
            a.o(0xA7);
            a.call(F_SCMP);
            match mode {
                0 => {
                    a.i64c(0);
                    a.raw(&[0x53]);
                }
                1 => {
                    a.i64c(0);
                    a.raw(&[0x55]);
                }
                2 => {
                    a.i64c(0);
                    a.raw(&[0x57]);
                }
                _ => {
                    a.i64c(0);
                    a.raw(&[0x59]);
                }
            }
            a.raw(&[0xAD]);
            a.call(F_BOXB);
            a.o(0x0F);
        });
        // both int?
        a.lget(2);
        a.raw(&[0x45]);
        a.lget(3);
        a.raw(&[0x45]);
        a.raw(&[0x71]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.lget(1);
            a.call(F_UBITS);
            a.raw(&[iop]);
            a.raw(&[0xAD]);
            a.call(F_BOXB);
            a.o(0x0F);
        });
        a.lget(0);
        a.call(F_TOF);
        a.lget(1);
        a.call(F_TOF);
        a.raw(&[fop]);
        a.raw(&[0xAD]);
        a.call(F_BOXB);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I32, VT::I32], a.0);
    }

    // ---- $veq(a, b) -> i64 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(2);
        a.lget(1);
        a.call(F_TAG);
        a.lset(3);
        // numeric cross (int/float)
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x4C]);
        a.lget(3);
        a.i32c(1);
        a.raw(&[0x4C]);
        a.raw(&[0x71]);
        a.lget(2);
        a.lget(3);
        a.raw(&[0x47]);
        a.raw(&[0x71]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_TOF);
            a.lget(1);
            a.call(F_TOF);
            a.raw(&[0x61]); // f64.eq
            a.raw(&[0xAD]);
            a.call(F_BOXB);
            a.call(F_UBITS);
            a.o(0x0F);
        });
        a.lget(2);
        a.lget(3);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.i64c(0);
            a.o(0x0F);
        });
        // int
        a.lget(2);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.lget(1);
            a.call(F_UBITS);
            a.raw(&[0x51]);
            a.raw(&[0xAD]); // i32 → i64
            a.o(0x0F);
        });
        // float
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.lget(1);
            a.call(F_UBITS);
            a.raw(&[0x51]);
            a.raw(&[0xAD]); // i32 → i64
            a.o(0x0F);
        });
        // string
        a.lget(2);
        a.i32c(2);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.o(0xA7);
            a.lget(1);
            a.call(F_UBITS);
            a.o(0xA7);
            a.call(F_SEQ);
            a.o(0x0F);
        });
        // bool
        a.lget(2);
        a.i32c(3);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.lget(1);
            a.call(F_UBITS);
            a.raw(&[0x51]);
            a.raw(&[0xAD]); // i32 → i64
            a.o(0x0F);
        });
        // array — deep
        a.lget(2);
        a.i32c(4);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_ALEN_RAW);
            a.lget(1);
            a.call(F_ALEN_RAW);
            a.raw(&[0x47]);
            a.if_void(|a| {
                a.i64c(0);
                a.o(0x0F);
            });
            a.i32c(0);
            a.lset(4);
            a.blk(0x02, 0x40);
            a.blk(0x03, 0x40);
            a.lget(4);
            a.lget(0);
            a.call(F_ALEN_RAW);
            a.raw(&[0x4E]);
            a.br_if(1);
            a.lget(0);
            a.lget(4);
            a.call(F_AGET_CELL);
            a.lget(1);
            a.lget(4);
            a.call(F_AGET_CELL);
            a.call(F_VEQ);
            a.raw(&[0x50]);
            a.if_void(|a| {
                a.i64c(0);
                a.o(0x0F);
            });
            a.lget(4);
            a.i32c(1);
            a.raw(&[0x6A]);
            a.lset(4);
            a.br(0);
            a.end();
            a.end();
            a.i64c(1);
            a.o(0x0F);
        });
        // map — deep
        a.lget(2);
        a.i32c(5);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_MHDR_LEN);
            a.lget(1);
            a.call(F_MHDR_LEN);
            a.raw(&[0x47]);
            a.if_void(|a| {
                a.i64c(0);
                a.o(0x0F);
            });
            // for each entry i in a: scan j in b → মেলেনি হলে 0 return
            a.i32c(0);
            a.lset(4); // i
            a.blk(0x02, 0x40); // done
            a.blk(0x03, 0x40); // iloop
            a.lget(4);
            a.lget(0);
            a.call(F_MHDR_LEN);
            a.raw(&[0x4E]);
            a.br_if(1); // সব দেখা হলো → done
            // kcell_i, vcell_i
            a.lget(0);
            a.call(F_MHDR_BASE);
            a.lget(4);
            a.i32c(32);
            a.raw(&[0x6C]);
            a.raw(&[0x6A]);
            a.ltee(5);
            a.i32c(16);
            a.raw(&[0x6A]);
            a.lset(6);
            // found = 0 (local 12 = I64)
            a.i64c(0);
            a.lset(12);
            // j scan
            a.i32c(0);
            a.lset(7);
            a.blk(0x02, 0x40); // jdone
            a.blk(0x03, 0x40); // jloop
            a.lget(7);
            a.lget(1);
            a.call(F_MHDR_LEN);
            a.raw(&[0x4E]);
            a.br_if(1); // j শেষ → jdone
            a.lget(1);
            a.call(F_MHDR_BASE);
            a.lget(7);
            a.i32c(32);
            a.raw(&[0x6C]);
            a.raw(&[0x6A]);
            a.ltee(8);
            a.i32c(16);
            a.raw(&[0x6A]);
            a.lset(9);
            // match = veq(k_i,k_j) && veq(v_i,v_j)
            a.lget(5);
            a.lget(8);
            a.call(F_VEQ);
            a.lget(6);
            a.lget(9);
            a.call(F_VEQ);
            a.raw(&[0x83]); // i64.and
            a.raw(&[0x50]); // মেলেনি → 1
            a.if_void(|a| {
                a.br(2); // মেলেনি → jdone (if=0, jloop=1, jdone=2)
            });
            a.i64c(1);
            a.lset(12); // found = 1
            a.br(1); // jdone
            a.end(); // jloop
            a.end(); // jdone
            // found == 0 → return 0
            a.lget(12);
            a.raw(&[0x50]);
            a.if_void(|a| {
                a.i64c(0);
                a.o(0x0F);
            });
            a.lget(4);
            a.i32c(1);
            a.raw(&[0x6A]);
            a.lset(4);
            a.br(0); // iloop চালিয়ে যাও
            a.end(); // iloop
            a.end(); // done
            a.i64c(1);
            a.o(0x0F);
        });
        // unit
        a.i64c(1);
        m.add_func(
            &[VT::I32, VT::I32],
            &[VT::I64],
            vec![
                VT::I32, VT::I32, VT::I32, VT::I32, VT::I32, VT::I32, VT::I32, VT::I32, VT::I32,
                VT::I32, VT::I64,
            ],
            a.0,
        );
    }
}

fn build_helpers2(m: &mut Mod) {
    let sc = |mm: &mut Mod, s: &str| mm.str_const(s);

    // ---- $arrnew() -> i32 ----
    {
        let mut a = Asm::new();
        a.i32c(16);
        a.call(F_ALLOC);
        a.lset(0);
        for off in [0u32, 4, 8] {
            a.lget(0);
            a.i32c(0);
            a.load(0x36, 2, off);
        }
        a.i32c(4);
        a.lget(0);
        a.raw(&[0xAD]);
        a.call(F_BOX);
        m.add_func(&[], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $apush(a, v) ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.lset(2); // hdr
        a.lget(2);
        a.load(0x28, 2, 0);
        a.lset(3); // len
        a.lget(2);
        a.load(0x28, 2, 4);
        a.lset(4); // cap
        a.lget(3);
        a.lget(4);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(4);
            a.raw(&[0x45]);
            a.blk(0x04, 0x40);
            a.i32c(4);
            a.lset(6);
            a.o(0x05);
            a.lget(4);
            a.i32c(2);
            a.raw(&[0x6C]);
            a.lset(6);
            a.end();
            a.lget(6);
            a.i32c(16);
            a.raw(&[0x6C]);
            a.call(F_ALLOC);
            a.lset(5); // nb
            a.lget(2);
            a.load(0x28, 2, 8);
            a.lset(7); // old cells
            a.lget(7);
            a.raw(&[0x45]);
            a.if_void(|a| {
                a.lget(5);
                a.lget(7);
                a.lget(3);
                a.i32c(16);
                a.raw(&[0x6C]);
                a.call(F_MEMCPY);
            });
            a.lget(2);
            a.lget(5);
            a.load(0x36, 2, 8);
            a.lget(2);
            a.lget(6);
            a.load(0x36, 2, 4);
        });
        a.lget(2);
        a.load(0x28, 2, 8);
        a.lset(7);
        a.lget(7);
        a.lget(3);
        a.i32c(16);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.lget(1);
        a.i32c(16);
        a.call(F_MEMCPY);
        a.lget(2);
        a.lget(3);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.load(0x36, 2, 0);
        m.add_func(&[VT::I32, VT::I32], &[], vec![VT::I32; 6], a.0);
    }

    // ---- $alen_raw(a) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.load(0x28, 2, 0);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $aget(a, idx: i64) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.lset(2); // hdr
        a.lget(2);
        a.load(0x28, 2, 0);
        a.lset(3); // len
        // ni = idx<0 ? idx+len : idx
        a.lget(1);
        a.i64c(0);
        a.raw(&[0x53]);
        a.blk(0x04, 0x7E);
        a.lget(1);
        a.lget(3);
        a.raw(&[0xAC]);
        a.raw(&[0x7C]);
        a.o(0x05);
        a.lget(1);
        a.end();
        a.lset(4); // ni
        a.lget(4);
        a.i64c(0);
        a.raw(&[0x53]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(4);
        a.lget(3);
        a.raw(&[0xAC]);
        a.raw(&[0x59]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.i32c(16);
        a.call(F_ALLOC);
        a.lset(5);
        a.lget(5);
        a.lget(2);
        a.load(0x28, 2, 8);
        a.lget(4);
        a.o(0xA7);
        a.i32c(16);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.i32c(16);
        a.call(F_MEMCPY);
        a.lget(5);
        m.add_func(&[VT::I32, VT::I64], &[VT::I32], vec![VT::I32, VT::I32, VT::I64, VT::I32], a.0);
    }

    // ---- $aget_cell(a, i: i32) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.load(0x28, 2, 8);
        a.lget(1);
        a.i32c(16);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $aset(a, idx: i64, v) ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.lset(3); // hdr
        a.lget(3);
        a.load(0x28, 2, 0);
        a.lset(4); // len
        a.lget(1);
        a.i64c(0);
        a.raw(&[0x53]);
        a.blk(0x04, 0x7E);
        a.lget(1);
        a.lget(4);
        a.raw(&[0xAC]);
        a.raw(&[0x7C]);
        a.o(0x05);
        a.lget(1);
        a.end();
        a.lset(5); // ni
        a.lget(5);
        a.i64c(0);
        a.raw(&[0x53]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(5);
        a.lget(4);
        a.raw(&[0xAC]);
        a.raw(&[0x59]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(3);
        a.load(0x28, 2, 8);
        a.lget(5);
        a.o(0xA7);
        a.i32c(16);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.lget(2);
        a.i32c(16);
        a.call(F_MEMCPY);
        m.add_func(&[VT::I32, VT::I64, VT::I32], &[], vec![VT::I32, VT::I32, VT::I64], a.0);
    }

    // ---- $apop(a) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.lset(1);
        a.lget(1);
        a.load(0x28, 2, 0);
        a.lset(2); // len
        a.lget(2);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.i32c(16);
        a.call(F_ALLOC);
        a.lset(3);
        a.lget(3);
        a.lget(1);
        a.load(0x28, 2, 8);
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.i32c(16);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.i32c(16);
        a.call(F_MEMCPY);
        a.lget(1);
        a.lget(2);
        a.i32c(1);
        a.raw(&[0x6B]);
        a.load(0x36, 2, 0);
        a.lget(3);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32, VT::I32, VT::I32], a.0);
    }

    // ---- $mapnew() -> i32 ----
    {
        let mut a = Asm::new();
        a.i32c(16);
        a.call(F_ALLOC);
        a.lset(0);
        for off in [0u32, 4, 8] {
            a.lget(0);
            a.i32c(0);
            a.load(0x36, 2, off);
        }
        a.i32c(5);
        a.lget(0);
        a.raw(&[0xAD]);
        a.call(F_BOX);
        m.add_func(&[], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $mhdr_len(m) -> i32 / $mhdr_base(m) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.load(0x28, 2, 0);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.load(0x28, 2, 8);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $mset(m, k, v) ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.lset(3); // hdr
        a.lget(3);
        a.load(0x28, 2, 0);
        a.lset(4); // len
        a.lget(3);
        a.load(0x28, 2, 4);
        a.lset(5); // cap
        a.lget(3);
        a.load(0x28, 2, 8);
        a.lset(6); // base
        a.i32c(0);
        a.lset(7); // j
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(7);
        a.lget(4);
        a.raw(&[0x4E]);
        a.br_if(1);
        // kcell = base + j*32
        a.lget(6);
        a.lget(7);
        a.i32c(32);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.lset(8);
        // match? → v বদলে br done
        a.lget(1);
        a.lget(8);
        a.call(F_VEQ);
        a.raw(&[0x50]);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.lget(8);
            a.i32c(16);
            a.raw(&[0x6A]);
            a.lget(2);
            a.i32c(16);
            a.call(F_MEMCPY);
            a.br(2); // done (if=0, jloop=1, done=2)
        });
        a.lget(7);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(7);
        a.br(0);
        a.end();
        a.end();
        // append
        a.lget(4);
        a.lget(5);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(5);
            a.raw(&[0x45]);
            a.blk(0x04, 0x40);
            a.i32c(4);
            a.lset(9);
            a.o(0x05);
            a.lget(5);
            a.i32c(2);
            a.raw(&[0x6C]);
            a.lset(9);
            a.end();
            a.lget(9);
            a.i32c(32);
            a.raw(&[0x6C]);
            a.call(F_ALLOC);
            a.lset(10); // nb
            a.lget(10);
            a.lget(6);
            a.lget(4);
            a.i32c(32);
            a.raw(&[0x6C]);
            a.call(F_MEMCPY);
            a.lget(3);
            a.lget(10);
            a.load(0x36, 2, 8);
            a.lget(3);
            a.lget(9);
            a.load(0x36, 2, 4);
            a.lget(3);
            a.load(0x28, 2, 8);
            a.lset(6); // base reload
        });
        a.lget(6);
        a.lget(4);
        a.i32c(32);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.lset(8); // new kcell
        a.lget(8);
        a.lget(1);
        a.i32c(16);
        a.call(F_MEMCPY);
        a.lget(8);
        a.i32c(16);
        a.raw(&[0x6A]);
        a.lget(2);
        a.i32c(16);
        a.call(F_MEMCPY);
        a.lget(3);
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.load(0x36, 2, 0);
        m.add_func(
            &[VT::I32, VT::I32, VT::I32],
            &[],
            vec![VT::I32; 8],
            a.0,
        );
    }

    // ---- $mget(m, k) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_UBITS);
        a.o(0xA7);
        a.lset(2);
        a.lget(2);
        a.load(0x28, 2, 0);
        a.lset(3);
        a.lget(2);
        a.load(0x28, 2, 8);
        a.lset(4);
        a.i32c(0);
        a.lset(5);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(5);
        a.lget(3);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(4);
        a.lget(5);
        a.i32c(32);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.lset(6); // kcell
        a.lget(1);
        a.lget(6);
        a.call(F_VEQ);
        a.raw(&[0x50]);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i32c(16);
            a.call(F_ALLOC);
            a.lset(7);
            a.lget(7);
            a.lget(6);
            a.i32c(16);
            a.raw(&[0x6A]);
            a.i32c(16);
            a.call(F_MEMCPY);
            a.lget(7);
            a.o(0x0F);
        });
        a.lget(5);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(5);
        a.br(0);
        a.end();
        a.end();
        a.o(0x00); // key নেই → trap
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I32; 6], a.0);
    }

    // ---- $mkeys(m) / $mvals(m) -> i32 ----
    for (_fidx, off) in [(F_MKEYS, 0u32), (F_MVALS, 16u32)] {
        let mut a = Asm::new();
        a.call(F_ARRNEW);
        a.lset(1); // arr
        a.lget(0);
        a.call(F_MHDR_LEN);
        a.lset(2); // len
        a.lget(0);
        a.call(F_MHDR_BASE);
        a.lset(3); // base
        a.i32c(0);
        a.lset(4); // i
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(4);
        a.lget(2);
        a.raw(&[0x4E]);
        a.br_if(1);
        // cell copy
        a.i32c(16);
        a.call(F_ALLOC);
        a.lset(5);
        a.lget(5);
        a.lget(3);
        a.lget(4);
        a.i32c(32);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.i32c(off as i32);
        a.raw(&[0x6A]);
        a.i32c(16);
        a.call(F_MEMCPY);
        a.lget(1);
        a.lget(5);
        a.call(F_APUSH);
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        a.lget(1);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32; 5], a.0);
    }

    // ---- $mhas(m, k) -> i32 (box bool) ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_MHDR_LEN);
        a.lset(2);
        a.lget(0);
        a.call(F_MHDR_BASE);
        a.lset(3);
        a.i32c(0);
        a.lset(4);
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(4);
        a.lget(2);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(3);
        a.lget(4);
        a.i32c(32);
        a.raw(&[0x6C]);
        a.raw(&[0x6A]);
        a.lset(5);
        a.lget(1);
        a.lget(5);
        a.call(F_VEQ);
        a.raw(&[0x50]);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i64c(1);
            a.call(F_BOXB);
            a.o(0x0F);
        });
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        a.i64c(0);
        a.call(F_BOXB);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I32; 4], a.0);
    }

    // ---- $out(s) ----
    {
        let mut a = Asm::new();
        a.gget(m.g_outp);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i32c(64);
            a.call(F_ALLOC);
            a.gset(m.g_outp);
            a.i32c(64);
            a.gset(m.g_outc);
        });
        a.gget(m.g_outl);
        a.lget(0);
        a.call(F_SLEN);
        a.raw(&[0x6A]);
        a.lset(1); // need
        a.lget(1);
        a.gget(m.g_outc);
        a.raw(&[0x4A]);
        a.if_void(|a| {
            a.lget(1);
            a.i32c(2);
            a.raw(&[0x6C]);
            a.call(F_ALLOC);
            a.lset(2);
            a.lget(2);
            a.gget(m.g_outp);
            a.gget(m.g_outl);
            a.call(F_MEMCPY);
            a.lget(2);
            a.gset(m.g_outp);
            a.lget(1);
            a.i32c(2);
            a.raw(&[0x6C]);
            a.gset(m.g_outc);
        });
        a.gget(m.g_outp);
        a.gget(m.g_outl);
        a.raw(&[0x6A]);
        a.lget(0);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.lget(0);
        a.call(F_SLEN);
        a.call(F_MEMCPY);
        a.gget(m.g_outl);
        a.lget(0);
        a.call(F_SLEN);
        a.raw(&[0x6A]);
        a.gset(m.g_outl);
        m.add_func(&[VT::I32], &[], vec![VT::I32, VT::I32], a.0);
    }

    // ---- $scontains(a, b) -> i64 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_SLEN);
        a.lset(2); // la
        a.lget(1);
        a.call(F_SLEN);
        a.lset(3); // lb
        a.lget(3);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i64c(1);
            a.o(0x0F);
        });
        a.i32c(0);
        a.lset(4); // i
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        // i > la-lb → done
        a.lget(4);
        a.lget(2);
        a.lget(3);
        a.raw(&[0x6B]);
        a.raw(&[0x4A]);
        a.br_if(1);
        // matched = 1; j scan
        a.i64c(1);
        a.lset(7); // matched (local 7 = I64)
        a.i32c(0);
        a.lset(5);
        a.blk(0x02, 0x40); // jdone
        a.blk(0x03, 0x40); // jloop
        a.lget(5);
        a.lget(3);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(0);
        a.lget(4);
        a.lget(5);
        a.raw(&[0x6A]);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.lget(1);
        a.lget(5);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.raw(&[0x47]);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i64c(0);
            a.lset(7); // matched = 0
            a.br(2); // jdone (if=0, jloop=1, jdone=2)
        });
        a.lget(5);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(5);
        a.br(0);
        a.end(); // jloop
        a.end(); // jdone
        // matched → 1 return
        a.lget(7);
        a.raw(&[0x50]); // i64.eqz
        a.raw(&[0x45]); // i32.eqz → 1 যদি matched
        a.if_void(|a| {
            a.i64c(1);
            a.o(0x0F);
        });
        a.lget(4);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        a.i64c(0);
        m.add_func(&[VT::I32, VT::I32], &[VT::I64], vec![VT::I32, VT::I32, VT::I32, VT::I32, VT::I32, VT::I64], a.0);
    }

    // ---- $schars(s) -> i32 ----
    {
        let mut a = Asm::new();
        a.call(F_ARRNEW);
        a.lset(1);
        a.i32c(0);
        a.lset(2); // i
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(2);
        a.lget(0);
        a.call(F_SLEN);
        a.raw(&[0x4E]);
        a.br_if(1);
        // b0
        a.lget(0);
        a.lget(2);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.lset(3); // b0
        // blen
        a.lget(3);
        a.i32c(0xF0);
        a.raw(&[0x4E]);
        a.blk(0x04, 0x7F);
        a.i32c(4);
        a.o(0x05);
        a.lget(3);
        a.i32c(0xE0);
        a.raw(&[0x4E]);
        a.blk(0x04, 0x7F);
        a.i32c(3);
        a.o(0x05);
        a.lget(3);
        a.i32c(0xC0);
        a.raw(&[0x4E]);
        a.blk(0x04, 0x7F);
        a.i32c(2);
        a.o(0x05);
        a.i32c(1);
        a.end();
        a.end();
        a.end();
        a.lset(4); // blen
        a.lget(4);
        a.call(F_SNEW);
        a.lset(5); // cs
        a.lget(5);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.lget(0);
        a.lget(2);
        a.raw(&[0x6A]);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.lget(4);
        a.call(F_MEMCPY);
        a.lget(1);
        a.lget(5);
        a.call(F_SBOX);
        a.call(F_APUSH);
        a.lget(2);
        a.lget(4);
        a.raw(&[0x6A]);
        a.lset(2);
        a.br(0);
        a.end();
        a.end();
        a.lget(1);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32; 5], a.0);
    }

    // ---- $s2i(s) -> i64 ----
    {
        let mut a = Asm::new();
        a.i32c(0);
        a.lset(1); // i
        a.i64c(1);
        a.lset(2); // sign
        a.i64c(0);
        a.lset(3); // n
        a.i64c(0);
        a.lset(4); // started
        // skip spaces
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(1);
        a.lget(0);
        a.call(F_SLEN);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(0);
        a.lget(1);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.i32c(32);
        a.raw(&[0x47]);
        a.br_if(1);
        a.lget(1);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(1);
        a.br(0);
        a.end();
        a.end();
        // sign
        a.lget(0);
        a.lget(1);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.i32c(45);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.i64c(-1);
            a.lset(2);
            a.lget(1);
            a.i32c(1);
            a.raw(&[0x6A]);
            a.lset(1);
        });
        a.lget(0);
        a.lget(1);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.i32c(43);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(1);
            a.i32c(1);
            a.raw(&[0x6A]);
            a.lset(1);
        });
        // digits
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(1);
        a.lget(0);
        a.call(F_SLEN);
        a.raw(&[0x4E]);
        a.br_if(1);
        a.lget(0);
        a.lget(1);
        a.raw(&[0x6A]);
        a.load(0x2D, 0, 8);
        a.lset(5); // c
        a.lget(5);
        a.i32c(48);
        a.raw(&[0x48]);
        a.br_if(1);
        a.lget(5);
        a.i32c(57);
        a.raw(&[0x4A]);
        a.br_if(1);
        a.lget(3);
        a.i64c(10);
        a.raw(&[0x7C]);
        a.lget(5);
        a.i32c(48);
        a.raw(&[0x6B]);
        a.raw(&[0xAD]);
        a.raw(&[0x7C]);
        a.lset(3);
        a.i64c(1);
        a.lset(4);
        a.lget(1);
        a.i32c(1);
        a.raw(&[0x6A]);
        a.lset(1);
        a.br(0);
        a.end();
        a.end();
        // started? trailing?
        a.lget(4);
        a.raw(&[0x50]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(1);
        a.lget(0);
        a.call(F_SLEN);
        a.raw(&[0x47]);
        a.if_void(|a| {
            a.o(0x00);
        });
        a.lget(3);
        a.lget(2);
        a.raw(&[0x7C]);
        m.add_func(&[VT::I32], &[VT::I64], vec![VT::I32, VT::I64, VT::I64, VT::I64, VT::I32], a.0);
    }

    // ---- $pow(a, b) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(1);
        a.call(F_TAG);
        a.raw(&[0x45]);
        a.if_void(|a| {
            // int^int — e >= 0; e==0 → 1
            a.i64c(1);
            a.lset(2); // r
            a.lget(1);
            a.call(F_UBITS);
            a.lset(3); // e
            a.blk(0x02, 0x40);
            a.blk(0x03, 0x40);
            a.lget(3);
            a.raw(&[0x50]);
            a.br_if(1);
            a.lget(2);
            a.lget(0);
            a.call(F_UBITS);
            a.raw(&[0x7E]); // i64.mul
            a.lset(2);
            a.lget(3);
            a.i64c(1);
            a.raw(&[0x7D]);
            a.lset(3);
            a.br(0);
            a.end();
            a.end();
            a.lget(2);
            a.call(F_BOXI);
            a.o(0x0F);
        });
        // float path — পূর্ণসংখ্যা ঘাত (ঋণাত্মক হলে 1/r)
        a.lget(1);
        a.call(F_TOF);
        a.raw(&[0xFC, 0x06]);
        a.lset(4); // n (i64)
        a.f64c(1.0);
        a.lset(5); // r
        a.lget(4);
        a.raw(&[0x50]);
        a.if_void(|a| {
            a.i64c(0);
            a.lget(4);
            a.raw(&[0x7D]); // 0 - n = |n|
            a.lset(4);
        });
        a.blk(0x02, 0x40);
        a.blk(0x03, 0x40);
        a.lget(4);
        a.raw(&[0x50]);
        a.br_if(1);
        a.lget(5);
        a.lget(0);
        a.call(F_TOF);
        a.raw(&[0xA2]);
        a.lset(5);
        a.lget(4);
        a.i64c(1);
        a.raw(&[0x7D]);
        a.lset(4);
        a.br(0);
        a.end();
        a.end();
        // n<0 ছিল? → 1/r
        a.lget(1);
        a.call(F_TAG);
        a.i32c(1);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(1);
            a.call(F_UBITS);
            a.raw(&[0xBF]);
            a.f64c(0.0);
            a.raw(&[0x65]);
            a.if_void(|a| {
                a.f64c(1.0);
                a.lget(5);
                a.raw(&[0xA3]);
                a.lset(5);
            });
        });
        a.lget(5);
        a.call(F_BOXF);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I64, VT::I64, VT::I64, VT::F64], a.0);
    }

    // ---- $blen / $btype ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(1);
        a.lget(1);
        a.i32c(2);
        a.raw(&[0x46]);
        a.if_void(|a| {
            // UTF-8-aware character গণনা (VM-এর সাথে মিলিয়ে)
            a.lget(0);
            a.call(F_UBITS);
            a.o(0xA7); // wrap → i32
            a.call(F_SCHARS);
            a.call(F_ALEN_RAW);
            a.raw(&[0xAD]);
            a.call(F_BOXI);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(4);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_ALEN_RAW);
            a.raw(&[0xAD]);
            a.call(F_BOXI);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(5);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_MHDR_LEN);
            a.raw(&[0xAD]);
            a.call(F_BOXI);
            a.o(0x0F);
        });
        a.o(0x00);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32], a.0);
    }
    {
        let names = ["int", "float", "string", "bool", "array", "map", "unit"];
        let addrs: Vec<u32> = names.iter().map(|n| sc(m, n)).collect();
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(1);
        // ধাপে ধাপে
        for (tag, addr) in addrs.iter().enumerate() {
            a.lget(1);
            a.i32c(tag as i32);
            a.raw(&[0x46]);
            a.if_void(|a| {
                a.i32c(*addr as i32);
                a.call(F_SBOX);
                a.o(0x0F);
            });
        }
        a.o(0x00);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $bfloor / $bceil / $babs / $bsqrt ----
    for (fidx, fop) in [(F_BFLOOR, 0x9Cu8), (F_BCEIL, 0x9Bu8), (F_BABS, 0x99u8), (F_BSQRT, 0x9Fu8)] {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.raw(&[0x45]);
        a.if_void(|a| {
            // int path
            if fidx == F_BABS {
                a.lget(0);
                a.call(F_UBITS);
                a.raw(&[0x50]); // bits < 0?
                a.raw(&[0x45]);
                a.if_void(|a| {
                    a.i64c(0);
                    a.lget(0);
                    a.call(F_UBITS);
                    a.raw(&[0x7D]);
                    a.call(F_BOXI);
                    a.o(0x0F);
                });
                a.lget(0);
            } else if fidx == F_BSQRT {
                a.lget(0);
                a.call(F_TOF);
                a.raw(&[fop]);
                a.call(F_BOXF);
            } else {
                a.lget(0); // floor/ceil: int অপরিবর্তিত
            }
            a.o(0x0F);
        });
        a.lget(0);
        a.call(F_TAG);
        a.i32c(1);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.raw(&[0xBF]);
            a.raw(&[fop]);
            if fidx == F_BFLOOR || fidx == F_BCEIL {
                a.raw(&[0xFC, 0x06]);
                a.call(F_BOXI);
            } else {
                a.call(F_BOXF);
            }
            a.o(0x0F);
        });
        a.o(0x00);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $bmin / $bmax ----
    for (_fidx, iop, fop) in [(F_BMIN, 0x53u8, 0x63u8), (F_BMAX, 0x55u8, 0x64u8)] {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.raw(&[0x45]);
        a.lget(1);
        a.call(F_TAG);
        a.raw(&[0x45]);
        a.raw(&[0x71]);
        a.if_void(|a| {
            a.lget(1);
            a.call(F_UBITS);
            a.lget(0);
            a.call(F_UBITS);
            a.raw(&[iop]);
            a.blk(0x04, 0x7F);
            a.lget(1);
            a.o(0x05);
            a.lget(0);
            a.end();
            a.o(0x0F);
        });
        a.lget(1);
        a.call(F_TOF);
        a.lget(0);
        a.call(F_TOF);
        a.raw(&[fop]);
        a.blk(0x04, 0x7F);
        a.lget(1);
        a.o(0x05);
        a.lget(0);
        a.end();
        a.o(0x0F);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $bcontains(a, b) ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(2);
        a.lget(2);
        a.i32c(2);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.o(0xA7);
            a.lget(1);
            a.call(F_UBITS);
            a.o(0xA7);
            a.call(F_SCONTAINS);
            a.call(F_BOXB);
            a.o(0x0F);
        });
        a.lget(2);
        a.i32c(4);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.i32c(0);
            a.lset(3);
            a.blk(0x02, 0x40);
            a.blk(0x03, 0x40);
            a.lget(3);
            a.lget(0);
            a.call(F_ALEN_RAW);
            a.raw(&[0x4E]);
            a.br_if(1);
            a.lget(0);
            a.lget(3);
            a.call(F_AGET_CELL);
            a.lget(1);
            a.call(F_VEQ);
            a.raw(&[0x50]);
            a.raw(&[0x45]);
            a.if_void(|a| {
                a.i64c(1);
                a.call(F_BOXB);
                a.o(0x0F);
            });
            a.lget(3);
            a.i32c(1);
            a.raw(&[0x6A]);
            a.lset(3);
            a.br(0);
            a.end();
            a.end();
            a.i64c(0);
            a.call(F_BOXB);
            a.o(0x0F);
        });
        a.o(0x00);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![VT::I32, VT::I32], a.0);
    }

    // ---- $biter(p) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.lset(1);
        a.lget(1);
        a.i32c(4);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(5);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_MKEYS);
            a.o(0x0F);
        });
        a.lget(1);
        a.i32c(2);
        a.raw(&[0x46]);
        a.if_void(|a| {
            a.lget(0);
            a.call(F_UBITS);
            a.o(0xA7);
            a.call(F_SCHARS);
            a.o(0x0F);
        });
        a.o(0x00);
        m.add_func(&[VT::I32], &[VT::I32], vec![VT::I32], a.0);
    }

    // ---- $bindex(a, k) -> i32 ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.i32c(5);
        a.raw(&[0x46]);
        a.blk(0x04, 0x40);
        a.lget(0);
        a.lget(1);
        a.call(F_MGET);
        a.o(0x0F);
        a.end();
        a.lget(0);
        a.lget(1);
        a.call(F_UBITS);
        a.call(F_AGET);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![], a.0);
    }

    // ---- $bindex_set(a, k, v) ----
    {
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.i32c(5);
        a.raw(&[0x46]);
        a.blk(0x04, 0x40);
        a.lget(0);
        a.lget(1);
        a.lget(2);
        a.call(F_MSET);
        a.o(0x0F); // return (void)
        a.end();
        a.lget(0);
        a.lget(1);
        a.call(F_UBITS);
        a.lget(2);
        a.call(F_ASET);
        m.add_func(&[VT::I32, VT::I32, VT::I32], &[], vec![], a.0);
    }

    // ---- $bassert / $bassert_eq / $fail ----
    {
        let msg = sc(m, "assert ব্যর্থ (assertion failed)");
        let mut a = Asm::new();
        a.lget(0);
        a.call(F_TAG);
        a.i32c(3);
        a.raw(&[0x46]);
        a.lget(0);
        a.call(F_UBITS);
        a.raw(&[0x50]);
        a.raw(&[0x45]);
        a.raw(&[0x71]);
        a.raw(&[0x45]);
        a.if_void(|a| {
            a.i32c(msg as i32);
            a.call(F_FAIL);
        });
        a.call(F_BOXU);
        m.add_func(&[VT::I32], &[VT::I32], vec![], a.0);
    }
    {
        let msg = sc(m, "assert_eq ব্যর্থ (equality assertion failed)");
        let mut a = Asm::new();
        a.lget(0);
        a.lget(1);
        a.call(F_VEQ);
        a.raw(&[0x50]);
        a.if_void(|a| {
            a.i32c(msg as i32);
            a.call(F_FAIL);
        });
        a.call(F_BOXU);
        m.add_func(&[VT::I32, VT::I32], &[VT::I32], vec![], a.0);
    }
    {
        let mut a = Asm::new();
        a.o(0x00); // unreachable — message প্যারামিটার শুধু তথ্যের জন্য
        m.add_func(&[VT::I32], &[], vec![], a.0);
    }
}

// ================= user code compiler =================

struct LoopCtx {
    break_d: usize,
    cont_d: usize,
}

struct WC<'m> {
    m: &'m mut Mod,
    code: Asm,
    scopes: Vec<std::collections::HashMap<String, (u32, bool)>>, // (local, raw_int)
    nparams: u32,
    locals: Vec<VT>,
    depth: usize,
    loops: Vec<LoopCtx>,
}

impl<'m> WC<'m> {
    fn declare(&mut self, name: &str, raw: bool) -> u32 {
        let l = self.nparams + self.locals.len() as u32;
        self.locals.push(if raw { VT::I64 } else { VT::I32 });
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.to_string(), (l, raw));
        l
    }

    fn resolve(&self, name: &str) -> Option<(u32, bool)> {
        for sc in self.scopes.iter().rev() {
            if let Some(&v) = sc.get(name) {
                return Some(v);
            }
        }
        None
    }

    fn enter(&mut self) {
        self.scopes.push(std::collections::HashMap::new());
    }
    fn exit(&mut self) {
        self.scopes.pop();
    }
    fn open(&mut self, op: u8, bt: u8) {
        self.code.blk(op, bt);
        self.depth += 1;
    }
    fn close(&mut self) {
        self.code.end();
        self.depth -= 1;
    }

    fn stmt(&mut self, st: &Stmt, ft: &std::collections::HashMap<String, u32>) -> Result<(), String> {
        match st {
            Stmt::Let(name, e) => {
                self.expr(e, ft)?;
                let l = self.declare(name, false);
                self.code.lset(l);
            }
            Stmt::Assign(name, e, _) => match self.resolve(name) {
                Some((l, raw)) => {
                    self.expr(e, ft)?;
                    if raw {
                        self.code.call(F_UBITS);
                    }
                    self.code.lset(l);
                }
                None => {
                    return Err(format!(
                        "অজানা ভ্যারিয়েবল '{}' (unknown variable)",
                        name
                    ))
                }
            },
            Stmt::IndexAssign(o, i, v, _) => {
                self.expr(o, ft)?;
                self.expr(i, ft)?;
                self.expr(v, ft)?;
                self.code.call(F_BINDEX_SET);
            }
            Stmt::If {
                cond,
                then_b,
                else_b,
            } => {
                self.expr(cond, ft)?;
                self.code.call(F_TRUTHY);
                self.open(0x04, 0x40);
                for s in then_b {
                    self.stmt(s, ft)?;
                }
                if let Some(eb) = else_b {
                    self.code.o(0x05);
                    for s in eb {
                        self.stmt(s, ft)?;
                    }
                }
                self.close();
            }
            Stmt::While { cond, body } => {
                self.open(0x02, 0x40);
                let d1 = self.depth;
                self.open(0x03, 0x40);
                let d2 = self.depth;
                self.loops.push(LoopCtx {
                    break_d: d1,
                    cont_d: d2,
                });
                self.expr(cond, ft)?;
                self.code.call(F_TRUTHY);
                self.code.raw(&[0x45]);
                self.code.br_if((self.depth - d1) as u32);
                for s in body {
                    self.stmt(s, ft)?;
                }
                self.code.br((self.depth - d2) as u32);
                self.close();
                self.close();
                self.loops.pop();
            }
            Stmt::For { var, iter, body } => {
                self.enter();
                match iter {
                    ForIter::Range(from, to) => {
                        let il = self.declare(var, true);
                        let el = self.nparams + self.locals.len() as u32;
                        self.locals.push(VT::I64);
                        self.expr(from, ft)?;
                        self.code.call(F_UBITS);
                        self.code.lset(il);
                        self.expr(to, ft)?;
                        self.code.call(F_UBITS);
                        self.code.lset(el);
                        self.open(0x02, 0x40);
                        let d1 = self.depth;
                        self.open(0x03, 0x40);
                        let d2 = self.depth;
                        self.open(0x02, 0x40); // C — continue anchor
                        let d3 = self.depth;
                        self.loops.push(LoopCtx {
                            break_d: d1,
                            cont_d: d3,
                        });
                        // cond: i < e
                        self.code.lget(il);
                        self.code.lget(el);
                        self.code.raw(&[0x53]); // i64.lt_s
                        self.code.raw(&[0x45]);
                        self.code.br_if((self.depth - d1) as u32); // d3-d1 = 2
                        // (var raw i64 local — body-তে Ident raw → box করে পড়ে)
                        for s in body {
                            self.stmt(s, ft)?;
                        }
                        self.close(); // C
                        // i++
                        self.code.lget(il);
                        self.code.i64c(1);
                        self.code.raw(&[0x7C]);
                        self.code.lset(il);
                        self.code.br((self.depth - d2) as u32); // 0
                        self.close(); // L
                        self.close(); // B
                        self.loops.pop();
                    }
                    ForIter::Coll(c) => {
                        let cl = self.nparams + self.locals.len() as u32;
                        self.locals.push(VT::I32);
                        self.expr(c, ft)?;
                        self.code.call(F_BITER);
                        self.code.lset(cl);
                        let xl = self.nparams + self.locals.len() as u32;
                        self.locals.push(VT::I64);
                        self.code.i64c(0);
                        self.code.lset(xl);
                        let vl = self.declare(var, false);
                        self.open(0x02, 0x40);
                        let d1 = self.depth;
                        self.open(0x03, 0x40);
                        let d2 = self.depth;
                        self.open(0x02, 0x40);
                        let d3 = self.depth;
                        self.loops.push(LoopCtx {
                            break_d: d1,
                            cont_d: d3,
                        });
                        // cond: idx < len(coll)
                        self.code.lget(xl);
                        self.code.o(0xA7); // wrap
                        self.code.lget(cl);
                        self.code.call(F_ALEN_RAW);
                        self.code.raw(&[0x48]); // i32.lt_s
                        self.code.raw(&[0x45]);
                        self.code.br_if((self.depth - d1) as u32);
                        // var = coll[idx]
                        self.code.lget(cl);
                        self.code.lget(xl);
                        self.code.call(F_AGET);
                        self.code.lset(vl);
                        for s in body {
                            self.stmt(s, ft)?;
                        }
                        self.close();
                        self.code.lget(xl);
                        self.code.i64c(1);
                        self.code.raw(&[0x7C]);
                        self.code.lset(xl);
                        self.code.br((self.depth - d2) as u32);
                        self.close();
                        self.close();
                        self.loops.pop();
                    }
                }
                self.exit();
            }
            Stmt::FnDecl { .. } => {
                return Err("ফাংশন শুধু top level-এ (functions only at top level)".into())
            }
            Stmt::Return(e) => {
                match e {
                    Some(x) => self.expr(x, ft)?,
                    None => {
                        self.code.call(F_BOXU);
                    }
                }
                self.code.o(0x0F);
            }
            Stmt::Break => match self.loops.last() {
                Some(l) => {
                    let d = (self.depth - l.break_d) as u32;
                    self.code.br(d);
                }
                None => return Err("'break' লুপের বাইরে ('break' outside loop)".into()),
            },
            Stmt::Continue => match self.loops.last() {
                Some(l) => {
                    let d = (self.depth - l.cont_d) as u32;
                    self.code.br(d);
                }
                None => return Err("'continue' লুপের বাইরে".into()),
            },
            Stmt::ExprStmt(e) => {
                self.expr(e, ft)?;
                self.code.o(0x1A); // drop
            }
        }
        Ok(())
    }

    fn expr(
        &mut self,
        e: &Expr,
        ft: &std::collections::HashMap<String, u32>,
    ) -> Result<(), String> {
        match e {
            Expr::Int(v) => {
                self.code.i64c(*v);
                self.code.call(F_BOXI);
            }
            Expr::Float(v) => {
                self.code.f64c(*v);
                self.code.call(F_BOXF);
            }
            Expr::Str(s) => {
                let a = self.m.str_const(s);
                self.code.i32c(a as i32);
                self.code.call(F_SBOX);
            }
            Expr::Bool(v) => {
                self.code.i64c(if *v { 1 } else { 0 });
                self.code.call(F_BOXB);
            }
            Expr::Ident(name) => match self.resolve(name) {
                Some((l, raw)) => {
                    self.code.lget(l);
                    if raw {
                        self.code.call(F_BOXI);
                    }
                }
                None => {
                    return Err(format!(
                        "অজানা ভ্যারিয়েবল '{}' (unknown variable)",
                        name
                    ))
                }
            },
            Expr::Unary(op, x) => {
                self.expr(x, ft)?;
                if op == "-" {
                    self.code.call(F_NEG);
                } else {
                    self.code.call(F_NOT);
                }
            }
            Expr::Binary(op, l, r) => {
                if op == "&&" {
                    self.expr(l, ft)?;
                    // temp local
                    let t = self.nparams + self.locals.len() as u32;
                    self.locals.push(VT::I32);
                    self.code.lset(t);
                    self.code.lget(t);
                    self.code.call(F_TRUTHY);
                    self.open(0x04, 0x7F);
                    self.expr(r, ft)?;
                    self.code.o(0x05);
                    self.code.lget(t);
                    self.close();
                } else if op == "||" {
                    self.expr(l, ft)?;
                    let t = self.nparams + self.locals.len() as u32;
                    self.locals.push(VT::I32);
                    self.code.lset(t);
                    self.code.lget(t);
                    self.code.call(F_TRUTHY);
                    self.open(0x04, 0x7F);
                    self.code.lget(t);
                    self.code.o(0x05);
                    self.expr(r, ft)?;
                    self.close();
                } else {
                    self.expr(l, ft)?;
                    self.expr(r, ft)?;
                    let f = match op.as_str() {
                        "+" => F_ADD,
                        "-" => F_SUB,
                        "*" => F_MUL,
                        "/" => F_DIV,
                        "%" => F_MOD,
                        "==" => return self.emit_veq(),
                        "!=" => {
                            self.emit_veq()?;
                            self.code.call(F_NOT);
                            return Ok(());
                        }
                        "<" => F_LT,
                        ">" => F_GT,
                        "<=" => F_LE,
                        _ => F_GE,
                    };
                    self.code.call(f);
                }
            }
            Expr::Call(name, args, _) => {
                if let Some(&ui) = ft.get(name) {
                    for a in args {
                        self.expr(a, ft)?;
                    }
                    self.code.call(N_HELPERS + ui);
                } else if let Some(b) = crate::keywords::lookup_builtin(name) {
                    self.emit_builtin(b, args, ft)?;
                } else {
                    return Err(format!(
                        "অজানা ফাংশন '{}' (unknown function) — wasm backend",
                        name
                    ));
                }
            }
            Expr::Array(items) => {
                self.code.call(F_ARRNEW);
                let t = self.nparams + self.locals.len() as u32;
                self.locals.push(VT::I32);
                self.code.lset(t);
                for it in items {
                    self.code.lget(t);
                    self.expr(it, ft)?;
                    self.code.call(F_APUSH);
                }
                self.code.lget(t);
            }
            Expr::MapLit(pairs) => {
                self.code.call(F_MAPNEW);
                let t = self.nparams + self.locals.len() as u32;
                self.locals.push(VT::I32);
                self.code.lset(t);
                for (k, v) in pairs {
                    self.code.lget(t);
                    self.expr(k, ft)?;
                    self.expr(v, ft)?;
                    self.code.call(F_MSET);
                }
                self.code.lget(t);
            }
            Expr::Index(o, i, _) => {
                self.expr(o, ft)?;
                self.expr(i, ft)?;
                self.code.call(F_BINDEX);
            }
            Expr::Method(recv, name, args, _) => {
                let b = match crate::keywords::method_builtin(name) {
                    Some((b, _)) => b,
                    None => {
                        return Err(format!(
                            "অজানা method '{}' (unknown method) — wasm backend",
                            name
                        ))
                    }
                };
                // wasm-এ সমর্থিত নয় এমন method আগেই বলে দিই
                match b {
                    crate::keywords::Builtin::Upper
                    | crate::keywords::Builtin::Lower
                    | crate::keywords::Builtin::Trim
                    | crate::keywords::Builtin::Split
                    | crate::keywords::Builtin::Replace
                    | crate::keywords::Builtin::Join
                    | crate::keywords::Builtin::Repeat
                    | crate::keywords::Builtin::Sub => {
                        return Err(format!(
                            "wasm backend v0.2: '.{}()' সমর্থিত নয় — VM (vortexc run) ব্যবহার করুন",
                            name
                        ))
                    }
                    _ => {}
                }
                self.expr(recv, ft)?;
                for a in args {
                    self.expr(a, ft)?;
                }
                self.emit_builtin(b, &[], ft)?; // args আগেই push হয়ে গেছে
            }
        }
        Ok(())
    }

    fn emit_veq(&mut self) -> Result<(), String> {
        self.code.call(F_VEQ);
        self.code.call(F_BOXB);
        Ok(())
    }

    /// built-in কল — args হয় এখনো emit করতে হবে, নয় স্ট্যাকে আছে
    fn emit_builtin(
        &mut self,
        b: crate::keywords::Builtin,
        args: &[Expr],
        ft: &std::collections::HashMap<String, u32>,
    ) -> Result<(), String> {
        for a in args {
            self.expr(a, ft)?;
        }
        let f: u32 = match b {
            Builtin::Show => {
                self.code.call(F_TOSTR);
                self.code.call(F_OUT);
                let nl = self.m.str_const("\n");
                self.code.i32c(nl as i32);
                self.code.call(F_OUT);
                self.code.call(F_BOXU);
                return Ok(());
            }
            Builtin::Len => F_BLEN,
            Builtin::Str => {
                self.code.call(F_TOSTR);
                self.code.call(F_SBOX);
                return Ok(());
            }
            Builtin::Num => {
                self.code.call(F_UBITS);
                self.code.o(0xA7);
                self.code.call(F_S2I);
                self.code.call(F_BOXI);
                return Ok(());
            }
            Builtin::Push => {
                self.code.call(F_APUSH);
                self.code.call(F_BOXU);
                return Ok(());
            }
            Builtin::Pop => F_APOP,
            Builtin::Keys => F_MKEYS,
            Builtin::Values => F_MVALS,
            Builtin::Has => F_MHAS,
            Builtin::TypeOf => F_BTYPE,
            Builtin::Iter => F_BITER,
            Builtin::Floor => F_BFLOOR,
            Builtin::Ceil => F_BCEIL,
            Builtin::Abs => F_BABS,
            Builtin::Min => F_BMIN,
            Builtin::Max => F_BMAX,
            Builtin::Sqrt => F_BSQRT,
            Builtin::Pow => F_POW,
            Builtin::Contains => F_BCONTAINS,
            Builtin::Chars => F_SCHARS,
            Builtin::Assert => F_BASSERT,
            Builtin::AssertEq => F_BASSERT_EQ,
            other => {
                return Err(format!(
                    "wasm backend v0.2: '{}' built-in সমর্থিত নয় — VM ব্যবহার করুন",
                    other.name()
                ))
            }
        };
        self.code.call(f);
        Ok(())
    }
}

use crate::keywords::Builtin;

// ================= module emission =================

/// VORTEX source → WebAssembly binary
pub fn compile_to_wasm(src: &str) -> Result<Vec<u8>, String> {
    let program: Program = crate::parser::parse(src).map_err(|e| {
        format!(
            "parser ত্রুটি [লাইন {}, কলাম {}]: {}",
            e.line, e.col, e.msg
        )
    })?;

    let mut user: Vec<(&String, &Vec<String>, &Vec<Stmt>)> = Vec::new();
    for st in &program.stmts {
        if let Stmt::FnDecl { name, params, body } = st {
            if name == "main" {
                return Err("'main' সংরক্ষিত (reserved)".into());
            }
            user.push((name, params, body));
        }
    }

    let mut m = Mod::new();
    build_helpers(&mut m);
    build_helpers2(&mut m);
    debug_assert_eq!(m.funcs.len() as u32, N_HELPERS);

    let mut ft: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (i, (n, _p, _)) in user.iter().enumerate() {
        ft.insert((*n).clone(), i as u32);
    }

    // user ফাংশন
    for (_name, params, body) in &user {
        let (code_bytes, locals) = {
            let mut wc = WC {
                code: Asm::new(),
                scopes: vec![std::collections::HashMap::new()],
                nparams: params.len() as u32,
                locals: Vec::new(),
                depth: 0,
                loops: Vec::new(),
                m: &mut m,
            };
            for (pi, p) in params.iter().enumerate() {
                // param রা local 0..arity-তে থাকে
                wc.scopes
                    .last_mut()
                    .unwrap()
                    .insert((*p).clone(), (pi as u32, false));
            }
            for st in *body {
                wc.stmt(st, &ft)?;
            }
            wc.code.call(F_BOXU);
            (wc.code.0, wc.locals)
        };
        let vt = vec![VT::I32; params.len()];
        m.add_func(&vt, &[VT::I32], locals, code_bytes);
    }

    // main (top-level)
    let (main_code, main_locals) = {
        let mut wc = WC {
            code: Asm::new(),
            scopes: vec![std::collections::HashMap::new()],
            nparams: 0,
            locals: Vec::new(),
            depth: 0,
            loops: Vec::new(),
            m: &mut m,
        };
        for st in &program.stmts {
            if let Stmt::FnDecl { .. } = st {
                continue;
            }
            wc.stmt(st, &ft)?;
        }
        wc.code.call(F_BOXU);
        (wc.code.0, wc.locals)
    };
    m.add_func(&[], &[VT::I32], main_locals, main_code);

    let main_idx = m.funcs.len() as u32 - 1;

    // export ফাংশন: vortex_main
    {
        let mut a = Asm::new();
        a.call(main_idx);
        m.add_func(&[], &[VT::I32], vec![], a.0);
    }
    // vortex_output
    {
        let mut a = Asm::new();
        a.gget(m.g_outl);
        a.call(F_SNEW);
        a.lset(0);
        a.lget(0);
        a.i32c(8);
        a.raw(&[0x6A]);
        a.gget(m.g_outp);
        a.gget(m.g_outl);
        a.call(F_MEMCPY);
        a.lget(0);
        m.add_func(&[], &[VT::I32], vec![VT::I32], a.0);
    }
    let vmain_idx = m.funcs.len() as u32 - 2;
    let vout_idx = m.funcs.len() as u32 - 1;

    // ---- বাইনারি এমিট ----
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // magic
    out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version

    // type section — entries নিজে-সীমাবদ্ধ (per-entry size prefix নেই!)
    {
        let mut p = Vec::new();
        uleb(&mut p, m.types.len() as u64);
        for (params, results) in &m.types {
            p.push(0x60);
            uleb(&mut p, params.len() as u64);
            for x in params {
                p.push(x.byte());
            }
            uleb(&mut p, results.len() as u64);
            for x in results {
                p.push(x.byte());
            }
        }
        section(&mut out, 1, &p);
    }

    // function section
    {
        let mut p = Vec::new();
        uleb(&mut p, m.funcs.len() as u64);
        for f in &m.funcs {
            uleb(&mut p, f.type_idx as u64);
        }
        section(&mut out, 3, &p);
    }

    // memory section — 1 page ন্যূনতম, $alloc grow করে
    {
        let mut p = Vec::new();
        uleb(&mut p, 1);
        p.push(0x00);
        uleb(&mut p, 1);
        section(&mut out, 5, &p);
    }

    // global section: ALLOC, OUTP, OUTL, OUTC
    {
        let data_end = m.data.len() as u32;
        let alloc_init = (data_end + 7) & !7u32;
        let mut p = Vec::new();
        uleb(&mut p, 4);
        for v in [alloc_init, 0, 0, 0] {
            p.push(VT::I32.byte());
            p.push(0x01); // mutable
            p.push(0x41);
            sleb(&mut p, v as i64);
            p.push(0x0B);
        }
        section(&mut out, 6, &p);
    }

    // export section
    {
        let mut p = Vec::new();
        uleb(&mut p, 3);
        let exp = |p: &mut Vec<u8>, name: &str, kind: u8, idx: u32| {
            let nb = name.as_bytes();
            uleb(p, nb.len() as u64);
            p.extend_from_slice(nb);
            p.push(kind);
            uleb(p, idx as u64);
        };
        exp(&mut p, "memory", 2, 0);
        exp(&mut p, "vortex_main", 0, vmain_idx);
        exp(&mut p, "vortex_output", 0, vout_idx);
        section(&mut out, 7, &p);
    }

    // code section
    {
        let mut items: Vec<Vec<u8>> = Vec::new();
        for f in &m.funcs {
            let mut b = Vec::new();
            // local groups
            let mut groups: Vec<(u32, VT)> = Vec::new();
            for l in &f.locals {
                match groups.last_mut() {
                    Some((n, t)) if *t == *l => *n += 1,
                    _ => groups.push((1, *l)),
                }
            }
            uleb(&mut b, groups.len() as u64);
            for (n, t) in groups {
                uleb(&mut b, n as u64);
                b.push(t.byte());
            }
            b.extend_from_slice(&f.body);
            items.push(b);
        }
        let payload = vec_bytes(&items);
        section(&mut out, 10, &payload);
    }

    // data section
    {
        let mut p = Vec::new();
        uleb(&mut p, 1);
        p.push(0x00); // memory 0
        p.push(0x41);
        sleb(&mut p, 0);
        p.push(0x0B);
        uleb(&mut p, m.data.len() as u64);
        p.extend_from_slice(&m.data);
        section(&mut out, 11, &p);
    }

    Ok(out)
}

// ================= টেস্ট =================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uleb_roundtrip() {
        for v in [0u64, 1, 127, 128, 300, 624485, u32::MAX as u64] {
            let mut b = Vec::new();
            uleb(&mut b, v);
            // decode
            let mut x: u64 = 0;
            let mut shift = 0;
            for (i, byte) in b.iter().enumerate() {
                x |= ((byte & 0x7F) as u64) << shift;
                if byte & 0x80 == 0 {
                    assert_eq!(x, v, "pos {}", i);
                    break;
                }
                shift += 7;
            }
        }
    }

    #[test]
    fn sleb_roundtrip() {
        for v in [0i64, -1, 1, 63, -64, 64, -65, i32::MIN as i64, i32::MAX as i64, i64::MIN, i64::MAX] {
            let mut b = Vec::new();
            sleb(&mut b, v);
            let mut x: i64 = 0;
            let mut shift = 0;
            for byte in b.iter() {
                x |= ((byte & 0x7F) as i64) << shift;
                shift += 7;
                if byte & 0x80 == 0 {
                    if shift < 64 && byte & 0x40 != 0 {
                        x |= -1i64 << shift;
                    }
                    break;
                }
            }
            assert_eq!(x, v);
        }
    }

    #[test]
    fn compiles_simple_program() {
        let wasm = compile_to_wasm("show(\"নমস্কাৰ\");").unwrap();
        assert_eq!(&wasm[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        assert_eq!(&wasm[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn compiles_full_language() {
        let src = r#"
            let a = [1, 2, 3];
            let m = {"নাম": "ৰাম"};
            for i in 0..5 {
                if i % 2 == 0 {
                    show(i);
                } else {
                    continue;
                }
            }
            while true {
                break;
            }
            for x in a {
                push(a, x * 2);
                break;
            }
            show(m["নাম"]);
            show(len(a) + 1.5);
        "#;
        let wasm = compile_to_wasm(src).unwrap();
        assert_eq!(&wasm[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    }

    #[test]
    fn rejects_unsupported_method() {
        let r = compile_to_wasm("let s = \"abc\"; show(s.upper());");
        assert!(r.is_err());
    }

    /// নিজস্ব structural validator: সব section সঠিক ক্রমে আছে কি না
    #[test]
    fn binary_sections_valid() {
        let wasm = compile_to_wasm(
            "fn add(x, y) { return x + y; } show(add(2, 3)); let a = [1,2]; show(a[0]);",
        )
        .unwrap();
        let mut i = 8;
        let mut last_id = 0;
        let mut code_count: Option<u64> = None;
        while i < wasm.len() {
            let id = wasm[i];
            i += 1;
            let mut size: u64 = 0;
            let mut shift = 0;
            loop {
                let b = wasm[i];
                i += 1;
                size |= ((b & 0x7F) as u64) << shift;
                shift += 7;
                if b & 0x80 == 0 {
                    break;
                }
            }
            eprintln!("DBG section id={} size={} at {}", id, size, i);
            assert!(i + size as usize <= wasm.len(), "section {} overflow", id);
            assert!(id > last_id, "section order: {} after {}", id, last_id);
            last_id = id;
            if id == 10 {
                // code section: function count পড়ি (i পরে restore — নিচে size দিয়ে jump হবে)
                let start = i;
                let mut n: u64 = 0;
                let mut s2 = 0;
                loop {
                    let b = wasm[i];
                    n |= ((b & 0x7F) as u64) << s2;
                    s2 += 7;
                    i += 1;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
                code_count = Some(n);
                i = start;
            }
            i += size as usize;
        }
        assert!(last_id >= 10, "code+data সেকশন থাকা চাই");
        let n = code_count.expect("code section");
        // 70 helper + 1 user + 1 main + 2 export = 74
        assert_eq!(n, 74, "ফাংশন সংখ্যা");
    }
}
