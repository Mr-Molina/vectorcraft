//! Device-independent bitmaps (DIBs), the pixels inside image records: written bottom-up as 24-bit
//! (opaque, composited over white) or 32-bit premultiplied BGRA (AlphaBlend's source); read from 1,
//! 4, 8, 16, 24 and 32-bit uncompressed and bit-field bitmaps and from embedded JPEG and PNG.

use std::io::Cursor;

use crate::bytes::Reader;

/// Most pixels along a side of an image the writers store (larger ones are scaled down).
const MAX_SIDE: u32 = 8192;
/// Most pixels an image the writers store holds.
const MAX_PIXELS: u64 = 1 << 24;
/// Most pixels along a side, and in all, of a bitmap the reader decodes.
const MAX_READ_SIDE: u32 = 1 << 15;
const MAX_READ_PIXELS: u64 = 1 << 26;

const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;
const BI_JPEG: u32 = 4;
const BI_PNG: u32 = 5;

/// Pixels with straight (not premultiplied) alpha, row by row from the top.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rgba {
    /// `img`, scaled down to what a metafile stores when it is larger.
    pub fn fitted(img: image::RgbaImage) -> Self {
        let (w, h) = img.dimensions();
        let side = f64::from(w.max(h)) / f64::from(MAX_SIDE);
        let area = (u64::from(w) * u64::from(h)) as f64 / MAX_PIXELS as f64;
        let k = side.max(area.sqrt());
        let img = if k > 1.0 {
            let nw = ((f64::from(w) / k).floor() as u32).max(1);
            let nh = ((f64::from(h) / k).floor() as u32).max(1);
            image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let (width, height) = img.dimensions();
        Self { width, height, pixels: img.into_raw() }
    }

    /// Every pixel fully opaque?
    pub fn opaque(&self) -> bool {
        self.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255)
    }

    /// As PNG bytes.
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let img = image::RgbaImage::from_raw(self.width, self.height, self.pixels.clone()).ok_or("the image's pixels don't match its size")?;
        let mut png = vec![];
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(png)
    }
}

/// A BITMAPINFOHEADER of a bottom-up, uncompressed `width` × `height` bitmap of `bits` per pixel.
fn header(width: u32, height: u32, bits: u16, size: usize) -> Vec<u8> {
    let mut h = crate::bytes::Out::default();
    h.u32(40);
    h.i32(i32::try_from(width).unwrap_or(i32::MAX));
    h.i32(i32::try_from(height).unwrap_or(i32::MAX));
    h.u16(1);
    h.u16(bits);
    h.u32(BI_RGB);
    h.u32(u32::try_from(size).unwrap_or(0));
    // 2835 pixels a metre: 72 ppi (the size on the page comes from the record).
    h.i32(2835);
    h.i32(2835);
    h.u32(0);
    h.u32(0);
    h.0
}

/// The bytes of a row of `width` pixels of `bits` each, padded to 4 bytes.
fn stride(width: u32, bits: u32) -> Option<usize> {
    let row_bits = u64::from(width).checked_mul(u64::from(bits))?;
    usize::try_from(row_bits.div_ceil(32) * 4).ok()
}

