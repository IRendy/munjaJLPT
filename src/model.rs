//! 试题 JSON 的数据模型。
//!
//! 与源 JSON 结构对应：
//! ```text
//! { "base": { title, listeningNum, ..., duration: {...} },
//!   "sections": [ { title, questionType, audio?, reference?, questions: [题...] } ] }
//! ```
//!
//! 一道“题”可能是两种形态，用 [`Question`] 枚举区分：
//! - [`Question::Leaf`]  单题（直接含 answer/options，如汉字读音、文脉规定）
//! - [`Question::Group`] 组题（含文章/听解原文与若干子题，如读解一篇、听解一段）
//! - [`Question::Null`]  源数据中的 `null` 占位（个别试卷存在，安全跳过）

use serde::Deserialize;

/// 选项列表：兼容字符串与数字两种形态
/// （如 `["つつんで", ...]` 与 `[1, 2, 3, 4]`，统一转成字符串）。
fn deser_string_list<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    let vals = Vec::<serde_json::Value>::deserialize(d)?;
    vals.into_iter()
        .map(|v| match v {
            serde_json::Value::String(s) => Ok(s),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            other => Err(D::Error::custom(format!("选项需为字符串或数字，实际: {other}"))),
        })
        .collect()
}

/// 答案下标：兼容 `"0"` 与 `0` 两种形态。
fn deser_opt_string<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    match Option::<serde_json::Value>::deserialize(d)? {
        None => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(other) => Err(D::Error::custom(format!("answer 需为字符串或数字，实际: {other}"))),
    }
}

/// 兼容字符串 / 数字 / 字符串数组（用 " / " 拼接）的字段（identity 等）。
fn deser_flex_string<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    match Option::<serde_json::Value>::deserialize(d)? {
        None => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(serde_json::Value::Array(arr)) => {
            let parts: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => String::new(),
                })
                .collect();
            Ok(Some(parts.join(" / ")))
        }
        Some(other) => Err(D::Error::custom(format!("identity 需为字符串或数组，实际: {other}"))),
    }
}

/// 一张试卷（一个 JSON 文件）。
#[derive(Debug, Deserialize)]
pub struct Exam {
    pub base: Base,
    #[serde(default)]
    pub sections: Vec<Section>,
}

/// `base` 字段：试卷元信息。
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Base {
    pub title: Option<String>,
    pub listening_num: Option<u32>,
    pub reading_num: Option<u32>,
    pub grammar_num: Option<u32>,
    pub vocabulary_num: Option<u32>,
    pub audio: Option<String>,
    pub duration: Option<Durations>,
}

/// 考试时长（分钟）。
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Durations {
    pub language_knowledge_reading: Option<u32>,
    pub listening: Option<u32>,
}

/// 一个大题（如“问题1 汉字读音”）。
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Section {
    /// 题目说明（HTML，可能含 `<u>` 下划线等）
    pub title: Option<String>,
    /// 题型编号：11–16 语彙 / 21–23 文法 / 31–37 読解 / 41–45 聴解
    pub question_type: Option<u32>,
    #[serde(deserialize_with = "deser_flex_string")]
    pub identity: Option<String>,
    /// 听解部分的导入音（如 `audio/...-l-p1-intro.mp3`）
    pub audio: Option<String>,
    /// 信息检索题的资料（HTML，含表格）
    pub reference: Option<String>,
    pub questions: Vec<Question>,
}

/// 一道题的两种形态。
#[derive(Debug)]
pub enum Question {
    Leaf(Leaf),
    Group(Group),
    Null,
}

/// 单题：选项 + 答案。
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Leaf {
    pub title: Option<String>,
    pub question_type: Option<u32>,
    /// 答案下标（"0".."3"），对应 options 第 index+1 项
    #[serde(deserialize_with = "deser_opt_string")]
    pub answer: Option<String>,
    #[serde(deserialize_with = "deser_string_list")]
    pub options: Vec<String>,
    pub image: Option<String>,
    #[serde(deserialize_with = "deser_flex_string")]
    pub identity: Option<String>,
}

