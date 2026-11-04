//! vpm — VORTEX Package Manager (নিজস্ব, শূন্য dependency)
//!
//! - Manifest: `vortex.pkg` (নিজস্ব INI-সদৃশ ফরম্যাট, নিজস্ব parser)
//! - Archive: `.vpa` (Vortex Package Archive — নিজস্ব TLV বাইনারি ফরম্যাট,
//!   নিজস্ব CRC32 ইমপ্লিমেন্টেশন দিয়ে integrity check)
//! - Dependency resolution: topological sort + cycle detection
//!
//! কমান্ড: init, add, build, pack, install, list, tree

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------- Manifest ----------------

#[derive(Debug, Clone, Default)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: String,
    /// নাম → সংস্করণ
    pub deps: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct ManifestError {
    pub msg: String,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

/// নিজস্ব manifest parser — `[section]` + `key = value` লাইন-ভিত্তিক
pub fn parse_manifest(src: &str) -> Result<Manifest, ManifestError> {
    let mut m = Manifest::default();
    let mut section = String::new();
    let mut has_pkg = false;
    for (n, raw) in src.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => {
                return Err(ManifestError {
                    msg: format!(
                        "লাইন {}: '=' নেই (expected key = value): {}",
                        n + 1,
                        line
                    ),
                })
            }
        };
        match section.as_str() {
            "package" => match k {
                "name" => {
                    m.name = v.to_string();
                    has_pkg = true;
                }
                "version" => m.version = v.to_string(),
                "entry" => m.entry = v.to_string(),
                _ => {}
            },
            "deps" => m.deps.push((k.to_string(), v.to_string())),
            "" => {
                return Err(ManifestError {
                    msg: format!("লাইন {}: '[package]'-এর আগে key দেওয়া যায় না", n + 1),
                })
            }
            other => {
                return Err(ManifestError {
                    msg: format!("লাইন {}: অজানা section [{}]", n + 1, other),
                })
            }
        }
    }
    if !has_pkg || m.name.is_empty() {
        return Err(ManifestError {
            msg: "[package] section-এ 'name' দরকার (manifest needs a name)".into(),
        });
    }
    if m.version.is_empty() {
        m.version = "0.1.0".into();
    }
    if m.entry.is_empty() {
        m.entry = "src/main.vx".into();
    }
    Ok(m)
}

pub fn write_manifest(m: &Manifest) -> String {
    let mut s = String::from("[package]\n");
    s.push_str(&format!("name = {}\n", m.name));
    s.push_str(&format!("version = {}\n", m.version));
    s.push_str(&format!("entry = {}\n", m.entry));
    if !m.deps.is_empty() {
        s.push_str("\n[deps]\n");
        for (k, v) in &m.deps {
            s.push_str(&format!("{} = {}\n", k, v));
        }
    }
    s
}

pub fn read_manifest_file(path: &Path) -> Result<Manifest, String> {
    let src = fs::read_to_string(path)
        .map_err(|e| format!("manifest পড়া যায়নি (cannot read) {}: {}", path.display(), e))?;
    parse_manifest(&src).map_err(|e| e.msg)
}

// ---------------- CRC32 (নিজস্ব ইমপ্লিমেন্টেশন) ----------------

fn crc32_table() -> [u32; 256] {
    // IEEE 802.3 polynomial 0xEDB88320 — টেবিল runtime-এ তৈরি হয়
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[i as usize] = c;
        i += 1;
    }
    table
}

pub fn crc32(data: &[u8]) -> u32 {
    let table = crc32_table();
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

// ---------------- .vpa Archive (নিজস্ব TLV ফরম্যাট) ----------------
//
//   magic  : "VPA1" (4 bytes)
//   entry* : kind(1B: 0=manifest, 1=source) | name_len(2B LE) | name(utf8)
//          | data_len(4B LE) | data | crc32(4B LE, name+data-এর উপর)

pub const VPA_MAGIC: &[u8; 4] = b"VPA1";

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub data: Vec<u8>,
    pub is_manifest: bool,
}

pub fn pack(entries: &[ArchiveEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(VPA_MAGIC);
    for e in entries {
        out.push(if e.is_manifest { 0 } else { 1 });
        let name = e.name.as_bytes();
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&e.data);
        let mut crc_input = Vec::with_capacity(name.len() + e.data.len());
        crc_input.extend_from_slice(name);
        crc_input.extend_from_slice(&e.data);
        out.extend_from_slice(&crc32(&crc_input).to_le_bytes());
    }
    out
}

