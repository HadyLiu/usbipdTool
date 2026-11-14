//! 命令输出解码：UTF-16（wsl.exe）/ UTF-8 / GBK / 系统 ANSI 代码页。

fn decode_utf16(data: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = data
        .chunks(2)
        .map(|c| {
            let a = c[0];
            let b = *c.get(1).unwrap_or(&0);
            if big_endian { u16::from_be_bytes([a, b]) } else { u16::from_le_bytes([a, b]) }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// 当前系统 ANSI 代码页对应的编码（仅 Windows；其它平台返回 None）。
#[cfg(windows)]
fn system_encoding() -> Option<&'static encoding_rs::Encoding> {
    let cp = unsafe { windows_sys::Win32::Globalization::GetACP() };
    codepage_to_encoding(cp)
}

#[cfg(not(windows))]
fn system_encoding() -> Option<&'static encoding_rs::Encoding> {
    None
}

/// 常见 Windows 代码页到 encoding_rs 编码的映射。
pub fn codepage_to_encoding(cp: u32) -> Option<&'static encoding_rs::Encoding> {
    use encoding_rs::*;
    Some(match cp {
        936 => GBK,
        54936 => GB18030,
        950 => BIG5,
        932 => SHIFT_JIS,
        949 => EUC_KR,
        874 => WINDOWS_874,
        1250 => WINDOWS_1250,
        1251 => WINDOWS_1251,
        1252 => WINDOWS_1252,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        1257 => WINDOWS_1257,
        1258 => WINDOWS_1258,
        65001 => UTF_8,
        _ => return None,
    })
}

/// 与原版 `_decode` 相同的策略：
/// 1. BOM → UTF-16；2. 大量 0 字节 → UTF-16LE（wsl.exe 无 BOM 输出）；
/// 3. 依次尝试 UTF-8、GBK、系统代码页；4. 最后 UTF-8 有损解码。
pub fn decode_output(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    if let Some(rest) = data.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, false);
    }
    if let Some(rest) = data.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, true);
    }
    if data.len() >= 4 && data.iter().filter(|b| **b == 0).count() > data.len() / 4 {
        return decode_utf16(data, false);
    }
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    if let Ok(s) = std::str::from_utf8(data) {
        return s.to_string();
    }
    let mut encs = vec![encoding_rs::GBK];
    if let Some(e) = system_encoding() {
        if e != encoding_rs::GBK {
            encs.push(e);
        }
    }
    for enc in encs {
        if let Some(s) = enc.decode_without_bom_handling_and_without_replacement(data) {
            return s.into_owned();
        }
    }
    String::from_utf8_lossy(data).into_owned()
}
