//! 命令行参数定义。

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// 整理 JLPT 试题：把每个试卷的 JSON 渲染成 HTML / Markdown / PDF。
#[derive(Parser, Debug)]
#[command(
    name = "munja",
    version,
    about = "整理 JLPT（日语能力考试）试题：JSON → HTML / Markdown / PDF"
)]
pub struct Cli {
    /// 要处理的级别或试卷 JSON 路径（.json 结尾视为单个文件；不写则默认 N3 N4 N5）
    #[arg(value_name = "LEVEL|JSON")]
    pub levels: Vec<String>,

    /// 数据根目录（含 N1–N5 子目录），默认 exam/
    #[arg(long, default_value = "exam")]
    pub data_dir: PathBuf,

    /// 输出格式，逗号分隔：html / md / pdf / all（默认 html,md）
    #[arg(short = 'f', long, value_delimiter = ',', default_values = ["html", "md"])]
    pub formats: Vec<Format>,

    /// 答案显示模式：fold=HTML/MD 折叠、show=直接显示、hide=不输出答案
    #[arg(long, value_enum, default_value = "fold")]
    pub answers: AnswerMode,

    /// 浏览器可执行文件路径（生成 PDF 用；默认自动检测 Edge/Chrome/Chromium）
    #[arg(long)]
    pub browser: Option<PathBuf>,

    /// 日文字体文件路径（HTML 以 @font-face 引用；默认自动查找）
    #[arg(long)]
    pub font: Option<PathBuf>,

    /// 只打印执行计划，不写任何文件
    #[arg(long)]
    pub dry_run: bool,

    /// 安静模式：只输出错误
    #[arg(short, long)]
    pub quiet: bool,

    /// 清理模式：删除各试卷生成的 .html/.md/.pdf（只删与试卷 JSON 同名的文件，不碰源 JSON、音频与图片）
    #[arg(long)]
    pub clean: bool,
}

/// 输出格式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Html,
    Md,
    Pdf,
    /// 全部格式（html + md + pdf）
    All,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Format::Html => "html",
            Format::Md => "md",
            Format::Pdf => "pdf",
            Format::All => "all",
        })
    }
}

/// 答案显示模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum AnswerMode {
    /// 折叠（HTML 用 <details>，Markdown 用折叠块）
    #[default]
    Fold,
    /// 直接显示
    Show,
    /// 不输出答案
    Hide,
}

impl std::fmt::Display for AnswerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AnswerMode::Fold => "折叠",
            AnswerMode::Show => "显示",
            AnswerMode::Hide => "隐藏",
        })
    }
}

impl Cli {
    /// 展开 "all" 为全部三种格式，去重并保持顺序。
    pub fn resolved_formats(&self) -> Vec<Format> {
        if self.formats.contains(&Format::All) {
            vec![Format::Html, Format::Md, Format::Pdf]
        } else {
            self.formats.clone()
        }
    }

    /// 未指定级别时，默认 N3 N4 N5。
    pub fn effective_levels(&self) -> Vec<String> {
        if self.levels.is_empty() {
            vec!["N3".into(), "N4".into(), "N5".into()]
        } else {
            self.levels.clone()
        }
    }
}