/// `img` at `opacity`, composited over white, as a 24-bit DIB → (header, bits).
pub(crate) fn rgb24(img: &Rgba, opacity: f32) -> (Vec<u8>, Vec<u8>) {
    let row = stride(img.width, 24).unwrap_or(0);
    let total = row.checked_mul(img.height as usize).unwrap_or(0);
    let mut bits = vec![0u8; total];
    let over = |c: u8, a: u32| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
    let row_src = (img.width as usize).checked_mul(4).unwrap_or(0).max(1);
    for (y, src) in img.pixels.chunks_exact(row_src).enumerate() {
        // Bottom-up: the first stored row is the image's last.
        let Some(at) = (img.height as usize).checked_sub(y.saturating_add(1)).and_then(|r| r.checked_mul(row)) else { continue };
        let Some(row_bytes) = (img.width as usize).checked_mul(3) else { continue };
        let Some(end) = at.checked_add(row_bytes) else { continue };
        let Some(dst) = bits.get_mut(at..end) else { continue };
        for (d, p) in dst.as_chunks_mut::<3>().0.iter_mut().zip(src.as_chunks::<4>().0) {
            let a = (f32::from(p.get(3).copied().unwrap_or(255)) * opacity.clamp(0.0, 1.0)).round() as u32;
            let b = over(p.first().copied().unwrap_or(0), a);
            let g = over(p.get(1).copied().unwrap_or(0), a);
            let r = over(p.get(2).copied().unwrap_or(0), a);
            d.copy_from_slice(&[r, g, b]);
        }
    }
    (header(img.width, img.height, 24, bits.len()), bits)
}

/// `img` at `opacity` as a 32-bit DIB of premultiplied BGRA → (header, bits).
pub(crate) fn bgra32(img: &Rgba, opacity: f32) -> (Vec<u8>, Vec<u8>) {
    let row = (img.width as usize).checked_mul(4).unwrap_or(0);
    let total = row.checked_mul(img.height as usize).unwrap_or(0);
    let mut bits = vec![0u8; total];
    for (y, src) in img.pixels.chunks_exact(row.max(1)).enumerate() {
        let Some(at) = (img.height as usize).checked_sub(y.saturating_add(1)).and_then(|r| r.checked_mul(row)) else { continue };
        let Some(end) = at.checked_add(row) else { continue };
        let Some(dst) = bits.get_mut(at..end) else { continue };
        for (d, p) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
            let a = (f32::from(p.get(3).copied().unwrap_or(255)) * opacity.clamp(0.0, 1.0)).round() as u32;
            let pre = |c: u8| ((u32::from(c) * a + 127) / 255) as u8;
            d.copy_from_slice(&[
                pre(p.get(2).copied().unwrap_or(0)),
                pre(p.get(1).copied().unwrap_or(0)),
                pre(p.first().copied().unwrap_or(0)),
                a as u8,
            ]);
        }
    }
    (header(img.width, img.height, 32, bits.len()), bits)
}

/// How the fourth byte of 32-bit pixels reads.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Alpha {
    /// Unused (StretchDIBits and BitBlt ignore it): every pixel opaque.
    Ignore,
    /// Premultiplied alpha (AlphaBlend with per-pixel alpha).
    Premultiplied,
}

/// A colour channel of `v` under `mask`, scaled to 0–255.
fn channel(v: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = u64::from(mask >> shift);
    let x = u64::from((v & mask) >> shift);
    ((x * 255 + max / 2) / max.max(1)).min(255) as u8
}

