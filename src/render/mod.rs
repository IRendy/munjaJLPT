//! 渲染器：把一张试卷渲染成目标格式。
//!
//! 三个后端都实现 [`Renderer`] trait：
//! - [`HtmlRenderer`]  直接由试题数据生成 HTML（内嵌 CSS 与音频，零 JS）
//! - [`MarkdownRenderer`] 生成 Markdown（听解含音频路径 + 全文）
//! - [`PdfRenderer`]   复用 [`HtmlRenderer`] 先写出 HTML，再交给无头浏览器打印成 PDF
//!
//! 渲染器只读数据、写目标文件，由 [`crate::pipeline::Pipeline`] 统一调度。

pub mod html;
pub mod markdown;
pub mod pdf;

pub use html::HtmlRenderer;
pub use markdown::MarkdownRenderer;
pub use pdf::PdfRenderer;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::browser::Browser;
use crate::cli::{AnswerMode, Format};
use crate::model::Exam;

/// 渲染一份试卷时需要的上下文。
#[derive(Debug, Clone)]
pub struct RenderContext<'a> {
    /// 级别，如 "N3"
    pub level: &'a str,
    /// 输出目录（默认 = 试卷目录，即“与 JSON 同级”；
    /// HTML 里的音频/图片/字体相对路径以此为基准）
    pub out_dir: &'a Path,
}

/// 渲染器接口。
pub trait Renderer: Send + Sync {
    /// 输出格式。
    fn format(&self) -> Format;
    /// 输出文件扩展名（不含点）。
    fn extension(&self) -> &'static str;
    /// 渲染并写入 `out_path`；返回实际写出的文件路径。
    ///
    /// 注：`out_path` 的扩展名由 pipeline 按 [`Renderer::extension`] 拼好，
    /// 但 PDF 渲染器会额外写出同名中间 HTML 文件。
    fn render(&self, ctx: &RenderContext, exam: &Exam, out_path: &Path) -> Result<PathBuf>;
}

/// 按 `--format` 组装渲染器。
pub fn build_renderers(
    formats: &[Format],
    answers: AnswerMode,
    font: Option<&Path>,
    browser: Option<&Browser>,
) -> Vec<Box<dyn Renderer>> {
    let mut v: Vec<Box<dyn Renderer>> = Vec::new();
    for f in formats {
        match f {
            Format::Html => v.push(Box::new(HtmlRenderer::new(
                answers,
                font.map(|p| p.to_path_buf()),
            ))),
            Format::Md => v.push(Box::new(MarkdownRenderer::new(answers))),
            Format::Pdf => {
                // browser 为 None 时 main 已提前报错，这里防御性跳过
                if let Some(b) = browser {
                    v.push(Box::new(PdfRenderer::new(
                        b.clone(),
                        answers,
                        font.map(|p| p.to_path_buf()),
                    )));
                }
            }
            Format::All => {}
        }
    }
    v
}
