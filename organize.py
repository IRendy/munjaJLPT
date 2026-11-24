#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
整理 N3 / N4 / N5 的题目，为每个考试生成同目录下的 .html 与 .md 文件。

HTML 内嵌音频(<audio>)与文本(听解原文/阅读文章)，答案用 <details> 折叠，不含 JS。
Markdown 中听解部分列出音频相对路径 + 完整听写文本，答案同样用 <details> 折叠。

用法:
    python3 organize.py                 # 处理 exam/ 下的 N3 N4 N5
    python3 organize.py N3 N4           # 只处理指定级别
    python3 organize.py --data-dir exam N5   # 指定数据根目录
"""

import glob
import html
import json
import os
import re
import sys

# 数据根目录：试卷数据集中在 exam/ 下（与源码分离）
DATA_DIR = os.environ.get("MUNJA_DATA_DIR", "exam")

LEVELS = ["N3", "N4", "N5"]
# LEVELS = ["N1", "N2", "N3", "N4", "N5"]

# 题型 → 简短标签
TYPE_LABELS = {
    11: "漢字読み",
    12: "漢字書き",
    13: "文脈規定",
    14: "文脈規定",
    15: "言い換え類義",
    16: "語彙用法",
    21: "文法形式",
    22: "文の組み立て",
    23: "文章の文法",
    31: "短文読解",
    32: "中文読解",
    33: "長文読解",
    34: "統合理解",
    35: "主張理解",
    36: "情報検索",
    37: "情報検索",
    41: "課題理解",
    42: "ポイント理解",
    43: "概要理解",
    44: "即時応答",
    45: "総合理解",
}


def category(qt):
    if 10 <= qt < 20:
        return "語彙", "cat-vocab"
    if 20 <= qt < 30:
        return "文法", "cat-grammar"
    if 30 <= qt < 40:
        return "読解", "cat-reading"
    if 40 <= qt < 50:
        return "聴解", "cat-listening"
    return "その他", "cat-other"


# ---------------------------------------------------------------------------
# 文本转换工具
# ---------------------------------------------------------------------------


def clean_html(s):
    """把内容字段(已经是 HTML)清理成可内嵌的 HTML：去掉 class/data 属性、
    去掉制表符缩进、把 &nbsp; 换成空格。保留 <u> <br> <table> 等标签。"""
    if not s:
        return ""
    s = re.sub(r'\s+class="[^"]*"', "", s)
    s = re.sub(r'\s+data-(?:starttime|endtime)="[^"]*"', "", s)
    s = s.replace("\t", "")
    s = re.sub(r"&nbsp;", " ", s)
    return s


def html_to_md(s):
    """把内容 HTML 转成 markdown 文本：<br>/</p> 换行、<u> 转粗体、
    表格粗转为竖线分隔行、去其余标签、反转义。"""
    if not s:
        return ""
    t = s
    t = re.sub(r"<br\s*/?>", "\n", t, flags=re.I)
    t = re.sub(r"</p\s*>", "\n\n", t, flags=re.I)
    t = re.sub(r"</tr\s*>", "\n", t, flags=re.I)
    t = re.sub(r"<(td|th)[^>]*>", " ", t, flags=re.I)
    t = re.sub(r"</(td|th)\s*>", " | ", t, flags=re.I)
    t = re.sub(r"<u>", "**", t, flags=re.I)
    t = re.sub(r"</u>", "**", t, flags=re.I)
    t = re.sub(r"<[^>]+>", "", t)
    t = html.unescape(t)
    t = t.replace("\xa0", " ")
    t = re.sub(r"(?m)^[ \t]+", "", t)  # 去掉行首的制表符缩进
    t = re.sub(r"[ \t]+\n", "\n", t)
    t = re.sub(r"\n{3,}", "\n\n", t)
    return t.strip()


def esc_md(text):
    """markdown 中纯文本字段的轻量转义。"""
    if text is None:
        return ""
    text = str(text).strip()
    text = text.replace("|", "\\|")
    return text


# ---------------------------------------------------------------------------
# 答案
# ---------------------------------------------------------------------------


def answer_text(q):
    """返回答案显示文本，如 '1（つつんで）' 或 '3'。"""
    try:
        idx = int(q.get("answer"))
    except (TypeError, ValueError):
        return ""
    opts = q.get("options") or []
    num = idx + 1
    if opts and idx < len(opts):
        txt = str(opts[idx]).strip()
        if txt and txt != str(num):
            return f"{num}（{txt}）"
    return str(num)


# ---------------------------------------------------------------------------
# HTML 渲染
# ---------------------------------------------------------------------------


def render_options_html(opts):
    if not opts:
        return ""
    items = "".join(f"<li>{html.escape(str(o).strip())}</li>" for o in opts)
    return f'<ol class="options">{items}</ol>'


def render_answer_html(q):
    ans = answer_text(q)
    if not ans:
        return ""
    return (
        f"<details><summary>答案</summary>"
        f'<div class="answer">答え：{html.escape(ans)}</div></details>'
    )


def render_audio_html(src):
    if not src:
        return ""
    return (
        f'<audio controls preload="none" src="{html.escape(src, quote=True)}"></audio>'
    )


def render_image_html(src):
    if not src:
        return ""
    return f'<img src="{html.escape(src, quote=True)}" alt="図">'


def render_script_html(subtitle):
    if not subtitle:
        return ""
    body = clean_html(subtitle)
    return f'<div class="script">{body}</div>'


def render_question_html(q, num_label, idx):
    """渲染一个叶子题。num_label 形如 '1.' 或 '1番'。"""
    parts = []
    title = q.get("title")
    if title:
        parts.append(f'<div class="q-title">{clean_html(title)}</div>')
    img = q.get("image")
    if img:
        parts.append(render_image_html(img))
    parts.append(render_options_html(q.get("options")))
    parts.append(render_answer_html(q))
    head = f'<span class="q-num">{num_label}</span>'
    return f'<div class="q">{head}' + "".join(parts) + "</div>"


def render_section_html(section):
    qt = section.get("questionType")
    cat, cat_cls = category(qt)
    label = TYPE_LABELS.get(qt, "")

    header = f'<span class="cat {cat_cls}">{cat}</span>'
    if label:
        header += f'<span class="cat-label">{label}</span>'
    h2 = f"<h2>{header}</h2>"

    instr = clean_html(section.get("title"))
    instr_html = f'<div class="instruction">{instr}</div>' if instr else ""

    sec_audio = render_audio_html(section.get("audio"))
    reference = section.get("reference")
    ref_html = (
        f'<div class="reference">{clean_html(reference)}</div>' if reference else ""
    )

    body = [h2, instr_html, sec_audio, ref_html]

    num = 0
    for q in section.get("questions") or []:
        if q is None:
            continue
        if isinstance(q.get("questions"), list):
            num += 1
            is_listening = bool(q.get("audio") or q.get("subtitle"))
            block = ['<div class="q group">']
            nlabel = f"{num}番" if is_listening else f"{num}."
            block.append(f'<span class="q-num">{nlabel}</span>')
            if q.get("audio"):
                block.append(render_audio_html(q["audio"]))
            if q.get("subtitle"):
                block.append(render_script_html(q["subtitle"]))
            if q.get("title"):
                block.append(f'<div class="q-passage">{clean_html(q["title"])}</div>')
            if q.get("image"):
                block.append(render_image_html(q["image"]))
            # 子题
            for si, sq in enumerate(q["questions"] or []):
                if sq is None:
                    continue
                block.append(f'<div class="subq"><span class="q-num">問{si + 1}</span>')
                if sq.get("title"):
                    block.append(
                        f'<div class="q-title">{clean_html(sq["title"])}</div>'
                    )
                if sq.get("image"):
                    block.append(render_image_html(sq["image"]))
                block.append(render_options_html(sq.get("options")))
                block.append(render_answer_html(sq))
                block.append("</div>")
            block.append("</div>")
            body.append("".join(block))
        else:
            num += 1
            body.append(render_question_html(q, f"{num}.", num))

    return f'<section class="sec sec-{cat_cls}">' + "".join(body) + "</section>"


CSS = """
:root{--ink:#1f2430;--muted:#6b7280;--accent:#c2410c;--bg:#faf9f6;--card:#fff;--line:#e7e3da;}
*{box-sizing:border-box;}
body{font-family:"Hiragino Kaku Gothic ProN","Yu Gothic","Noto Sans CJK JP","Meiryo",sans-serif;
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
.script{background:#f6f4ef;border-radius:8px;padding:12px 16px;font-size:.94em;margin:8px 0;}
.script p{margin:5px 0;}
img{max-width:100%;border:1px solid var(--line);border-radius:8px;margin:8px 0;}
.subq{margin:12px 0 12px 14px;padding-left:12px;border-left:3px solid var(--line);}
details{margin-top:10px;}
summary{cursor:pointer;color:var(--muted);font-size:.88em;font-weight:600;}
.answer{color:#047857;font-weight:700;margin-top:4px;}
"""


def build_html(level, exam):
    base = exam["base"]
    title = f"{base.get('title', '')} {level}" if base.get("title") else level
    duration = base.get("duration") or {}
    chips = []
    for key, name in [
        ("vocabularyNum", "語彙"),
        ("grammarNum", "文法"),
        ("readingNum", "読解"),
        ("listeningNum", "聴解"),
    ]:
        if base.get(key) is not None:
            chips.append(f"{name} {base[key]}問")
    if duration.get("languageKnowledgeReading"):
        chips.append(f"言語知識・読解 {duration['languageKnowledgeReading']}分")
    if duration.get("listening"):
        chips.append(f"聴解 {duration['listening']}分")

    chips_html = "".join(f'<span class="chip">{html.escape(c)}</span>' for c in chips)
    full_audio = render_audio_html(base.get("audio"))

    sections_html = "".join(render_section_html(s) for s in exam.get("sections") or [])

    return f"""<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>日本語能力試験 {html.escape(title)}</title>
<style>{CSS}</style>
</head>
<body>
<header>
<h1>日本語能力試験 {html.escape(title)}</h1>
<div class="meta">
<div class="chips">{chips_html}</div>
{full_audio}
</div>
</header>
<main>
{sections_html}
</main>
</body>
</html>
"""


# ---------------------------------------------------------------------------
# Markdown 渲染
# ---------------------------------------------------------------------------


def render_options_md(opts):
    if not opts:
        return ""
    return "\n".join(f"- {i + 1}. {esc_md(o)}" for i, o in enumerate(opts))


def render_answer_md(q):
    ans = answer_text(q)
    if not ans:
        return ""
    return f"<details><summary>答案</summary>{esc_md(ans)}</details>"


def render_question_md(q, num_label):
    lines = []
    if q.get("title"):
        lines.append(f"**{num_label}** {html_to_md(q['title'])}")
    else:
        lines.append(f"**{num_label}**")
    if q.get("image"):
        lines.append(f"![図]({q['image']})")
    opts = render_options_md(q.get("options"))
    if opts:
        lines.append(opts)
    ans = render_answer_md(q)
    if ans:
        lines.append(ans)
    return "\n\n".join(lines)


def render_section_md(section):
    qt = section.get("questionType")
    cat, _ = category(qt)
    label = TYPE_LABELS.get(qt, "")
    heading = f"{cat}"
    if label:
        heading += f"・{label}"
    lines = [f"## {heading}", ""]
    instr = html_to_md(section.get("title"))
    if instr:
        lines.append(f"> {instr}")
        lines.append("")
    if section.get("audio"):
        lines.append(f"🎧 導入音声：[{section['audio']}]({section['audio']})")
        lines.append("")
    if section.get("reference"):
        lines.append(html_to_md(section["reference"]))
        lines.append("")

    num = 0
    for q in section.get("questions") or []:
        if q is None:
            continue
        if isinstance(q.get("questions"), list):
            num += 1
            is_listening = bool(q.get("audio") or q.get("subtitle"))
            nlabel = f"{num}番" if is_listening else f"{num}."
            lines.append(f"### {nlabel}")
            if q.get("audio"):
                lines.append(f"🎧 [{q['audio']}]({q['audio']})")
            if q.get("subtitle"):
                lines.append(html_to_md(q["subtitle"]))
            if q.get("title"):
                lines.append(html_to_md(q["title"]))
            if q.get("image"):
                lines.append(f"![図]({q['image']})")
            for si, sq in enumerate(q["questions"] or []):
                if sq is None:
                    continue
                lines.append("")
                if sq.get("title"):
                    lines.append(f"**問{si + 1}** {html_to_md(sq['title'])}")
                else:
                    lines.append(f"**問{si + 1}**")
                if sq.get("image"):
                    lines.append(f"![図]({sq['image']})")
                opts = render_options_md(sq.get("options"))
                if opts:
                    lines.append(opts)
                ans = render_answer_md(sq)
                if ans:
                    lines.append(ans)
        else:
            num += 1
            lines.append(render_question_md(q, f"{num}."))
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def build_md(level, exam):
    base = exam["base"]
    title = f"{base.get('title', '')} {level}" if base.get("title") else level
    lines = [f"# 日本語能力試験 {title}", ""]
    meta = []
    for key, name in [
        ("vocabularyNum", "語彙"),
        ("grammarNum", "文法"),
        ("readingNum", "読解"),
        ("listeningNum", "聴解"),
    ]:
        if base.get(key) is not None:
            meta.append(f"{name} {base[key]}問")
    duration = base.get("duration") or {}
    if duration.get("languageKnowledgeReading"):
        meta.append(f"言語知識・読解 {duration['languageKnowledgeReading']}分")
    if duration.get("listening"):
        meta.append(f"聴解 {duration['listening']}分")
    if meta:
        lines.append("・".join(meta))
        lines.append("")
    if base.get("audio"):
        lines.append(f"🎧 聴解全文：[{base['audio']}]({base['audio']})")
        lines.append("")

    for s in exam.get("sections") or []:
        lines.append(render_section_md(s))
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------


def process(level, data_dir):
    pattern = os.path.join(data_dir, level, "*", "*.json")
    files = sorted(glob.glob(pattern))
    stats = {"html": 0, "md": 0, "questions": 0, "skipped": 0}
    for path in files:
        try:
            with open(path, encoding="utf-8") as f:
                exam = json.load(f)
        except Exception as e:
            print(f"  ! 读取失败 {path}: {e}")
            stats["skipped"] += 1
            continue

        base, _ = os.path.splitext(path)
        html_path = base + ".html"
        md_path = base + ".md"

        with open(html_path, "w", encoding="utf-8") as f:
            f.write(build_html(level, exam))
        with open(md_path, "w", encoding="utf-8") as f:
            f.write(build_md(level, exam))

        stats["html"] += 1
        stats["md"] += 1
        stats["questions"] += (
            exam["base"].get("listeningNum", 0)
            + exam["base"].get("readingNum", 0)
            + exam["base"].get("grammarNum", 0)
            + exam["base"].get("vocabularyNum", 0)
        )
        print(f"  ✓ {html_path}")
        print(f"  ✓ {md_path}")
    return stats


def main():
    args = sys.argv[1:]
    data_dir = DATA_DIR
    if "--data-dir" in args:
        i = args.index("--data-dir")
        if i + 1 >= len(args):
            print("--data-dir 需要一个路径参数")
            sys.exit(2)
        data_dir = args[i + 1]
        del args[i : i + 2]
    levels = [a.upper() for a in args if not a.startswith("-")]

    if not os.path.isdir(data_dir):
        print(f"数据根目录不存在: {data_dir}")
        sys.exit(1)

    if not levels:
        levels = LEVELS
    levels = [l for l in levels if l in LEVELS]
    if not levels:
        print(f"未知的级别。可用: {', '.join(LEVELS)}")
        sys.exit(1)

    total = {"html": 0, "md": 0, "questions": 0, "skipped": 0}
    for lv in levels:
        print(f"处理 {lv} …")
        st = process(lv, data_dir)
        for k in total:
            total[k] += st[k]
    print("-" * 40)
    print(
        f"完成：生成 HTML {total['html']} 份、Markdown {total['md']} 份，"
        f"共约 {total['questions']} 题。跳过 {total['skipped']} 个文件。"
    )


if __name__ == "__main__":
    main()
