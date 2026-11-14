//! UsbipdTool 核心库：不依赖 GUI，可在任意平台上单元测试。
pub mod core;

/// 与 Cargo.toml / Windows 版本资源 / 窗口标题中的版本一致。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
