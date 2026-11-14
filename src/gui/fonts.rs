//! 运行时加载系统中文字体（微软雅黑 / 黑体等），保证中文可显示。

use eframe::egui;
use egui::{FontData, FontFamily, FontInsert, FontPriority, InsertFontFamily};

const CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyh.ttf",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
    r"C:\Windows\Fonts\Deng.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];

const MONO_CANDIDATES: &[&str] = &[r"C:\Windows\Fonts\consola.ttf"];

fn read_first(paths: &[&str]) -> Option<(String, Vec<u8>)> {
    paths
        .iter()
        .find_map(|p| std::fs::read(p).ok().map(|d| (p.to_string(), d)))
}

/// 安装字体：Consolas 作为日志等宽字体（若存在），CJK 字体作为所有族的回退。
pub fn install(ctx: &egui::Context) {
    if let Some((_, data)) = read_first(MONO_CANDIDATES) {
        ctx.add_font(FontInsert::new(
            "consolas",
            FontData::from_owned(data),
            vec![InsertFontFamily { family: FontFamily::Monospace, priority: FontPriority::Highest }],
        ));
    }
    if let Some((_, data)) = read_first(CANDIDATES) {
        ctx.add_font(FontInsert::new(
            "cjk",
            FontData::from_owned(data),
            vec![
                InsertFontFamily { family: FontFamily::Proportional, priority: FontPriority::Lowest },
                InsertFontFamily { family: FontFamily::Monospace, priority: FontPriority::Lowest },
            ],
        ));
    }
}
