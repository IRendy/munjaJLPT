//! HTML 渲染器：单文件、内嵌 CSS + `<audio>` 音频、零 JS。
//!
//! - 屏幕样式 + `@media print` 打印样式（打印时隐藏播放器、显示音频文件名、展开答案）；
//! - `@font-face` 引用 [`crate::font::FontLocator`] 找到的字体（file:// URL，加载失败则回退系统字体）；
//! - 答案按 [`AnswerMode`]：fold → `<details>` 折叠 / show → 直接显示 / hide → 不输出。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::AnswerMode;
use crate::model::{answer_label, category_of, type_label, Exam, Group, Leaf, Question, Section};
use crate::text;

use super::{Format, RenderContext, Renderer};

pub struct HtmlRenderer {
    answers: AnswerMode,
    font: Option<PathBuf>,
}

impl HtmlRenderer {
    pub fn new(answers: AnswerMode, font: Option<PathBuf>) -> Self {
        Self { answers, font }
    }

    fn font_face(&self, out_dir: &Path) -> String {
        let Some(f) = &self.font else {
            return String::new();
        };
        // 字体在输出目录树内 → 用相对路径（便于整体打包）；否则回退 file:// 绝对 URL
        let url = match relative_path(out_dir, f) {
            Some(rel) => rel,
            None => format!("file://{}", f.display()),
        };
        format!(
            "@font-face{{font-family:\"MunjaCJK\";src:url(\"{}\");}}",
            text::escape_html(&url)
        )
    }
}

impl Renderer for HtmlRenderer {
    fn format(&self) -> Format {
        Format::Html
    }

    fn extension(&self) -> &'static str {
        "html"
    }

    fn render(&self, ctx: &RenderContext, exam: &Exam, out_path: &Path) -> Result<PathBuf> {
        let html = build_document(ctx, exam, &self.font_face(ctx.out_dir), self.answers);
        fs::write(out_path, html)
            .with_context(|| format!("写入失败: {}", out_path.display()))?;
        Ok(out_path.to_path_buf())
    }
}

/// 计算 `from_dir` 到 `to_file` 的相对路径（两路径均需存在）。
/// 不同根/盘时返回 `None`。结果用 `/` 分隔，可直接用于 HTML `url()`。
fn relative_path(from_dir: &Path, to_file: &Path) -> Option<String> {
    use std::path::Component;
    let from = from_dir.canonicalize().ok()?;
    let to = to_file.canonicalize().ok()?;
    let fc: Vec<Component<'_>> = from.components().collect();
    let tc: Vec<Component<'_>> = to.components().collect();
    if fc.first() != tc.first() {
        return None;
    }
    let mut i = 0;
    while i < fc.len() && i < tc.len() && fc[i] == tc[i] {
        i += 1;
    }
    let mut rel = std::path::PathBuf::new();
    for _ in i..fc.len() {
        rel.push("..");
    }
    for c in &tc[i..] {
        rel.push(c.as_os_str());
    }
    if rel.as_os_str().is_empty() {
        return Some(".".into());
    }
    Some(rel.to_string_lossy().replace('\\', "/"))
}

// ---------------------------------------------------------------------------
// 小部件
// ---------------------------------------------------------------------------

fn esc(s: &str) -> String {
    text::escape_html(s)
}

fn audio_html(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    format!(
        "<audio controls preload=\"none\" src=\"{0}\"></audio><span class=\"audio-filename\">{0}</span>",
        esc(src)
    )
}

fn image_html(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    format!("<img src=\"{}\" alt=\"図\">", esc(src))
}

fn script_html(subtitle: &str) -> String {
    if subtitle.is_empty() {
        return String::new();
    }
    format!(
        "<div class=\"script\">{}</div>",
        text::clean_html(subtitle)
    )
}

fn options_html(opts: &[String]) -> String {
    if opts.is_empty() {
        return String::new();
    }
    let items: String = opts
        .iter()
        .map(|o| format!("<li>{}</li>", esc(o.trim())))
        .collect();
    format!("<ol class=\"options\">{items}</ol>")
}

fn answer_html(leaf: &Leaf, mode: AnswerMode) -> String {
    let Some(ans) = answer_label(leaf) else {
        return String::new();
    };
    match mode {
        AnswerMode::Hide => String::new(),
        AnswerMode::Show => format!("<div class=\"answer\">答え：{}</div>", esc(&ans)),
        AnswerMode::Fold => format!(
            "<details><summary>答案</summary><div class=\"answer\">答え：{}</div></details>",
            esc(&ans)
        ),
    }
}

