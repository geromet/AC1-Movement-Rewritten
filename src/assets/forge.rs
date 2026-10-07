//! `.forge` archive reader (format version 25) — port of RE/tools/forge.py; format in RE/08.
//! Read-only: opens the user's own game files at runtime; nothing is copied into the project.

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 9] = b"scimitar\0";
const COMPRESSED_MAGIC: u64 = 0x1004_FA99_57FB_AA33;
const FILEDATA_HEADER_SIZE: u64 = 0x1B8;

/// Bound on how many forge block-link records one index may traverse. A visited set also
/// rejects cycles; this is the backstop for a well-formed but pathologically long chain.
const MAX_FORGE_BLOCKS: usize = 1 << 20;
/// Per-container decode budget: a malformed chunk table must not be able to declare more
/// decompressed output than this before the structure has been proven valid.
const MAX_CONTAINER_OUT: usize = 512 << 20;
/// Allocation budget for the standalone LZO entry point.
const MAX_LZO_OUT: usize = 512 << 20;

#[derive(Debug, Clone)]
pub struct ForgeEntry {
    pub index: usize,
    pub offset: u64,
    pub file_id: u32,
    pub size: u32,
    pub name: String,
}

/// One serialized engine object inside a stored file.
#[derive(Debug, Clone)]
pub struct Resource {
    pub id: u32,
    /// CRC32 of the C++ class name (e.g. 0x415D9568 = "Mesh").
    pub class_hash: u32,
    pub name: String,
    pub payload: Vec<u8>,
}

pub struct Forge {
    file: File,
    pub entries: Vec<ForgeEntry>,
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_u64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

impl Forge {
    pub fn open(path: &Path) -> io::Result<Forge> {
        let mut file = File::open(path)?;
        let mut hdr = [0u8; 0x1D];
        file.read_exact(&mut hdr)?;
        if &hdr[..9] != MAGIC {
            return Err(bad("not a forge file"));
        }
        let version = rd_u32(&hdr, 9);
        if version != 25 {
            return Err(bad(format!("unsupported forge version {version}")));
        }
        let fdh = rd_u64(&hdr, 0x0D);
        let mut forge = Forge { file, entries: Vec::new() };
        let h = forge.read_at(fdh, 0x28)?;
        let mut block = rd_u64(&h, 0x20);
        // A damaged/corrupt index can link block records into a cycle. Track every visited
        // offset so traversal always terminates; the old loop would spin forever on a cycle.
        let mut visited: HashSet<u64> = HashSet::new();
        while block != 0 && block != u64::MAX {
            if !visited.insert(block) {
                return Err(bad("cyclic forge block links"));
            }
            if visited.len() > MAX_FORGE_BLOCKS {
                return Err(bad("forge block chain exceeds traversal budget"));
            }
            let b = forge.read_at(block, 0x30)?;
            let count = rd_u32(&b, 0) as i32;
            let index_off = rd_u64(&b, 8);
            let next = rd_u64(&b, 0x10);
            let names_off = rd_u64(&b, 0x20);
            if count > 0 {
                // `count` is untrusted and read before any bound has been established: compute
                // the record-table sizes with checked arithmetic so a huge count cannot wrap.
                let n = count as usize;
                let idx_len = 16usize.checked_mul(n).ok_or_else(|| bad("forge index size overflow"))?;
                let names_len = 0xBCusize.checked_mul(n).ok_or_else(|| bad("forge name table size overflow"))?;
                let idx = forge.read_at(index_off, idx_len)?;
                let names = forge.read_at(names_off, names_len)?;
                for i in 0..n {
                    let nb = &names[0xBC * i..0xBC * (i + 1)];
                    let raw = &nb[0x2C..0xAC];
                    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                    forge.entries.push(ForgeEntry {
                        index: forge.entries.len(),
                        offset: rd_u64(&idx, 16 * i),
                        file_id: rd_u32(&idx, 16 * i + 8),
                        size: rd_u32(&idx, 16 * i + 12),
                        name: raw[..end].iter().map(|&c| c as char).collect(),
                    });
                }
            }
            block = next;
        }
        Ok(forge)
    }

