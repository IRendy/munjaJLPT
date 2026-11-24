//! 调度流水线：试卷 × 渲染器，逐份写出目标文件。

use std::path::PathBuf;

use crate::render::{RenderContext, Renderer};
use crate::source::Source;

/// 执行报告。
#[derive(Default)]
pub struct PipelineReport {
    /// dry-run 下计划生成的文件数
    pub planned: usize,
    /// 实际写入的文件数
    pub files_written: usize,
    /// 失败清单：(目标路径, 错误信息)
    pub failed: Vec<(PathBuf, String)>,
}

pub struct Pipeline<'a> {
    pub source: &'a Source,
    pub renderers: Vec<Box<dyn Renderer>>,
    pub dry_run: bool,
    pub quiet: bool,
}

impl Pipeline<'_> {
    pub fn run(&self) -> PipelineReport {
        let mut rep = PipelineReport::default();

        for entry in &self.source.exams {
            let stem = entry
                .json_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");

            let ctx = RenderContext {
                level: &entry.level,
                out_dir: &entry.dir,
            };

            for r in &self.renderers {
                let out_path = entry.dir.join(format!("{stem}.{}", r.extension()));
                if self.dry_run {
                    rep.planned += 1;
                    if !self.quiet {
                        println!(
                            "  [计划] {} → {}（{}）",
                            entry.json_path.display(),
                            out_path.display(),
                            r.format(),
                        );
                    }
                } else {
                    match r.render(&ctx, &entry.exam, &out_path) {
                        Ok(p) => {
                            rep.files_written += 1;
                            if !self.quiet {
                                println!("  ✓ {}", p.display());
                            }
                        }
                        Err(e) => rep.failed.push((out_path, format!("{e:#}"))),
                    }
                }
            }
        }

        rep
    }
}
