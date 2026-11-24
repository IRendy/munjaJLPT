//! 无头浏览器探测与 HTML → PDF 打印。
//!
//! PDF 由浏览器把渲染好的 HTML 打印生成，因此：
//! - Windows 上优先找系统自带的 Edge（Win10+ 都有），其次 Chrome；
//! - macOS 找 Chrome / Edge / Chromium 应用包；
//! - Linux 在 PATH 中找 microsoft-edge / google-chrome / chromium 等。
//!
//! 找不到时用 `--browser <路径>` 显式指定。

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    Edge,
    Chrome,
    Chromium,
}

impl BrowserKind {
    pub fn name(&self) -> &'static str {
        match self {
            BrowserKind::Edge => "Edge",
            BrowserKind::Chrome => "Chrome",
            BrowserKind::Chromium => "Chromium",
        }
    }
}

/// 已定位的浏览器。
#[derive(Debug, Clone)]
pub struct Browser {
    pub path: PathBuf,
    pub kind: BrowserKind,
}

impl Browser {
    /// 用无头模式把 HTML 文件打印成 PDF。
    ///
    /// 使用独立 `--user-data-dir` 避免把任务转交给已打开的浏览器实例。
    pub fn html_to_pdf(&self, html_path: &Path, pdf_path: &Path) -> Result<()> {
        // 转成绝对路径：file:// URL 与 --print-to-pdf 都要求绝对路径
        let abs_html = std::path::absolute(html_path)
            .unwrap_or_else(|_| html_path.to_path_buf());
        let abs_pdf = std::path::absolute(pdf_path)
            .unwrap_or_else(|_| pdf_path.to_path_buf());
        let html_url = {
            let s = abs_html.to_string_lossy().replace('\\', "/");
            if s.starts_with('/') {
                format!("file://{s}")
            } else {
                format!("file:///{s}")
            }
        };
        let profile = std::env::temp_dir().join(format!("munja-profile-{}", std::process::id()));

        let output = Command::new(&self.path)
            .args(["--headless", "--disable-gpu", "--no-pdf-header-footer"])
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(format!("--print-to-pdf={}", abs_pdf.display()))
            .arg(&html_url)
            .output()
            .with_context(|| format!("启动浏览器失败: {}", self.path.display()))?;

        let _ = fs_remove_dir(&profile);

        if !output.status.success() {
            bail!(
                "浏览器退出码 {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        if !pdf_path.is_file() {
            bail!("未生成 PDF 文件: {}", pdf_path.display());
        }
        Ok(())
    }
}

fn fs_remove_dir(p: &Path) -> std::io::Result<()> {
    std::fs::remove_dir_all(p)
}

/// 浏览器查找。
pub struct BrowserLocator;

impl BrowserLocator {
    /// `--browser` 显式指定优先，否则按平台候选列表依次探测。
    pub fn locate(explicit: Option<&Path>) -> Result<Browser> {
        if let Some(p) = explicit {
            if !p.is_file() {
                bail!("指定的浏览器不存在: {}", p.display());
            }
            return Ok(Browser {
                path: p.to_path_buf(),
                kind: guess_kind(p),
            });
        }
        for (kind, path) in Self::candidates() {
            if path.is_file() {
                return Ok(Browser { path, kind });
            }
        }
        Err(anyhow::anyhow!(
            "未找到可用的浏览器。Windows 请确认 Edge 存在；\
             其它平台请安装 Chromium/Chrome，或用 --browser <路径> 指定。"
        ))
    }

    /// 各平台的候选路径列表。
    pub fn candidates() -> Vec<(BrowserKind, PathBuf)> {
        let mut v: Vec<(BrowserKind, PathBuf)> = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(pf) = std::env::var("PROGRAMFILES(X86)") {
                v.push((BrowserKind::Edge, PathBuf::from(&pf).join(r"Microsoft\Edge\Application\msedge.exe")));
                v.push((BrowserKind::Chrome, PathBuf::from(&pf).join(r"Google\Chrome\Application\chrome.exe")));
            }
            if let Ok(pf) = std::env::var("PROGRAMFILES") {
                v.push((BrowserKind::Edge, PathBuf::from(&pf).join(r"Microsoft\Edge\Application\msedge.exe")));
                v.push((BrowserKind::Chrome, PathBuf::from(&pf).join(r"Google\Chrome\Application\chrome.exe")));
            }
            if let Ok(la) = std::env::var("LOCALAPPDATA") {
                v.push((BrowserKind::Edge, PathBuf::from(&la).join(r"Microsoft\Edge\Application\msedge.exe")));
                v.push((BrowserKind::Chrome, PathBuf::from(&la).join(r"Google\Chrome\Application\chrome.exe")));
            }
        }

        #[cfg(target_os = "macos")]
        {
            v.push((
                BrowserKind::Chrome,
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
            ));
            v.push((
                BrowserKind::Edge,
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into(),
            ));
            v.push((
                BrowserKind::Chromium,
                "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            for name in [
                "microsoft-edge",
                "microsoft-edge-stable",
                "google-chrome",
                "google-chrome-stable",
                "chromium",
                "chromium-browser",
            ] {
                if let Some(p) = find_in_path(name) {
                    let kind = if name.contains("edge") {
                        BrowserKind::Edge
                    } else if name.contains("chrome") {
                        BrowserKind::Chrome
                    } else {
                        BrowserKind::Chromium
                    };
                    v.push((kind, p));
                }
            }
        }

        v
    }
}

#[cfg(target_os = "linux")]
fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}

/// 由文件名猜测浏览器类型（`--browser` 显式指定时用）。
fn guess_kind(p: &Path) -> BrowserKind {
    let s = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if s.contains("edge") {
        BrowserKind::Edge
    } else if s.contains("chrom") {
        BrowserKind::Chromium
    } else {
        BrowserKind::Chrome
    }
}
