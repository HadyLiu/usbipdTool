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