/// Read a DIB from its BITMAPINFO (`bmi`: the header and colour table) and pixel `bits`.
pub(crate) fn decode(bmi: &[u8], bits: &[u8], alpha: Alpha) -> Result<Rgba, String> {
    let bad = || "a bitmap is damaged".to_string();
    let mut r = Reader::new(bmi);
    let size = r.u32().ok_or_else(bad)?;
    let (width, height, bpp, compression, colors, palette_entry, masks_at) = if size == 12 {
        // BITMAPCOREHEADER: 16-bit sizes, RGB triples.
        let w = i32::from(r.u16().ok_or_else(bad)?);
        let h = i32::from(r.u16().ok_or_else(bad)?);
        r.skip(2).ok_or_else(bad)?;
        let bpp = r.u16().ok_or_else(bad)?;
        (w, h, bpp, BI_RGB, 0, 3, 0)
    } else if size >= 40 {
        let w = r.i32().ok_or_else(bad)?;
        let h = r.i32().ok_or_else(bad)?;
        r.skip(2).ok_or_else(bad)?;
        let bpp = r.u16().ok_or_else(bad)?;
        let c = r.u32().ok_or_else(bad)?;
        r.skip(12).ok_or_else(bad)?;
        let used = r.u32().ok_or_else(bad)?;
        (w, h, bpp, c, used, 4, 40)
    } else {
        return Err(bad());
    };
    if compression == BI_JPEG || compression == BI_PNG {
        let (w, h) = image::ImageReader::new(Cursor::new(bits)).with_guessed_format().ok().and_then(|r| r.into_dimensions().ok()).ok_or_else(bad)?;
        let pixels = (w as usize).checked_mul(h as usize).ok_or_else(|| "a bitmap is too large to read".to_string())?;
        if w > MAX_READ_SIDE || h > MAX_READ_SIDE || (pixels as u64) > MAX_READ_PIXELS {
            return Err("a bitmap is too large to read".into());
        }
        let img = image::load_from_memory(bits).map_err(|_| bad())?.to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = (w as usize).checked_mul(h as usize).ok_or_else(|| "a bitmap is too large to read".to_string())?;
        if w > MAX_READ_SIDE || h > MAX_READ_SIDE || (pixels as u64) > MAX_READ_PIXELS {
            return Err("a bitmap is too large to read".into());
        }
        return Ok(Rgba { width: w, height: h, pixels: img.into_raw() });
    }
    if compression != BI_RGB && compression != BI_BITFIELDS {
        return Err("compressed (RLE) bitmaps can't be read yet".into());
    }
    let top_down = height < 0;
    let (w, h) = (width.unsigned_abs(), height.unsigned_abs());
    if w == 0 || h == 0 {
        return Err("a bitmap is empty".into());
    }
    if w > MAX_READ_SIDE || h > MAX_READ_SIDE || u64::from(w) * u64::from(h) > MAX_READ_PIXELS {
        return Err("a bitmap is too large to read".into());
    }
    let header_len = size as usize;
    // Bit-field masks follow a plain 40-byte header, or sit inside the larger ones.
    let fields = compression == BI_BITFIELDS;
    let masks: Option<[u32; 4]> = fields.then(|| {
        let mut m = Reader::new(bmi.get(masks_at..).unwrap_or_default());
        let rgb = [m.u32().unwrap_or(0), m.u32().unwrap_or(0), m.u32().unwrap_or(0)];
        // An alpha mask comes with BITMAPV3INFOHEADER and later.
        let a = if header_len >= 56 { m.u32().unwrap_or(0) } else { 0 };
        [rgb[0], rgb[1], rgb[2], a]
    });
    let palette_at = header_len + if fields && header_len == 40 { 12 } else { 0 };
    let bpp_u = u32::from(bpp);
    let palette: Vec<[u8; 3]> = if bpp_u <= 8 {
        let n = if colors == 0 { 1usize << bpp_u } else { (colors as usize).min(256) };
        bmi.get(palette_at..)
            .unwrap_or_default()
            .chunks_exact(palette_entry)
            .take(n)
            .map(|e| [e.get(2).copied().unwrap_or(0), e.get(1).copied().unwrap_or(0), e.first().copied().unwrap_or(0)])
            .collect()
    } else {
        vec![]
    };
    if !matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32) {
        return Err(format!("{bpp}-bit bitmaps can't be read"));
    }
    let row = stride(w, bpp_u).ok_or_else(bad)?;
    if bits.len() < row.checked_mul(h as usize).ok_or_else(bad)? {
        return Err(bad());
    }
    let (mr, mg, mb, ma) = match (masks, bpp) {
        (Some([r, g, b, a]), _) => (r, g, b, a),
        (None, 16) => (0x7c00, 0x03e0, 0x001f, 0),
        _ => (0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000),
    };
    let cap = (w as usize).checked_mul(h as usize).and_then(|a| a.checked_mul(4)).unwrap_or(0);
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(cap).map_err(|_| bad())?;
    for y in 0..h as usize {
        let src_row = if top_down { y } else { (h as usize).checked_sub(1 + y).ok_or_else(bad)? };
        let row_start = src_row.checked_mul(row).ok_or_else(bad)?;
        let row_end = row_start.checked_add(row).ok_or_else(bad)?;
        let line = bits.get(row_start..row_end).ok_or_else(bad)?;
        for x in 0..w as usize {
            let px: [u8; 4] = match bpp {
                1 | 4 | 8 => {
                    let bit = x.saturating_mul(bpp_u as usize);
                    let byte = line.get(bit / 8).copied().unwrap_or(0);
                    let idx = (byte >> (8 - bpp_u as usize - bit % 8)) & ((1u16 << bpp_u) - 1) as u8;
                    let [r, g, b] = palette.get(idx as usize).copied().unwrap_or([0, 0, 0]);
                    [r, g, b, 255]
                }
                16 => {
                    let x2 = x.saturating_mul(2);
                    let v = u32::from(u16::from_le_bytes([line.get(x2).copied().unwrap_or(0), line.get(x2.saturating_add(1)).copied().unwrap_or(0)]));
                    [channel(v, mr), channel(v, mg), channel(v, mb), 255]
                }
                24 => {
                    let start = x.checked_mul(3).ok_or_else(bad)?;
                    let end = start.checked_add(3).ok_or_else(bad)?;
                    let p = line.get(start..end).unwrap_or(&[0, 0, 0]);
                    [p.get(2).copied().unwrap_or(0), p.get(1).copied().unwrap_or(0), p.first().copied().unwrap_or(0), 255]
                }
                _ => {
                    let start = x.checked_mul(4).ok_or_else(bad)?;
                    let end = start.checked_add(4).ok_or_else(bad)?;
                    let p = line.get(start..end).unwrap_or(&[0, 0, 0, 0]);
                    let v = u32::from_le_bytes([
                        p.first().copied().unwrap_or(0),
                        p.get(1).copied().unwrap_or(0),
                        p.get(2).copied().unwrap_or(0),
                        p.get(3).copied().unwrap_or(0),
                    ]);
                    let a = if alpha == Alpha::Premultiplied && ma != 0 { channel(v, ma) } else { 255 };
                    let un =
                        |c: u8| if alpha == Alpha::Premultiplied && a < 255 { (u32::from(c) * 255 / u32::from(a.max(1))).min(255) as u8 } else { c };
                    [un(channel(v, mr)), un(channel(v, mg)), un(channel(v, mb)), if alpha == Alpha::Premultiplied { a } else { 255 }]
                }
            };
            pixels.extend_from_slice(&px);
        }
    }
    Ok(Rgba { width: w, height: h, pixels })
}

