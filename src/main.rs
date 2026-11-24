//! munja —— 整理 JLPT 试题：JSON → HTML / Markdown / PDF。

mod browser;
mod cli;
mod font;
mod model;
mod pipeline;
mod render;
mod source;
mod text;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let formats = cli.resolved_formats();

    // 1) 解析位置参数：.json 结尾的是试卷文件，其余是级别名
    let positional = &cli.levels;
    let json_files: Vec<std::path::PathBuf> = positional
        .iter()
        .filter(|s| s.ends_with(".json"))
        .map(std::path::PathBuf::from)
        .collect();
    let level_args: Vec<String> = positional
        .iter()
        .filter(|s| !s.ends_with(".json"))
        .cloned()
        .collect();
    let levels = if level_args.is_empty() && json_files.is_empty() {
        cli.effective_levels()
    } else {
        level_args
    };

    // 扫描并加载试卷（支持级别目录 + 直接指定 JSON 文件两种方式）
    let (mut source, mut load_errors) = source::Source::scan_and_load(&cli.data_dir, &levels)?;
    if !json_files.is_empty() {
        let (fsrc, ferr) = source::Source::load_files(&json_files);
        source.exams.extend(fsrc.exams);
        load_errors.extend(ferr);
    }
    if source.exams.is_empty() {
        anyhow::bail!("未找到任何试卷 JSON（数据根目录: {}）", cli.data_dir.display());
    }

    // 清理模式：删除各试卷生成的 html/md/pdf，然后退出
    if cli.clean {
        let (count, bytes) = source::clean_generated(&source.exams, cli.dry_run, cli.quiet);
        if !cli.quiet {
            if cli.dry_run {
                println!("清理计划：{} 个文件，共 {}", count, fmt_size(bytes));
            } else {
                println!("清理完成：删除 {} 个文件，释放 {}", count, fmt_size(bytes));
            }
        }
        return Ok(());
    }

    // 2) PDF 需要无头浏览器：提前定位，失败时给出明确提示
    let browser = if formats.contains(&cli::Format::Pdf) {
        Some(browser::BrowserLocator::locate(cli.browser.as_deref()).map_err(|e| {
            anyhow::anyhow!("生成 PDF 需要无头浏览器（Edge/Chrome/Chromium）：{e}")
        })?)
    } else {
        None
    };

    // 3) 定位日文字体（HTML @font-face 用；找不到则回退系统字体）
    let font_path = font::FontLocator::locate(cli.font.as_deref(), &cli.data_dir);

    // 4) 组装渲染器并执行
    let renderers = render::build_renderers(
        &formats,
        cli.answers,
        font_path.as_deref(),
        browser.as_ref(),
    );

    if !cli.quiet {
        let fmt_list = formats.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
        println!("试卷 {} 份 | 格式: {} | 答案: {}", source.exams.len(), fmt_list, cli.answers);
        if let Some(b) = &browser {
            println!("浏览器: {}（{}）", b.path.display(), b.kind.name());
        }
        match &font_path {
            Some(f) => println!("字体: {}", f.display()),
            None => println!("字体: 未找到（HTML 将回退系统字体）"),
        }
    }

    let rep = pipeline::Pipeline {
        source: &source,
        renderers,
        dry_run: cli.dry_run,
        quiet: cli.quiet,
    }
    .run();

    // 5) 报告
    for (path, err) in &rep.failed {
        eprintln!("  ! {}: {}", path.display(), err);
    }
    for (path, err) in &load_errors {
        eprintln!("  ! 解析失败 {}: {err:#}", path.display());
    }
    if !cli.quiet {
        if cli.dry_run {
            println!("计划完成：共 {} 个文件（未写入）", rep.planned);
        } else {
            println!(
                "完成：写入 {} 个文件，失败 {} 个",
                rep.files_written,
                rep.failed.len() + load_errors.len()
            );
        }
    }
    Ok(())
}

/// 把字节数格式化成可读的 KB / MB。
fn fmt_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
