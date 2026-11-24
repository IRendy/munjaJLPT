//! Markdown 渲染器。
//!
//! - 标题 + 题量/时长统计行；
//! - 听解：音频相对路径链接 + 完整听写原文（`<p data-starttime>` 逐段转纯文本）；
//! - 阅读：文章以段落呈现，子题编号「問N」；
//! - 下划线 `<u>` 转 `**粗体**`（标出考点词）；
//! - 图片用 `![図](images/...)`；
//! - 答案按 [`AnswerMode`]：fold → `<details>` 折叠 / show → 直接显示 / hide → 不输出。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::AnswerMode;
use crate::model::{answer_label, category_of, type_label, Exam, Group, Leaf, Question, Section};
use crate::text;

use super::{Format, RenderContext, Renderer};

pub struct MarkdownRenderer {
    answers: AnswerMode,
}

impl MarkdownRenderer {
    pub fn new(answers: AnswerMode) -> Self {
        Self { answers }
    }
}

impl Renderer for MarkdownRenderer {
    fn format(&self) -> Format {
        Format::Md
    }

    fn extension(&self) -> &'static str {
        "md"
    }

    fn render(&self, ctx: &RenderContext, exam: &Exam, out_path: &Path) -> Result<PathBuf> {
        let md = build_document(ctx, exam, self.answers);
        fs::write(out_path, md).with_context(|| format!("写入失败: {}", out_path.display()))?;
        Ok(out_path.to_path_buf())
    }
}

// ---------------------------------------------------------------------------

fn esc_md(s: &str) -> String {
    text::escape_md(s.trim())
}

fn options_md(opts: &[String]) -> String {
    if opts.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = Vec::new();
    for (i, o) in opts.iter().enumerate() {
        lines.push(format!("- {}. {}", i + 1, esc_md(o)));
    }
    lines.join("\n")
}

fn answer_md(leaf: &Leaf, mode: AnswerMode) -> String {
    let Some(ans) = answer_label(leaf) else {
        return String::new();
    };
    match mode {
        AnswerMode::Hide => String::new(),
        AnswerMode::Show => format!("**答え：{}**", esc_md(&ans)),
        AnswerMode::Fold => format!("<details><summary>答案</summary>{}</details>", esc_md(&ans)),
    }
}

fn leaf_md(leaf: &Leaf, num_label: &str, mode: AnswerMode) -> String {
    let mut parts: Vec<String> = Vec::new();
    match &leaf.title {
        Some(t) => parts.push(format!("**{num_label}** {}", text::html_to_md(t))),
        None => parts.push(format!("**{num_label}**")),
    }
    if let Some(img) = &leaf.image {
        parts.push(format!("![図]({img})"));
    }
    let opts = options_md(&leaf.options);
    if !opts.is_empty() {
        parts.push(opts);
    }
    let ans = answer_md(leaf, mode);
    if !ans.is_empty() {
        parts.push(ans);
    }
    parts.join("\n\n")
}

fn subq_md(leaf: &Leaf, idx: usize, mode: AnswerMode) -> String {
    let mut parts: Vec<String> = Vec::new();
    match &leaf.title {
        Some(t) => parts.push(format!("**問{}** {}", idx + 1, text::html_to_md(t))),
        None => parts.push(format!("**問{}**", idx + 1)),
    }
    if let Some(img) = &leaf.image {
        parts.push(format!("![図]({img})"));
    }
    let opts = options_md(&leaf.options);
    if !opts.is_empty() {
        parts.push(opts);
    }
    let ans = answer_md(leaf, mode);
    if !ans.is_empty() {
        parts.push(ans);
    }
    parts.join("\n\n")
}

fn group_md(group: &Group, num: usize, mode: AnswerMode) -> String {
    let is_listening = group.audio.is_some() || group.subtitle.is_some();
    let num_label = if is_listening {
        format!("{num}番")
    } else {
        format!("{num}.")
    };

    let mut lines: Vec<String> = vec![format!("### {num_label}")];
    if let Some(a) = &group.audio {
        lines.push(format!("🎧 [{a}]({a})"));
    }
    if let Some(sub) = &group.subtitle {
        let md = text::html_to_md(sub);
        if !md.is_empty() {
            lines.push(md);
        }
    }
    if let Some(t) = &group.title {
        let md = text::html_to_md(t);
        if !md.is_empty() {
            lines.push(md);
        }
    }
    if let Some(img) = &group.image {
        lines.push(format!("![図]({img})"));
    }
    for (i, sub) in group.questions.iter().enumerate() {
        if let Question::Leaf(l) = sub {
            lines.push(String::new());
            lines.push(subq_md(l, i, mode));
        }
    }
    lines.join("\n")
}

fn section_md(section: &Section, mode: AnswerMode) -> String {
    let qt = section.question_type.unwrap_or(0);
    let cat = category_of(qt);
    let label = type_label(qt).unwrap_or("");

    let mut heading = cat.label_zh().to_string();
    if !label.is_empty() {
        heading.push_str("・");
        heading.push_str(label);
    }

    let mut lines: Vec<String> = vec![format!("## {heading}"), String::new()];

    if let Some(t) = &section.title {
        let instr = text::html_to_md(t);
        if !instr.is_empty() {
            lines.push(format!("> {instr}"));
            lines.push(String::new());
        }
    }
    if let Some(a) = &section.audio {
        lines.push(format!("🎧 導入音声：[{a}]({a})"));
        lines.push(String::new());
    }
    if let Some(r) = &section.reference {
        let md = text::html_to_md(r);
        if !md.is_empty() {
            lines.push(md);
            lines.push(String::new());
        }
    }

    let mut num = 0usize;
    for q in &section.questions {
        match q {
            Question::Null => {}
            Question::Group(g) => {
                num += 1;
                lines.push(group_md(g, num, mode));
            }
            Question::Leaf(l) => {
                num += 1;
                lines.push(leaf_md(l, &format!("{num}."), mode));
            }
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string() + "\n"
}

fn build_document(ctx: &RenderContext, exam: &Exam, mode: AnswerMode) -> String {
    let base = &exam.base;
    let title = match &base.title {
        Some(t) => format!("{t} {}", ctx.level),
        None => ctx.level.to_string(),
    };

    let mut lines: Vec<String> = vec![format!("# 日本語能力試験 {title}"), String::new()];

    let mut meta: Vec<String> = Vec::new();
    for (n, name) in [
        (base.vocabulary_num, "語彙"),
        (base.grammar_num, "文法"),
        (base.reading_num, "読解"),
        (base.listening_num, "聴解"),
    ] {
        if let Some(n) = n {
            meta.push(format!("{name} {n}問"));
        }
    }
    if let Some(d) = &base.duration {
        if let Some(m) = d.language_knowledge_reading {
            meta.push(format!("言語知識・読解 {m}分"));
        }
        if let Some(m) = d.listening {
            meta.push(format!("聴解 {m}分"));
        }
    }
    if !meta.is_empty() {
        lines.push(meta.join("・"));
        lines.push(String::new());
    }
    if let Some(a) = &base.audio {
        lines.push(format!("🎧 聴解全文：[{a}]({a})"));
        lines.push(String::new());
    }

    for s in &exam.sections {
        lines.push(section_md(s, mode));
    }
    lines.join("\n")
}