/// 组题：文章/听解原文 + 若干子题。
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Group {
    /// 阅读文章 / 题目原文（HTML）
    pub title: Option<String>,
    /// 听解原文（HTML，带 `<p data-starttime>` 时间戳）
    pub subtitle: Option<String>,
    pub question_type: Option<u32>,
    pub audio: Option<String>,
    pub image: Option<String>,
    #[serde(deserialize_with = "deser_flex_string")]
    pub identity: Option<String>,
    pub questions: Vec<Question>,
}

impl<'de> Deserialize<'de> for Question {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(d)?;
        if v.is_null() {
            return Ok(Question::Null);
        }
        let is_group = v.get("questions").map(|q| q.is_array()).unwrap_or(false);
        if is_group {
            Group::deserialize(v)
                .map(Question::Group)
                .map_err(serde::de::Error::custom)
        } else {
            Leaf::deserialize(v)
                .map(Question::Leaf)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl Question {
    #[allow(dead_code)]
    pub fn is_group(&self) -> bool {
        matches!(self, Question::Group(_))
    }

    #[allow(dead_code)]
    pub fn question_type(&self) -> Option<u32> {
        match self {
            Question::Leaf(l) => l.question_type,
            Question::Group(g) => g.question_type,
            Question::Null => None,
        }
    }
}

/// 题目大类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Vocabulary,
    Grammar,
    Reading,
    Listening,
    Other,
}

impl Category {
    pub fn label_zh(&self) -> &'static str {
        match self {
            Category::Vocabulary => "語彙",
            Category::Grammar => "文法",
            Category::Reading => "読解",
            Category::Listening => "聴解",
            Category::Other => "その他",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            Category::Vocabulary => "cat-vocab",
            Category::Grammar => "cat-grammar",
            Category::Reading => "cat-reading",
            Category::Listening => "cat-listening",
            Category::Other => "cat-other",
        }
    }
}

/// 题型 → 大类。
pub fn category_of(qt: u32) -> Category {
    match qt {
        10..=19 => Category::Vocabulary,
        20..=29 => Category::Grammar,
        30..=39 => Category::Reading,
        40..=49 => Category::Listening,
        _ => Category::Other,
    }
}

/// 题型 → 中文短标签。
pub fn type_label(qt: u32) -> Option<&'static str> {
    Some(match qt {
        11 => "漢字読み",
        12 => "漢字書き",
        13 | 14 => "文脈規定",
        15 => "言い換え類義",
        16 => "語彙用法",
        21 => "文法形式",
        22 => "文の組み立て",
        23 => "文章の文法",
        31 => "短文読解",
        32 => "中文読解",
        33 => "長文読解",
        34 => "統合理解",
        35 => "主張理解",
        36 | 37 => "情報検索",
        41 => "課題理解",
        42 => "ポイント理解",
        43 => "概要理解",
        44 => "即時応答",
        45 => "総合理解",
        _ => return None,
    })
}

impl Exam {
    /// 由 base 统计的题数（语彙+文法+読解+聴解）。
    #[allow(dead_code)]
    pub fn total_questions(&self) -> u32 {
        self.base.vocabulary_num.unwrap_or(0)
            + self.base.grammar_num.unwrap_or(0)
            + self.base.reading_num.unwrap_or(0)
            + self.base.listening_num.unwrap_or(0)
    }
}

/// 单题的答案展示文本，如 `1（つつんで）`；选项为空或与序号相同时只返回序号 `1`。
pub fn answer_label(leaf: &Leaf) -> Option<String> {
    let idx: usize = leaf.answer.as_deref()?.trim().parse().ok()?;
    let num = idx + 1;
    if let Some(opts) = leaf.options.get(idx) {
        let txt = opts.trim();
        if !txt.is_empty() && txt != num.to_string() {
            return Some(format!("{num}（{txt}）"));
        }
    }
    Some(num.to_string())
}