fn leaf_html(leaf: &Leaf, num_label: &str, mode: AnswerMode) -> String {
    let mut out = format!("<div class=\"q\"><span class=\"q-num\">{num_label}</span>");
    if let Some(t) = &leaf.title {
        out.push_str(&format!("<div class=\"q-title\">{}</div>", text::clean_html(t)));
    }
    if let Some(img) = &leaf.image {
        out.push_str(&image_html(img));
    }
    out.push_str(&options_html(&leaf.options));
    out.push_str(&answer_html(leaf, mode));
    out.push_str("</div>");
    out
}

fn subq_html(leaf: &Leaf, idx: usize, mode: AnswerMode) -> String {
    let mut out = format!(
        "<div class=\"subq\"><span class=\"q-num\">問{}</span>",
        idx + 1
    );
    if let Some(t) = &leaf.title {
        out.push_str(&format!("<div class=\"q-title\">{}</div>", text::clean_html(t)));
    }
    if let Some(img) = &leaf.image {
        out.push_str(&image_html(img));
    }
    out.push_str(&options_html(&leaf.options));
    out.push_str(&answer_html(leaf, mode));
    out.push_str("</div>");
    out
}

fn group_html(group: &Group, num: usize, mode: AnswerMode) -> String {
    let is_listening = group.audio.is_some() || group.subtitle.is_some();
    let num_label = if is_listening {
        format!("{num}番")
    } else {
        format!("{num}.")
    };

    let mut out = format!("<div class=\"q group\"><span class=\"q-num\">{num_label}</span>");
    if let Some(a) = &group.audio {
        out.push_str(&audio_html(a));
    }
    if let Some(sub) = &group.subtitle {
        out.push_str(&script_html(sub));
    }
    if let Some(t) = &group.title {
        out.push_str(&format!(
            "<div class=\"q-passage\">{}</div>",
            text::clean_html(t)
        ));
    }
    if let Some(img) = &group.image {
        out.push_str(&image_html(img));
    }
    for (i, sub) in group.questions.iter().enumerate() {
        if let Question::Leaf(l) = sub {
            out.push_str(&subq_html(l, i, mode));
        }
    }
    out.push_str("</div>");
    out
}

fn section_html(section: &Section, mode: AnswerMode) -> String {
    let qt = section.question_type.unwrap_or(0);
    let cat = category_of(qt);
    let label = type_label(qt).unwrap_or("");

    let mut out = format!(
        "<section class=\"sec sec-{}\"><h2><span class=\"cat {}\">{}</span>",
        cat.css_class(),
        cat.css_class(),
        esc(cat.label_zh())
    );
    if !label.is_empty() {
        out.push_str(&format!("<span class=\"cat-label\">{}</span>", esc(label)));
    }
    out.push_str("</h2>");

    if let Some(t) = &section.title {
        out.push_str(&format!(
            "<div class=\"instruction\">{}</div>",
            text::clean_html(t)
        ));
    }
    if let Some(a) = &section.audio {
        out.push_str(&audio_html(a));
    }
    if let Some(r) = &section.reference {
        out.push_str(&format!(
            "<div class=\"reference\">{}</div>",
            text::clean_html(r)
        ));
    }

    let mut num = 0usize;
    for q in &section.questions {
        match q {
            Question::Null => {}
            Question::Group(g) => {
                num += 1;
                out.push_str(&group_html(g, num, mode));
            }
            Question::Leaf(l) => {
                num += 1;
                out.push_str(&leaf_html(l, &format!("{num}."), mode));
            }
        }
    }
    out.push_str("</section>");
    out
}

// ---------------------------------------------------------------------------
// 整页
// ---------------------------------------------------------------------------

const CSS: &str = r#":root{--ink:#1f2430;--muted:#6b7280;--accent:#c2410c;--bg:#faf9f6;--card:#fff;--line:#e7e3da;}
*{box-sizing:border-box;}
body{font-family:"MunjaCJK","Hiragino Kaku Gothic ProN","Yu Gothic","Noto Sans CJK JP","Meiryo",sans-serif;
     color:var(--ink);background:var(--bg);max-width:920px;margin:0 auto;padding:28px 20px 60px;line-height:1.8;}
