// 仅在目标为 Windows 时嵌入资源（图标 + requireAdministrator 清单 + 版本信息）。
// 版本号取自 Cargo.toml（CARGO_PKG_VERSION）；交叉编译时 embed-resource
// 会自动调用 `x86_64-w64-mingw32-windres`。
use std::path::{Path, PathBuf};

fn rc_escape(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// 将 "0.1.0" / "0.1.0-rc.1" 解析为 VERSIONINFO 的四段数字。
fn file_version(v: &str) -> (u16, u16, u16, u16) {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let mut it = core.split('.').filter_map(|s| s.parse::<u16>().ok());
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

fn main() {
    println!("cargo:rerun-if-changed=resources/app.manifest");
    println!("cargo:rerun-if-changed=resources/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let (maj, min, pat, bld) = file_version(&version);
    let numbered = format!("{maj},{min},{pat},{bld}");
    let dotted = format!("{maj}.{min}.{pat}.{bld}");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let icon = rc_escape(&manifest_dir.join("resources/icon.ico"));
    let manifest_src = manifest_dir.join("resources/app.manifest");
    let manifest_out = out_dir.join("app.manifest");

    let mut mf = std::fs::read_to_string(&manifest_src).expect("read app.manifest");
    // 把 assemblyIdentity 的 version 与 Cargo 对齐（保留四段）
    if let Some(start) = mf.find("assemblyIdentity version=\"") {
        let val_start = start + "assemblyIdentity version=\"".len();
        if let Some(rel_end) = mf[val_start..].find('"') {
            mf.replace_range(val_start..val_start + rel_end, &dotted);
        }
    }
    std::fs::write(&manifest_out, mf).expect("write generated manifest");

    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"
1 24 "{manifest}"

1 VERSIONINFO
FILEVERSION {numbered}
PRODUCTVERSION {numbered}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "UsbipdTool - USBIPD WSL USB Helper"
      VALUE "FileVersion", "{version}"
      VALUE "ProductName", "UsbipdTool"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "UsbipdTool.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = icon,
        manifest = rc_escape(&manifest_out),
        numbered = numbered,
        version = version,
    );
    let rc_path = out_dir.join("app.rc");
    std::fs::write(&rc_path, rc).expect("write generated app.rc");

    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed Windows resources (icon + manifest + version)");
}
