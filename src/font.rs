//! 日文字体查找。
//!
//! 用途：HTML 通过 `@font-face` 引用同一字体文件，保证在不同机器上
//! （尤其生成 PDF 时）日文排版一致。
//!
//! 查找顺序：
//! 1. `--font` 显式指定；
//! 2. `<data_dir>/fonts/` 下的常见字体名；
//! 3. 系统常见路径（Windows 的 Yu Gothic / MS Gothic，macOS 的 Hiragino，Linux 的 Noto CJK / IPA）；
//! 4. Linux 上调用 `fc-match` 兜底。
//!
//! 找不到时返回 `None`，HTML 回退到浏览器默认字体。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 本地字体目录（相对数据根目录）里会尝试的文件名。
/// 优先静态 Regular 字重（思源黑体 / Noto），VF 变量字体（NotoSansCJK-VF.ttc）
/// 在 Chromium 下会误选 Thin 实例，故不列入。
const FONT_NAMES: &[&str] = &[
    "SourceHanSansSC-Regular.otf",
    "SourceHanSansJP-Regular.otf",
    "NotoSansCJKjp-Regular.otf",
    "NotoSansJP-Regular.otf",
    "ipag.ttf",
    "YuGothM.ttc",
    "msgothic.ttc",
    "meiryo.ttc",
];

/// 字体查找器。
pub struct FontLocator;

impl FontLocator {
    pub fn locate(explicit: Option<&Path>, data_dir: &Path) -> Option<PathBuf> {
        // 1) 显式指定
        if let Some(p) = explicit {
            return p.is_file().then(|| p.to_path_buf());
        }
        // 2) 数据根目录 fonts/ 下的常见名
        let local_dir = data_dir.join("fonts");
        for name in FONT_NAMES {
            let p = local_dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        // 2b) 项目根目录 fonts/（数据在 exam/ 下、字体在根目录时）
        let cwd_fonts = Path::new("fonts");
        if cwd_fonts != local_dir {
            for name in FONT_NAMES {
                let p = cwd_fonts.join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        // 3) 系统常见路径
        for p in Self::system_candidates() {
            if p.is_file() {
                return Some(p);
            }
        }
        // 4) Linux: fc-match 兜底
        #[cfg(target_os = "linux")]
        {
            if let Some(p) = fc_match_ja() {
                return Some(p);
            }
        }
        None
    }

    /// 各平台系统字体候选路径。
    pub fn system_candidates() -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Vec::new();
        #[cfg(target_os = "windows")]
        v.extend([
            PathBuf::from(r"C:\Windows\Fonts\YuGothM.ttc"),
            PathBuf::from(r"C:\Windows\Fonts\YuGothR.ttc"),
            PathBuf::from(r"C:\Windows\Fonts\msgothic.ttc"),
            PathBuf::from(r"C:\Windows\Fonts\meiryo.ttc"),
        ]);
        #[cfg(target_os = "macos")]
        v.extend([
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc".into(),
            "/System/Library/Fonts/Hiragino Sans GB.ttc".into(),
            "/Library/Fonts/Arial Unicode.ttf".into(),
        ]);
        #[cfg(target_os = "linux")]
        v.extend([
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf".into(),
        ]);
        v
    }
}

/// 用 fontconfig 查日文字体文件路径。
#[cfg(target_os = "linux")]
fn fc_match_ja() -> Option<PathBuf> {
    let out = Command::new("fc-match")
        .args(["-f", "%{file}", ":lang=ja"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return None;
    }
    let p = PathBuf::from(s);
    p.is_file().then_some(p)
}
