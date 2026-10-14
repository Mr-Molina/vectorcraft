//! A store-only (uncompressed) ZIP archive: several exported files as one download. The files are
//! already compressed (PNG, JPEG, PDF…), so storing them keeps the archive fast to write and read.

use crate::EngineError;
use flate2::Crc;

/// The DOS date of every entry: 1980-01-01, the earliest a ZIP date can say (no clock on the web,
/// and a fixed date keeps archives of the same files identical).
const DOS_DATE: u16 = (1 << 5) | 1;
/// General purpose flag: file names are UTF-8.
const UTF8_NAMES: u16 = 1 << 11;
/// Version needed to extract: 2.0.
const VERSION: u16 = 20;

pub const MAX_ZIP_BYTES: usize = 512 * 1024 * 1024;

impl EngineError {
    #[allow(non_snake_case)]
    pub fn InvalidParams(msg: String) -> Self {
        EngineError::BadParams { cmd: "zip".into(), msg }
    }
    #[allow(non_snake_case)]
    pub fn Io(msg: String) -> Self {
        EngineError::Other(msg)
    }
}

pub fn validate_zip_entry_name(name: &str) -> Result<(), EngineError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains(':')
        || name.contains('\\')
        || name.starts_with('/')
        || name.split('/').any(|seg| seg == ".." || seg == ".")
    {
        return Err(EngineError::InvalidParams(format!("invalid zip entry name: {name}")));
    }
    Ok(())
}

/// The archive of `files` (`(name, bytes)`; `/` separates folders in a name), stored without
/// compression. Fails past the limits of a ZIP without its 64-bit extension (65535 files, 4 GB)
/// or if total uncompressed size exceeds [`MAX_ZIP_BYTES`].
pub fn zip_pack<N: AsRef<str>, B: AsRef<[u8]>>(files: &[(N, B)]) -> Result<Vec<u8>, EngineError> {
    let too_big = || EngineError::InvalidParams("archive exceeds maximum size".into());
    let count = u16::try_from(files.len()).map_err(|_| too_big())?;

    let mut total_cap: usize = 22; // EOCD record
    for (n, b) in files {
        validate_zip_entry_name(n.as_ref())?;
        let n_len = n.as_ref().len();
        let b_len = b.as_ref().len();
        let header_overhead = 76usize.checked_add(n_len.checked_mul(2).ok_or_else(too_big)?).ok_or_else(too_big)?;
        let entry_len = header_overhead.checked_add(b_len).ok_or_else(too_big)?;
        total_cap = total_cap.checked_add(entry_len).ok_or_else(too_big)?;
        if total_cap > MAX_ZIP_BYTES {
            return Err(too_big());
        }
    }

    let mut out = Vec::with_capacity(total_cap);
    let mut central = Vec::new();
    for (name, bytes) in files {
        let (name_str, bytes) = (name.as_ref(), bytes.as_ref());
        let name_bytes = name_str.as_bytes();
        let offset = u32::try_from(out.len()).map_err(|_| too_big())?;
        let size = u32::try_from(bytes.len()).map_err(|_| too_big())?;
        let name_len = u16::try_from(name_bytes.len()).map_err(|_| EngineError::InvalidParams(format!("file name too long to zip: {name_str}")))?;
        let mut crc = Crc::new();
        crc.update(bytes);
        // The fields both headers share: flags, method (0: stored), time, date, CRC, sizes, name length.
        let mut shared = Vec::with_capacity(26);
        for v in [UTF8_NAMES, 0, 0, DOS_DATE] {
            shared.extend_from_slice(&v.to_le_bytes());
        }
        for v in [crc.sum(), size, size] {
            shared.extend_from_slice(&v.to_le_bytes());
        }
        shared.extend_from_slice(&name_len.to_le_bytes());
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&shared);
        out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(bytes);
        // Central directory record.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&VERSION.to_le_bytes()); // made by
        central.extend_from_slice(&VERSION.to_le_bytes()); // needed to extract
        central.extend_from_slice(&shared);
        // Extra and comment lengths, disk number, internal and external attributes.
        central.extend_from_slice(&[0; 2 + 2 + 2 + 2 + 4]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let start = u32::try_from(out.len()).map_err(|_| too_big())?;
    let central_len = u32::try_from(central.len()).map_err(|_| too_big())?;
    out.extend_from_slice(&central);
    // End of central directory.
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]); // this disk, the directory's disk
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&central_len.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    Ok(out)
}

/// The archive of `files` (`(name, bytes)`; `/` separates folders in a name), stored without
/// compression. Fails past the limits of a ZIP without its 64-bit extension (65535 files, 4 GB).
pub fn store<N: AsRef<str>, B: AsRef<[u8]>>(files: &[(N, B)]) -> Result<Vec<u8>, String> {
    zip_pack(files).map_err(|e| e.to_string())
}

/// The entries of a store-only archive written by [`zip_pack`] (read through its central directory,
/// checking every CRC) → `(name, bytes)`.
#[allow(dead_code)]
pub fn zip_unpack(zip: &[u8]) -> Result<Vec<(String, Vec<u8>)>, EngineError> {
    let err = |msg: &str| EngineError::Other(msg.to_string());
    let u16_at = |at: usize| zip.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| err("truncated"));
    let u32_at = |at: usize| zip.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| err("truncated"));
    let end = zip.len().checked_sub(22).ok_or_else(|| err("no end record"))?;
    if u32_at(end)? != 0x0605_4b50 {
        return Err(err("no end record"));
    }
    let count = u16_at(end + 10)? as usize;
    let mut at = u32_at(end + 16)? as usize;
    let mut out = vec![];
    for _ in 0..count {
        if u32_at(at)? != 0x0201_4b50 || u16_at(at + 10)? != 0 {
            return Err(err("not a stored central record"));
        }
        let (crc, size, name_len) = (u32_at(at + 16)?, u32_at(at + 20)? as usize, u16_at(at + 28)? as usize);
        let extra = u16_at(at + 30)? as usize + u16_at(at + 32)? as usize;
        let local = u32_at(at + 42)? as usize;
        let name_bytes = zip.get(at + 46..at + 46 + name_len).ok_or_else(|| err("truncated"))?;
        let name = String::from_utf8(name_bytes.to_vec()).map_err(|e| EngineError::Other(e.to_string()))?;
        validate_zip_entry_name(&name)?;
        if u32_at(local)? != 0x0403_4b50 {
            return Err(EngineError::Other(format!("{name}: no local header")));
        }
        let data = local + 30 + u16_at(local + 26)? as usize + u16_at(local + 28)? as usize;
        let bytes = zip.get(data..data + size).ok_or_else(|| err("truncated"))?.to_vec();
        let mut c = Crc::new();
        c.update(&bytes);
        if c.sum() != crc {
            return Err(EngineError::Other(format!("{name}: CRC mismatch")));
        }
        out.push((name, bytes));
        at += 46 + name_len + extra;
    }
    Ok(out)
}

/// The entries of a store-only archive written by [`store`] (read through its central directory,
/// checking every CRC) → `(name, bytes)`.
#[allow(dead_code)]
pub fn entries(zip: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    zip_unpack(zip).map_err(|e| e.to_string())
}