pub fn unpack(data: &[u8]) -> Result<Vec<ArchiveEntry>, String> {
    if data.len() < 4 || &data[0..4] != VPA_MAGIC {
        return Err("এটা VORTEX package নয় (not a .vpa archive — magic মেলেনি)".into());
    }
    let mut i = 4;
    let mut entries = Vec::new();
    while i < data.len() {
        if i + 7 > data.len() {
            return Err("ভাঙা archive (corrupt: truncated header)".into());
        }
        let kind = data[i];
        i += 1;
        let name_len = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
        i += 2;
        if i + name_len > data.len() {
            return Err("ভাঙা archive (corrupt: name)".into());
        }
        let name = String::from_utf8(data[i..i + name_len].to_vec())
            .map_err(|_| "ভাঙা archive (name utf8 নয়)".to_string())?;
        i += name_len;
        if i + 4 > data.len() {
            return Err("ভাঙা archive (corrupt: data_len)".into());
        }
        let data_len = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
            as usize;
        i += 4;
        if i + data_len > data.len() {
            return Err("ভাঙা archive (corrupt: data)".into());
        }
        let entry_data = data[i..i + data_len].to_vec();
        i += data_len;
        if i + 4 > data.len() {
            return Err("ভাঙা archive (corrupt: crc)".into());
        }
        let stored = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        i += 4;
        let mut crc_input = Vec::with_capacity(name_len + data_len);
        crc_input.extend_from_slice(name.as_bytes());
        crc_input.extend_from_slice(&entry_data);
        let actual = crc32(&crc_input);
        if actual != stored {
            return Err(format!(
                "CRC মেলেনি (corruption detected) '{}': সংরক্ষিত {:08x}, হিসাব {:08x}",
                name, stored, actual
            ));
        }
        entries.push(ArchiveEntry {
            name,
            data: entry_data,
            is_manifest: kind == 0,
        });
    }
    Ok(entries)
}

/// একটি package directory থেকে .vpa বানায় (manifest + সব .vx)
pub fn pack_dir(dir: &Path) -> Result<Vec<u8>, String> {
    let manifest_path = dir.join("vortex.pkg");
    let m = read_manifest_file(&manifest_path)?;
    let mut entries = vec![ArchiveEntry {
        name: "vortex.pkg".into(),
        data: write_manifest(&m).into_bytes(),
        is_manifest: true,
    }];
    let src_dir = dir.join("src");
    let mut vx_files: Vec<PathBuf> = Vec::new();
    collect_vx(&src_dir, &mut vx_files);
    for f in vx_files {
        let rel = f
            .strip_prefix(dir)
            .map_err(|_| "path ত্রুটি".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let data = fs::read(&f).map_err(|e| format!("পড়া যায়নি {}: {}", f.display(), e))?;
        entries.push(ArchiveEntry {
            name: rel,
            data,
            is_manifest: false,
        });
    }
    Ok(pack(&entries))
}

fn collect_vx(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        let mut items: Vec<_> = rd.filter_map(|e| e.ok()).collect();
        items.sort_by_key(|e| e.path());
        for e in items {
            let p = e.path();
            if p.is_dir() {
                collect_vx(&p, out);
            } else if p.extension().map_or(false, |x| x == "vx") {
                out.push(p);
            }
        }
    }
}

/// .vpa ইনস্টল করে deps ডিরেক্টরিতে
pub fn install(vpa_path: &Path, deps_dir: &Path) -> Result<String, String> {
    let data = fs::read(vpa_path)
        .map_err(|e| format!("পড়া যায়নি {}: {}", vpa_path.display(), e))?;
    let entries = unpack(&data)?;
    let manifest_entry = entries
        .iter()
        .find(|e| e.is_manifest)
        .ok_or_else(|| "archive-এ manifest নেই (no manifest in package)".to_string())?;
    let m = parse_manifest(
        std::str::from_utf8(&manifest_entry.data)
            .map_err(|_| "manifest utf8 নয়".to_string())?,
    )
    .map_err(|e| e.msg)?;
    let dest = deps_dir.join(&m.name);
    fs::create_dir_all(&dest).map_err(|e| format!("ডিরেক্টরি বানানো যায়নি: {}", e))?;
    for e in &entries {
        let p = dest.join(&e.name);
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&p, &e.data)
            .map_err(|e| format!("লেখা যায়নি {}: {}", p.display(), e))?;
    }
    Ok(m.name)
}

// ---------------- Dependency resolution ----------------