/// A packed DIB (a BITMAPINFO and its bits, one after the other, as WMF records hold them) split
/// into the two.
pub(crate) fn split(packed: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut r = Reader::new(packed);
    let size = r.u32()? as usize;
    let (bpp, compression, used, entry) = if size == 12 {
        r.skip(6)?;
        (r.u16()?, BI_RGB, 0, 3)
    } else {
        r.skip(10)?;
        let bpp = r.u16()?;
        let c = r.u32()?;
        r.skip(12)?;
        (bpp, c, r.u32()? as usize, 4)
    };
    let masks = if compression == BI_BITFIELDS && size == 40 { 12 } else { 0 };
    let colors = if used > 0 {
        used.min(256)
    } else if bpp <= 8 {
        1usize << bpp
    } else {
        0
    };
    let len = size.checked_add(masks)?.checked_add(colors * entry)?;
    Some((packed.get(..len)?, packed.get(len..)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Rgba {
        // 3 × 2: red, green, blue / half-transparent black, transparent, white.
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 128, 0, 0, 0, 0, 255, 255, 255, 255];
        Rgba { width: 3, height: 2, pixels }
    }

    #[test]
    fn rgb24_is_bottom_up_bgr_over_white() {
        let (bmi, bits) = rgb24(&sample(), 1.0);
        assert_eq!(bmi.len(), 40);
        assert_eq!(&bmi[14..16], &[24, 0], "24 bits a pixel");
        // Rows of 9 bytes padded to 12, the last row first: half-transparent black over white is
        // mid grey, transparent is white.
        assert_eq!(bits, vec![127, 127, 127, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn bgra32_is_premultiplied() {
        let (bmi, bits) = bgra32(&sample(), 1.0);
        assert_eq!(&bmi[14..16], &[32, 0]);
        assert_eq!(&bits[..12], &[0, 0, 0, 128, 0, 0, 0, 0, 255, 255, 255, 255]);
        assert_eq!(&bits[12..16], &[0, 0, 255, 255], "red, as BGRA");
        // At half opacity.
        let (_, half) = bgra32(&sample(), 0.5);
        assert_eq!(&half[12..16], &[0, 0, 128, 128]);
    }

    #[test]
    fn rgb24_round_trips_over_white() {
        let (bmi, bits) = rgb24(&sample(), 1.0);
        assert_eq!(bmi.len(), 40);
        // Rows of 9 bytes padded to 12.
        assert_eq!(bits.len(), 24);
        let back = decode(&bmi, &bits, Alpha::Ignore).unwrap();
        assert_eq!((back.width, back.height), (3, 2));
        assert_eq!(&back.pixels[..12], &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
        // Half-transparent black over white is mid grey; transparent is white.
        assert_eq!(&back.pixels[12..16], &[127, 127, 127, 255]);
        assert_eq!(&back.pixels[16..20], &[255, 255, 255, 255]);
    }

    #[test]
    fn bgra32_round_trips_with_alpha() {
        let (bmi, bits) = bgra32(&sample(), 1.0);
        let back = decode(&bmi, &bits, Alpha::Premultiplied).unwrap();
        assert_eq!(&back.pixels[..12], &sample().pixels[..12]);
        assert_eq!(back.pixels[15], 128);
        assert_eq!(back.pixels[19], 0);
        // Read as StretchDIBits does, alpha is ignored.
        assert!(decode(&bmi, &bits, Alpha::Ignore).unwrap().opaque());
    }

    #[test]
    fn palette_bitmaps_and_damage() {
        // A 1-bit 2 × 1 bitmap: black then white.
        let mut bmi = header(2, 1, 1, 4);
        bmi.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
        let back = decode(&bmi, &[0b0100_0000, 0, 0, 0], Alpha::Ignore).unwrap();
        assert_eq!(back.pixels, vec![0, 0, 0, 255, 255, 255, 255, 255]);
        // Too few bits, a silly size, an unknown depth: errors, no panic.
        assert!(decode(&bmi, &[0], Alpha::Ignore).is_err());
        assert!(decode(&header(1 << 20, 1 << 20, 24, 0), &[], Alpha::Ignore).is_err());
        assert!(decode(&header(1, 1, 7, 0), &[0; 4], Alpha::Ignore).is_err());
        assert!(decode(&[1, 2], &[], Alpha::Ignore).is_err());
    }

    #[test]
    fn large_images_are_scaled_to_fit() {
        let img = image::RgbaImage::new(MAX_SIDE * 2, 4);
        let f = Rgba::fitted(img);
        assert_eq!((f.width, f.height), (MAX_SIDE, 2));
    }

    #[test]
    fn png_bitmap_decodes() {
        let rgba = Rgba { width: 1, height: 1, pixels: vec![255, 0, 0, 255] };
        let png = rgba.to_png().unwrap();
        let mut bmi = header(1, 1, 32, png.len());
        bmi[16..20].copy_from_slice(&BI_PNG.to_le_bytes());
        let back = decode(&bmi, &png, Alpha::Ignore).unwrap();
        assert_eq!((back.width, back.height), (1, 1));
        assert_eq!(back.pixels, vec![255, 0, 0, 255]);
    }
}