h1{font-size:1.45em;margin:0 0 6px;}
.meta{color:var(--muted);font-size:.92em;margin-bottom:8px;}
.meta .chips{display:flex;flex-wrap:wrap;gap:8px;margin-top:6px;}
.chip{background:#fff;border:1px solid var(--line);border-radius:999px;padding:2px 12px;font-size:.85em;}
section{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:18px 22px;margin:20px 0;}
h2{margin:0 0 10px;font-size:1.15em;}
.cat{display:inline-block;padding:2px 11px;border-radius:999px;color:#fff;font-size:.78em;margin-right:8px;}
.cat-label{color:var(--muted);font-weight:600;font-size:.9em;}
.cat-vocab{background:#d97706}.cat-grammar{background:#059669}
.cat-reading{background:#2563eb}.cat-listening{background:#db2777}.cat-other{background:#6b7280}
.instruction{color:var(--muted);font-size:.9em;border-left:3px solid var(--line);padding-left:12px;margin:4px 0 12px;}
.reference{margin:8px 0;padding:10px 14px;background:#fbfaf7;border:1px dashed var(--line);border-radius:8px;}
.reference table{border-collapse:collapse;margin:8px 0;}
.reference th,.reference td{border:1px solid #d8d3c8;padding:4px 8px;font-size:.9em;}
.q{margin:20px 0;padding-top:14px;border-top:1px dashed var(--line);}
.q:first-of-type{border-top:none;padding-top:0;}
.q-num{font-weight:700;color:var(--accent);margin-right:8px;}
.q-title{margin:4px 0;}
.q-passage{margin:8px 0;padding:10px 14px;background:#fbfaf7;border-radius:8px;}
.options{margin:8px 0;padding-left:24px;}
.options li{margin:3px 0;}
audio{width:100%;margin:10px 0;}
.audio-filename{display:none;}
.script{background:#f6f4ef;border-radius:8px;padding:12px 16px;font-size:.94em;margin:8px 0;}
.script p{margin:5px 0;}
img{max-width:100%;border:1px solid var(--line);border-radius:8px;margin:8px 0;}
.subq{margin:12px 0 12px 14px;padding-left:12px;border-left:3px solid var(--line);}
details{margin-top:10px;}
summary{cursor:pointer;color:var(--muted);font-size:.88em;font-weight:600;}
.answer{color:#047857;font-weight:700;margin-top:4px;}
@media print{
  @page{size:A4;margin:14mm 12mm;}
  body{max-width:none;padding:0;background:#fff;font-size:10.5pt;}
  section{border:none;border-radius:0;padding:0;margin:14px 0;break-inside:auto;}
  .q,.subq{break-inside:avoid;}
  .script p{margin:2px 0;}
  audio{display:none;}
  .audio-filename{display:inline;color:var(--muted);font-size:.85em;margin-left:8px;}
  details summary{display:none;}
  details .answer{display:block;}
  h1,h2{break-after:avoid;}
}
"#;

fn build_document(ctx: &RenderContext, exam: &Exam, font_face: &str, mode: AnswerMode) -> String {
    let base = &exam.base;
    let title = match &base.title {
        Some(t) => format!("{t} {}", ctx.level),
        None => ctx.level.to_string(),
    };

    // 统计 chips
    let mut chips: Vec<String> = Vec::new();
    for (n, name) in [
        (base.vocabulary_num, "語彙"),
        (base.grammar_num, "文法"),
        (base.reading_num, "読解"),
        (base.listening_num, "聴解"),
    ] {
        if let Some(n) = n {
            chips.push(format!("{name} {n}問"));
        }
    }
    if let Some(d) = &base.duration {
        if let Some(m) = d.language_knowledge_reading {
            chips.push(format!("言語知識・読解 {m}分"));
        }
        if let Some(m) = d.listening {
            chips.push(format!("聴解 {m}分"));
        }
    }
    let chips_html: String = chips
        .iter()
        .map(|c| format!("<span class=\"chip\">{}</span>", esc(c)))
        .collect();

    let full_audio = audio_html(base.audio.as_deref().unwrap_or(""));

    let sections: String = exam
        .sections
        .iter()
        .map(|s| section_html(s, mode))
        .collect();

    format!(
        "<!DOCTYPE html>\n<html lang=\"ja\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>日本語能力試験 {title}</title>\n\
         <style>{font_face}{CSS}</style>\n</head>\n<body>\n\
         <header>\n<h1>日本語能力試験 {title}</h1>\n\
         <div class=\"meta\">\n<div class=\"chips\">{chips_html}</div>\n{full_audio}\n</div>\n</header>\n\
         <main>\n{sections}\n</main>\n</body>\n</html>\n",
        title = esc(&title),
    )
}