/// নাম → (সংস্করণ, নিজের deps) গ্রাফ থেকে topo order (deps আগে)
/// cycle থাকলে Err
pub fn resolve_order(
    graph: &HashMap<String, Vec<String>>,
) -> Result<Vec<String>, String> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        None,
        Temp,
        Done,
    }
    let mut order = Vec::new();
    let mut mark: HashMap<String, Mark> =
        graph.keys().map(|k| (k.clone(), Mark::None)).collect();

    fn visit(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        mark: &mut HashMap<String, Mark>,
        order: &mut Vec<String>,
        stack: &mut Vec<String>,
    ) -> Result<(), String> {
        match mark.get(node).copied().unwrap_or(Mark::None) {
            Mark::Done => return Ok(()),
            Mark::Temp => {
                return Err(format!(
                    "নির্ভরতায় লুপ (dependency cycle): {} → {}",
                    stack.join(" → "),
                    node
                ))
            }
            Mark::None => {}
        }
        mark.insert(node.to_string(), Mark::Temp);
        stack.push(node.to_string());
        if let Some(deps) = graph.get(node) {
            for d in deps {
                visit(d, graph, mark, order, stack)?;
            }
        }
        stack.pop();
        mark.insert(node.to_string(), Mark::Done);
        order.push(node.to_string());
        Ok(())
    }

    let mut stack = Vec::new();
    // স্থিতিশীল আউটপুটের জন্য নাম অনুযায়ী সাজানো ক্রমে ভিজিট
    let mut names: Vec<&String> = graph.keys().collect();
    names.sort();
    for n in names {
        visit(n, graph, &mut mark, &mut order, &mut stack)?;
    }
    Ok(order)
}

// ---------------- কমান্ড ইমপ্লিমেন্টেশন ----------------

pub fn cmd_init(dir: &Path, name: &str) -> Result<String, String> {
    let manifest_path = dir.join("vortex.pkg");
    if manifest_path.exists() {
        return Err(format!(
            "'{}' ইতিমধ্যে আছে (already exists) — আগের package আছে কি না দেখুন",
            manifest_path.display()
        ));
    }
    let m = Manifest {
        name: name.to_string(),
        version: "0.1.0".into(),
        entry: "src/main.vx".into(),
        deps: Vec::new(),
    };
    fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, write_manifest(&m)).map_err(|e| e.to_string())?;
    let hello = format!("// {} — VORTEX package\n\nfn মূল_বার্তা() {{\n    return \"{} স্বাগতম!\";\n}}\n\nshow(মূল_বার্তা());\n", name, name);
    fs::write(dir.join("src/main.vx"), hello).map_err(|e| e.to_string())?;
    Ok(manifest_path.display().to_string())
}

pub fn cmd_build(dir: &Path) -> Result<String, String> {
    let m = read_manifest_file(&dir.join("vortex.pkg"))?;
    let src_path = dir.join(&m.entry);
    let src = fs::read_to_string(&src_path)
        .map_err(|e| format!("entry পড়া যায়নি {}: {}", src_path.display(), e))?;
    let (_, vir_text) = crate::compile_source(&src).map_err(|e| e.to_string())?;
    let out = dir.join(&m.name).with_extension("vxc");
    fs::write(&out, &vir_text).map_err(|e| e.to_string())?;
    Ok(out.display().to_string())
}

pub fn cmd_pack(dir: &Path) -> Result<String, String> {
    let data = pack_dir(dir)?;
    let m = read_manifest_file(&dir.join("vortex.pkg"))?;
    let out = dir.join(format!("{}-{}.vpa", m.name, m.version));
    fs::write(&out, &data).map_err(|e| e.to_string())?;
    Ok(out.display().to_string())
}

pub fn cmd_list(deps_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(deps_dir) {
        let mut names: Vec<_> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        names.sort();
        for d in names {
            let mf = d.join("vortex.pkg");
            let label = match read_manifest_file(&mf) {
                Ok(m) => format!("{} {}", m.name, m.version),
                Err(_) => d
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            out.push(label);
        }
    }
    out
}

/// নির্ভরতা গ্রাফ পড়ে topo order দেয় — `tree` কমান্ডের ভিত্তি
pub fn build_graph(deps_dir: &Path) -> Result<HashMap<String, Vec<String>>, String> {
    let mut graph = HashMap::new();
    if let Ok(rd) = fs::read_dir(deps_dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                let mf = p.join("vortex.pkg");
                if let Ok(m) = read_manifest_file(&mf) {
                    graph.insert(
                        m.name.clone(),
                        m.deps.iter().map(|(d, _)| d.clone()).collect(),
                    );
                }
            }
        }
    }
    Ok(graph)
}