    fn read_at(&mut self, off: u64, n: usize) -> io::Result<Vec<u8>> {
        // Prove the request is backed by real file bytes before allocating. Without this, a
        // declared-size field could reserve gigabytes of address space ahead of a read that
        // is guaranteed to fail (or, for `count as usize * stride`, to overflow).
        let len = self.file.metadata()?.len();
        let end = off.checked_add(n as u64).ok_or_else(|| bad("read offset overflow"))?;
        if end > len {
            return Err(bad(format!("read of {n} bytes at {off:#x} extends past the {len}-byte file")));
        }
        self.file.seek(SeekFrom::Start(off))?;
        let mut v = vec![0u8; n];
        self.file.read_exact(&mut v)?;
        Ok(v)
    }

    pub fn find(&self, name: &str) -> Option<&ForgeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Resource ids without inflating the resource container (RE/08 §4). Used to resolve
    /// shared map dependencies across stored files rather than guessing from resource names.
    pub fn resource_ids(&mut self, entry: &ForgeEntry) -> io::Result<Vec<u32>> {
        let start = entry.offset.checked_add(FILEDATA_HEADER_SIZE).ok_or_else(|| bad("resource offset overflow"))?;
        if rd_u64(&self.read_at(start, 8)?, 0) != COMPRESSED_MAGIC { return Ok(Vec::new()); }
        let (toc, _) = self.container(start)?;
        if toc.len() < 2 { return Err(bad("truncated resource TOC")); }
        let n = rd_u16(&toc, 0) as usize;
        if toc.len() != 2 + 8 * n { return Err(bad("invalid resource TOC length")); }
        Ok((0..n).map(|i| rd_u32(&toc, 2 + 8 * i)).collect())
    }

    /// Decode one compressed container (RE/08 §3); returns (bytes, end offset).
    fn container(&mut self, off: u64) -> io::Result<(Vec<u8>, u64)> {
        let h = self.read_at(off, 17)?;
        if rd_u64(&h, 0) != COMPRESSED_MAGIC {
            return Err(bad(format!("bad container magic at {off:#x}")));
        }
        let codec = h[10];
        if codec > 1 {
            return Err(bad(format!("unsupported codec {codec}")));
        }
        let n = rd_u16(&h, 15) as usize;
        let table_off = off.checked_add(17).ok_or_else(|| bad("container table offset overflow"))?;
        let table = self.read_at(table_off, 4 * n)?;
        let chunks: Vec<(usize, usize)> =
            (0..n).map(|i| (rd_u16(&table, 4 * i) as usize, rd_u16(&table, 4 * i + 2) as usize)).collect();
        // Prove the aggregate declared sizes with checked arithmetic and a budget before any
        // allocation: the old code summed raw u16 sizes straight into `with_capacity`.
        let (mut body_len, mut raw_total) = (0usize, 0usize);
        for (raw, packed) in &chunks {
            raw_total = raw_total.checked_add(*raw).ok_or_else(|| bad("container output size overflow"))?;
            let chunk = packed.checked_add(4).ok_or_else(|| bad("container chunk size overflow"))?;
            body_len = body_len.checked_add(chunk).ok_or_else(|| bad("container body size overflow"))?;
        }
        if raw_total > MAX_CONTAINER_OUT {
            return Err(bad(format!("container declares {raw_total} output bytes (budget {MAX_CONTAINER_OUT})")));
        }
        let pos = table_off.checked_add(4 * n as u64).ok_or_else(|| bad("container body offset overflow"))?;
        let body = self.read_at(pos, body_len)?;
        let mut out = Vec::with_capacity(raw_total);
        let mut q = 0;
        for (raw, packed) in chunks {
            let chk = rd_u32(&body, q);
            let blob = &body[q + 4..q + 4 + packed];
            q += 4 + packed;
            if adler32_init0(blob) != chk {
                return Err(bad("chunk checksum mismatch"));
            }
            if packed == raw {
                out.extend_from_slice(blob);
            } else {
                let d = lzo1x_decompress(blob, raw).map_err(bad)?;
                out.extend_from_slice(&d);
            }
        }
        Ok((out, pos + q as u64))
    }

