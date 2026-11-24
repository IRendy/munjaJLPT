//! PDF 渲染器：复用 HTML 渲染，再由无头浏览器打印成 PDF。
//!
//! 设计：PDF 不单独维护一套排版逻辑，而是
//! 1. 用 [`HtmlRenderer`] 把试题渲染成 HTML（含打印样式）；
//! 2. 调用 [`crate::browser::Browser::html_to_pdf`] 无头打印。
//!
//! 因此 PDF 与 HTML 视觉一致，且中间 HTML 文件也会留在同目录下
//! （与单独 `--format html` 的输出相同）。

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::browser::Browser;
use crate::cli::AnswerMode;
use crate::model::Exam;

use super::{Format, HtmlRenderer, RenderContext, Renderer};

pub struct PdfRenderer {
    browser: Browser,
    html: HtmlRenderer,
}

impl PdfRenderer {
    pub fn new(browser: Browser, answers: AnswerMode, font: Option<PathBuf>) -> Self {
        Self {
            browser,
            html: HtmlRenderer::new(answers, font),
        }
    }
}

impl Renderer for PdfRenderer {
    fn format(&self) -> Format {
        Format::Pdf
    }

    fn extension(&self) -> &'static str {
        "pdf"
    }

    fn render(&self, ctx: &RenderContext, exam: &Exam, out_path: &Path) -> Result<PathBuf> {
        // 1) 先写出中间 HTML（与 --format html 输出一致）
        let html_path = out_path.with_extension("html");
        self.html.render(ctx, exam, &html_path)?;
        // 2) 无头浏览器打印为 PDF
        self.browser.html_to_pdf(&html_path, out_path)?;
        Ok(out_path.to_path_buf())
    }
}
