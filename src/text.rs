//! 文本转换工具：HTML 清理 / 转义 / HTML→Markdown。
//!
//! 供 HTML 与 Markdown 两个渲染器共用。正则用 [`std::sync::OnceLock`] 缓存，
//! 避免每道题重复编译。

use std::sync::OnceLock;

use regex::Regex;

// ---------------------------------------------------------------------------
// 正则缓存
// ---------------------------------------------------------------------------

fn re_class() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\s+class="[^"]*""#).unwrap())
}

fn re_data_attr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\s+data-(?:starttime|endtime)="[^"]*""#).unwrap())
}

fn re_br() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)<br\s*/?>").unwrap())
}

fn re_close_p() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)</p\s*>").unwrap())
}

fn re_close_tr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)</tr\s*>").unwrap())
}

fn re_td_th_open() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)<(?:td|th)[^>]*>").unwrap())
}

fn re_td_th_close() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)</(?:td|th)\s*>").unwrap())
}

fn re_u_open() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)<u>").unwrap())
}

fn re_u_close() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)</u>").unwrap())
}

fn re_any_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<[^>]+>").unwrap())
}

fn re_line_indent() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^[ \t]+").unwrap())
}

fn re_trailing_ws() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[ \t]+\n").unwrap())
}

fn re_many_nl() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\n{3,}").unwrap())
}

// ---------------------------------------------------------------------------
// HTML 转义
// ---------------------------------------------------------------------------

/// 把文本转义成可安全内嵌进 HTML 的字符实体。
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// 反转义常见的 HTML 命名/数字实体。
pub fn unescape_html(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

// ---------------------------------------------------------------------------
// HTML 清理 / 转换
// ---------------------------------------------------------------------------

/// 清理内容字段（本身就是 HTML）：去掉 `class`/`data-*` 属性、
/// 制表符缩进，`&nbsp;` 换空格，保留 `<u>` `<br>` `<table>` 等结构标签。
pub fn clean_html(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut t = re_class().replace_all(s, "").to_string();
    t = re_data_attr().replace_all(&t, "").to_string();
    t = t.replace('\t', "");
    t = unescape_html(&t);
    t
}

/// 把内容 HTML 转成 Markdown 文本：
/// `<br>`/`</p>` 换行、`<u>` 转粗体、表格转竖线分隔行、去其余标签、去行首缩进。
pub fn html_to_md(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut t = re_br().replace_all(s, "\n").to_string();
    t = re_close_p().replace_all(&t, "\n\n").to_string();
    t = re_close_tr().replace_all(&t, "\n").to_string();
    t = re_td_th_open().replace_all(&t, " ").to_string();
    t = re_td_th_close().replace_all(&t, " | ").to_string();
    t = re_u_open().replace_all(&t, "**").to_string();
    t = re_u_close().replace_all(&t, "**").to_string();
    t = re_any_tag().replace_all(&t, "").to_string();
    t = unescape_html(&t);
    t = re_line_indent().replace_all(&t, "").to_string();
    t = re_trailing_ws().replace_all(&t, "\n").to_string();
    t = re_many_nl().replace_all(&t, "\n\n").to_string();
    t.trim().to_string()
}

/// Markdown 纯文本字段的轻量转义（竖线会导致表格解析问题）。
pub fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
}