    /// All resources of a stored file (empty for uncompressed records such as GlobalMetaFile).
    pub fn resources(&mut self, entry: &ForgeEntry) -> io::Result<Vec<Resource>> {
        let start = entry.offset.checked_add(FILEDATA_HEADER_SIZE).ok_or_else(|| bad("resource offset overflow"))?;
        if rd_u64(&self.read_at(start, 8)?, 0) != COMPRESSED_MAGIC {
            return Ok(Vec::new());
        }
        let (toc, next) = self.container(start)?;
        if toc.len() < 2 { return Err(bad("truncated resource TOC")); }
        let n = rd_u16(&toc, 0) as usize;
        if toc.len() != 2 + 8 * n { return Err(bad("invalid resource TOC length")); }
        let (data, _) = self.container(next)?;
        let mut out = Vec::with_capacity(n);
        let mut pos = 0usize;
        for i in 0..n {
            let id = rd_u32(&toc, 2 + 8 * i);
            let size = rd_u32(&toc, 2 + 8 * i + 4) as usize;
            if size < 12 { return Err(bad("resource record shorter than its header")); }
            let end = pos.checked_add(size).ok_or_else(|| bad("resource size overflow"))?;
            if end > data.len() { return Err(bad("resource extends past container data")); }
            let class_hash = rd_u32(&data, pos);
            let dsz = rd_u32(&data, pos + 4) as usize;
            let nl = rd_u32(&data, pos + 8) as usize;
            // Payload = last `dataSize` bytes, `hdr = size - dsz` is the optional sub-header.
            // Both a size/dsz underflow and a name that does not fit are corrupt-data errors.
            let hdr = size.checked_sub(dsz).ok_or_else(|| bad("resource size/dsz underflow"))?;
            if 12usize.checked_add(nl).map_or(true, |need| need > size) {
                return Err(bad("resource name exceeds record"));
            }
            let name = data[pos + 12..pos + 12 + nl].iter().map(|&c| c as char).collect();
            out.push(Resource { id, class_hash, name, payload: data[pos + hdr..end].to_vec() });
            pos = end;
        }
        if pos != data.len() {
            return Err(bad("TOC sizes do not match data"));
        }
        Ok(out)
    }
}

/// Adler-32 with initial value 0 (the container chunk checksum, RE/08 §3).
pub fn adler32_init0(data: &[u8]) -> u32 {
    let (mut a, mut b) = (0u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

/// Clean-room LZO1X decompressor (public bitstream format; the game uses lzo1x_decompress 0x9A0F40).
pub fn lzo1x_decompress(src: &[u8], out_len: usize) -> Result<Vec<u8>, String> {
    // Prove the declared output size before reserving address space for it.
    if out_len > MAX_LZO_OUT {
        return Err(format!("declared output {out_len} exceeds decode budget {MAX_LZO_OUT}"));
    }
    let mut out: Vec<u8> = Vec::with_capacity(out_len);
    let mut ip = 0usize;
    let get = |ip: &mut usize| -> Result<usize, String> {
        let b = *src.get(*ip).ok_or("input overrun")? as usize;
        *ip += 1;
        Ok(b)
    };
    let run_len = |t: usize, mask: usize, ip: &mut usize| -> Result<usize, String> {
        if t != 0 {
            return Ok(t);
        }
        let mut t = mask;
        while *src.get(*ip).ok_or("input overrun")? == 0 {
            t = t.checked_add(255).ok_or("run length overflow")?;
            *ip += 1;
        }
        t += *src.get(*ip).ok_or("input overrun")? as usize;
        *ip += 1;
        Ok(t)
    };
    // A malformed stream can encode a run far larger than `out_len`; reject it as soon as the
    // declared size is exceeded instead of growing `out` without bound first.
    let copy_lit = |out: &mut Vec<u8>, ip: &mut usize, n: usize| -> Result<(), String> {
        if out.len().checked_add(n).map_or(true, |end| end > out_len) {
            return Err(format!("literal run exceeds declared output {out_len}"));
        }
        let s = src.get(*ip..*ip + n).ok_or("literal overrun")?;
        out.extend_from_slice(s);
        *ip += n;
        Ok(())
    };
    let copy_match = |out: &mut Vec<u8>, dist: usize, n: usize| -> Result<(), String> {
        if out.len().checked_add(n).map_or(true, |end| end > out_len) {
            return Err(format!("match run exceeds declared output {out_len}"));
        }
        let start = out.len().checked_sub(dist).ok_or("lookbehind overrun")?;
        for k in 0..n {
            let b = out[start + k];
            out.push(b);
        }
        Ok(())
    };

    let mut state = 0usize;
    let mut t = get(&mut ip)?;
    if t > 17 {
        copy_lit(&mut out, &mut ip, t - 17)?;
        state = if t - 17 >= 4 { 4 } else { t - 17 };
        t = get(&mut ip)?;
    }
    loop {
        let (cnt, dist);
        if t >= 64 {
            cnt = (t >> 5) - 1 + 2;
            dist = ((t >> 2) & 7) + (get(&mut ip)? << 3) + 1;
        } else if t >= 32 {
            cnt = run_len(t & 31, 31, &mut ip)? + 2;
            let lo = get(&mut ip)?;
            let hi = get(&mut ip)?;
            dist = ((hi << 8 | lo) >> 2) + 1;
            t = lo;
        } else if t >= 16 {
            cnt = run_len(t & 7, 7, &mut ip)? + 2;
            let lo = get(&mut ip)?;
            let hi = get(&mut ip)?;
            let d = ((t & 8) << 11) + ((hi << 8 | lo) >> 2);
            if d == 0 {
                break;
            }
            dist = d + 16384;
            t = lo;
        } else if state == 0 {
            let n = run_len(t, 15, &mut ip)? + 3;
            copy_lit(&mut out, &mut ip, n)?;
            state = 4;
            t = get(&mut ip)?;
            continue;
        } else if state < 4 {
            cnt = 2;
            dist = (t >> 2) + (get(&mut ip)? << 2) + 1;
        } else {
            cnt = 3;
            dist = (t >> 2) + (get(&mut ip)? << 2) + 2049;
        }
        copy_match(&mut out, dist, cnt)?;
        let lit = t & 3;
        copy_lit(&mut out, &mut ip, lit)?;
        state = lit;
        t = get(&mut ip)?;
    }
    if out.len() != out_len {
        return Err(format!("size mismatch {} != {}", out.len(), out_len));
    }
    Ok(out)
}

/// CRC32 (zlib/IEEE) — the hash used for class and property names (RE/07 §4b).
pub fn crc32(s: &str) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in s.as_bytes() {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

#[cfg(test)]
mod checked_decode_tests {
    use super::*;

    fn temp_file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("ac_port_forge_{}_{tag}", std::process::id()));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// Encode one uncompressed single-chunk container (checksummed, `packed == raw`).
    fn container(payload: &[u8]) -> Vec<u8> {
        assert!(payload.len() <= u16::MAX as usize);
        let mut out = vec![0u8; 17];
        out[..8].copy_from_slice(&COMPRESSED_MAGIC.to_le_bytes());
        out[10] = 0; // codec 0
        out[15..17].copy_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes()); // raw size
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes()); // packed size
        out.extend_from_slice(&adler32_init0(payload).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn open_forge(bytes: &[u8], offset: u64) -> (Forge, ForgeEntry, std::path::PathBuf) {
        let path = temp_file("resources", bytes);
        let file = File::open(&path).unwrap();
        let entry = ForgeEntry { index: 0, offset, file_id: 1, size: 0, name: "synthetic".into() };
        (Forge { file, entries: vec![entry.clone()] }, entry, path)
    }

    #[test]
    fn cyclic_forge_index_terminates_with_error() {
        // A self-referential block link: the old loop never advanced past it.
        let mut f = vec![0u8; 0x300];
        f[..9].copy_from_slice(MAGIC);
        f[9..13].copy_from_slice(&25u32.to_le_bytes());
        let fdh = 0x100u64;
        f[13..21].copy_from_slice(&fdh.to_le_bytes());
        f[0x120..0x128].copy_from_slice(&0x200u64.to_le_bytes()); // block pointer at fdh + 0x20
        f[0x210..0x218].copy_from_slice(&0x200u64.to_le_bytes()); // block.next points back at the block
        let path = temp_file("cycle", &f);
        let err = match Forge::open(&path) {
            Ok(_) => panic!("a cyclic block link must be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("cyclic"), "{err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn truncated_resource_toc_is_rejected() {
        let mut toc = Vec::new();
        toc.extend_from_slice(&2u16.to_le_bytes()); // declares two records
        toc.extend_from_slice(&7u32.to_le_bytes());
        toc.extend_from_slice(&16u32.to_le_bytes()); // but only one is present
        let start = FILEDATA_HEADER_SIZE as usize;
        let toc_bytes = container(&toc);
        let data_bytes = container(&[0u8; 16]);
        let mut f = vec![0u8; start];
        f.extend_from_slice(&toc_bytes);
        f.extend_from_slice(&data_bytes);
        let (mut forge, entry, path) = open_forge(&f, 0);
        let err = forge.resources(&entry).unwrap_err();
        assert!(err.to_string().contains("TOC"), "{err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn container_size_dsz_underflow_is_rejected() {
        let mut toc = Vec::new();
        toc.extend_from_slice(&1u16.to_le_bytes());
        toc.extend_from_slice(&9u32.to_le_bytes());
        toc.extend_from_slice(&20u32.to_le_bytes()); // size
        let mut record = Vec::new();
        record.extend_from_slice(&0x1122_3344u32.to_le_bytes()); // class hash
        record.extend_from_slice(&100u32.to_le_bytes()); // dsz > size
        record.extend_from_slice(&0u32.to_le_bytes()); // name length
        record.resize(20, 0);
        let start = FILEDATA_HEADER_SIZE as usize;
        let toc_bytes = container(&toc);
        let data_bytes = container(&record);
        let mut f = vec![0u8; start];
        f.extend_from_slice(&toc_bytes);
        f.extend_from_slice(&data_bytes);
        let (mut forge, entry, path) = open_forge(&f, 0);
        let err = forge.resources(&entry).unwrap_err();
        assert!(err.to_string().contains("underflow"), "{err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn well_formed_resource_still_decodes() {
        let mut toc = Vec::new();
        toc.extend_from_slice(&1u16.to_le_bytes());
        toc.extend_from_slice(&7u32.to_le_bytes());
        toc.extend_from_slice(&16u32.to_le_bytes()); // size = 12 header + 4 payload
        let mut record = Vec::new();
        record.extend_from_slice(&0xAABB_CCDDu32.to_le_bytes());
        record.extend_from_slice(&4u32.to_le_bytes()); // dsz
        record.extend_from_slice(&0u32.to_le_bytes()); // name length
        record.extend_from_slice(&[1, 2, 3, 4]);
        let start = FILEDATA_HEADER_SIZE as usize;
        let toc_bytes = container(&toc);
        let data_bytes = container(&record);
        let mut f = vec![0u8; start];
        f.extend_from_slice(&toc_bytes);
        f.extend_from_slice(&data_bytes);
        let (mut forge, entry, path) = open_forge(&f, 0);
        let res = forge.resources(&entry).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, 7);
        assert_eq!(res[0].class_hash, 0xAABB_CCDD);
        assert_eq!(res[0].payload, vec![1, 2, 3, 4]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn lzo_beyond_declared_output_is_rejected() {
        // 0x12 = 18 > 17: a one-byte literal run, which already overruns a declared size of 0.
        let err = lzo1x_decompress(&[18, 0], 0).unwrap_err();
        assert!(err.contains("declared output"), "{err}");
        // A declared output beyond the allocation budget is rejected before reserving memory.
        assert!(lzo1x_decompress(&[], MAX_LZO_OUT + 1).is_err());
    }
}
