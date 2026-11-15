//! 从 .ico（PNG 条目）中挑选合适尺寸的 PNG 数据，用于窗口图标。

/// 返回 ico 中最接近 `want` 像素且不小于它的 PNG 条目（找不到则取最大）。
pub fn ico_png_entry(ico: &[u8], want: u32) -> Option<&[u8]> {
    if ico.len() < 6 || ico[0..4] != [0, 0, 1, 0] {
        return None;
    }
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let mut best: Option<(u32, &[u8])> = None;
    let mut largest: Option<(u32, &[u8])> = None;
    for i in 0..count {
        let e = ico.get(6 + 16 * i..22 + 16 * i)?;
        let w = if e[0] == 0 { 256 } else { e[0] as u32 };
        let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        let data = match ico.get(off..off + size) {
            Some(d) if d.starts_with(b"\x89PNG") => d,
            _ => continue,
        };
        if w >= want && best.is_none_or(|(bw, _)| w < bw) {
            best = Some((w, data));
        }
        if largest.is_none_or(|(lw, _)| w > lw) {
            largest = Some((w, data));
        }
    }
    best.or(largest).map(|(_, d)| d)
}

#[cfg(test)]
mod tests {
    use super::ico_png_entry;

    /// Minimal 1x1 ICO whose image payload is a PNG (not BMP).
    fn png_ico() -> Vec<u8> {
        const PNG: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
            0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
            0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
            0x42, 0x60, 0x82,
        ];
        let mut ico = vec![0, 0, 1, 0, 1, 0];
        let size = PNG.len() as u32;
        let offset = 6 + 16;
        ico.extend_from_slice(&[1, 1, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&size.to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        ico.extend_from_slice(PNG);
        ico
    }

    #[test]
    fn icon_png_entry_reads_embedded_png() {
        let ico = png_ico();
        let png = ico_png_entry(&ico, 64).expect("png entry");
        assert!(png.starts_with(b"\x89PNG"));
        assert_eq!(ico_png_entry(b"nope", 64), None);
    }
}
