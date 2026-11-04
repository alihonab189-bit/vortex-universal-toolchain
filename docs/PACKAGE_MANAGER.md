# vpm — VORTEX Package Manager

> ১০০% নিজস্ব — নিজস্ব manifest ফরম্যাট, নিজস্ব `.vpa` archive (নিজস্ব TLV
> binary + নিজস্ব CRC32), নিজস্ব dependency resolver। কোনো tar/gzip/serde নেই।

## Manifest: `vortex.pkg`

```ini
[package]
name = mathutils
version = 0.1.0
entry = src/main.vx

[deps]
strings = 0.1.0
```

নিজস্ব parser (`src/pkg.rs`) — `[section]` + `key = value`, `#` comment।

## Archive: `.vpa` (Vortex Package Archive)

নিজস্ব TLV binary:

```
magic "VPA1"
entry*: kind(1B: 0=manifest, 1=source) | name_len(2B LE) | name(utf8)
      | data_len(4B LE) | data | crc32(name+data, 4B LE)
```

CRC32 (IEEE 0xEDB88320) নিজেই implement করা — runtime-এ টেবিল তৈরি হয়।
ভাঙা ফাইল / বদলানো ফাইল ধরা পড়ে (`v02_pkg_archive_corruption_detected` টেস্ট)।

## কমান্ড

```bash
vortexc pkg init mypkg        # vortex.pkg + src/main.vx বুটস্ট্র্যাপ
vortexc pkg build             # entry → <name>.vxc (নিজস্ব VIR artifact)
vortexc pkg pack              # পুরো package → <name>-<version>.vpa
vortexc pkg install x.vpa     # → .vortex/deps/<name>/ (CRC যাচাই সহ)
vortexc pkg list              # ইনস্টল করা packages
vortexc pkg tree              # dependency topo-order (cycle হলে ভুল)
```

## Dependency resolution

DFS + রঙ (white/grey/black) দিয়ে topological sort; grey-তে পুনরাবৃত্তি
মানেই **dependency cycle** — সুন্দর ভুলবার্তা সহ থামে। টেস্টে প্রমাণিত:
`v02_pkg_resolve_order_and_cycle`।

## নীতি

- এই সংস্করণে registry সম্পূর্ণ **local** (নেটওয়ার্ক নেই — শূন্য
  dependency-র প্রতিশ্রুতি)। নিজস্ব registry server হলো roadmap-এর
  ধাপ (Category #12-এর পরিকল্পনা অনুযায়ী signing সহ)।
- Archive format, manifest format, CRC — সব নিজস্ব, নিজস্ব ডকুমেন্টেশন সহ।
