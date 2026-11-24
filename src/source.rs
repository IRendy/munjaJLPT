//! 扫描并加载试卷 JSON。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::model::Exam;

/// 一张已加载的试卷。
pub struct ExamEntry {
    pub level: String,
    /// 试卷目录（JSON 所在目录，音频/图片在其 `audio/`、`images/` 子目录）
    pub dir: PathBuf,
    pub json_path: PathBuf,
    pub exam: Exam,
}

/// 扫描结果：成功加载的试卷。
pub struct Source {
    pub exams: Vec<ExamEntry>,
}

impl Source {
    /// 扫描 `data_dir/<level>/<folder>/*.json`（每张试卷一个 JSON）。
    ///
    /// 返回 `(Source, 失败清单)`；解析失败的试卷从 `exams` 中剔除，不中断整体。
    pub fn scan_and_load(data_dir: &Path, levels: &[String]) -> Result<(Self, Vec<(PathBuf, anyhow::Error)>)> {
        let mut exams = Vec::new();
        let mut failures = Vec::new();

        for lv in levels {
            let lv_dir = data_dir.join(lv);
            if !lv_dir.is_dir() {
                bail!("未找到级别目录: {}", lv_dir.display());
            }

            let mut folders: Vec<PathBuf> = fs::read_dir(&lv_dir)
                .with_context(|| format!("读取目录失败: {}", lv_dir.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect();
            folders.sort();

            for folder in folders {
                // 试卷 JSON 直接位于该目录下（文件名可能与目录名不一致，如 *_vietnam）
                let mut jsons: Vec<PathBuf> = fs::read_dir(&folder)
                    .with_context(|| format!("读取目录失败: {}", folder.display()))?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.is_file() && p.extension().map(|e| e == "json").unwrap_or(false))
                    .collect();
                jsons.sort();

                for json_path in jsons {
                    match load_exam(&json_path) {
                        Ok(exam) => exams.push(ExamEntry {
                            level: lv.clone(),
                            dir: folder.clone(),
                            json_path,
                            exam,
                        }),
                        Err(e) => failures.push((json_path, e)),
                    }
                }
            }
        }

        Ok((Source { exams }, failures))
    }
}

fn load_exam(path: &Path) -> Result<Exam> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("读取失败: {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("解析失败: {}", path.display()))
}

/// 删除各试卷同名的生成文件（.html / .md / .pdf）。
///
/// 只删「与试卷 JSON 同目录、同文件名」的三个扩展名，
/// 源 JSON、`audio/`、`images/` 一概不动。
/// 返回 (删除数量, 释放字节数)。
pub fn clean_generated(exams: &[ExamEntry], dry_run: bool, quiet: bool) -> (usize, u64) {
    let mut count = 0usize;
    let mut bytes = 0u64;

    for e in exams {
        let Some(stem) = e.json_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        for ext in ["html", "md", "pdf"] {
            let p = e.dir.join(format!("{stem}.{ext}"));
            if !p.is_file() {
                continue;
            }
            let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if dry_run {
                count += 1;
                bytes += len;
                if !quiet {
                    println!("  [计划删除] {}", p.display());
                }
            } else {
                match fs::remove_file(&p) {
                    Ok(()) => {
                        count += 1;
                        bytes += len;
                        if !quiet {
                            println!("  删除 {}", p.display());
                        }
                    }
                    Err(e) => eprintln!("  ! 删除失败 {}: {e}", p.display()),
                }
            }
        }
    }

    (count, bytes)
}

impl Source {
    /// 直接加载指定 JSON 文件（路径以 `.json` 结尾）。
    pub fn load_files(paths: &[PathBuf]) -> (Self, Vec<(PathBuf, anyhow::Error)>) {
        let mut exams = Vec::new();
        let mut failures = Vec::new();
        for p in paths {
            let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
            match load_exam(p) {
                Ok(exam) => exams.push(ExamEntry {
                    level: detect_level(p),
                    dir,
                    json_path: p.clone(),
                    exam,
                }),
                Err(e) => failures.push((p.clone(), e)),
            }
        }
        (Source { exams }, failures)
    }
}

/// 沿路径向上查找 N1–N5 级别名（如 `N3/2010-07_N3/x.json` → "N3"）。
fn detect_level(path: &Path) -> String {
    let mut cur = Some(path);
    while let Some(p) = cur {
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            for lv in ["N5", "N4", "N3", "N2", "N1"] {
                if name == lv || name.ends_with(&format!("_{lv}")) {
                    return lv.to_string();
                }
            }
        }
        cur = p.parent();
    }
    "?".to_string()
}
