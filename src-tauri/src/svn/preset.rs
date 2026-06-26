use std::path::Path;

use crate::errors::{AppError, AppResult};
use crate::models::{CapturePresetFile, PresetApplyPlan, PresetFile, PresetFragment};

// 片段前后各取多少行做定位锚点
const CONTEXT_LINES: usize = 3;

fn split_lines(text: &str) -> Vec<String> {
    // 统一去掉行尾 \r，写回时按目标文件原有的换行风格重组
    text.split('\n')
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect()
}

fn lines_eq(a: &str, b: &str) -> bool {
    // 行尾空白不参与比较，避免编辑器差异造成锚点失配
    a.trim_end() == b.trim_end()
}

fn seq_matches(haystack: &[String], at: usize, needle: &[String]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if at + needle.len() > haystack.len() {
        return false;
    }
    needle
        .iter()
        .enumerate()
        .all(|(i, n)| lines_eq(&haystack[at + i], n))
}

// 找出 needle 在 haystack 中的所有起始下标
fn find_all(haystack: &[String], needle: &[String]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    (0..=haystack.len() - needle.len())
        .filter(|&i| seq_matches(haystack, i, needle))
        .collect()
}

/// 从源文件内容捕获一个行片段，自动附带上下文锚点。
fn capture_fragment(lines: &[String], start: usize, end: usize) -> AppResult<PresetFragment> {
    if start == 0 || end < start || end > lines.len() {
        return Err(AppError::Other(format!(
            "行范围 {start}-{end} 超出文件行数 {}",
            lines.len()
        )));
    }
    let s = start - 1;
    let ctx_start = s.saturating_sub(CONTEXT_LINES);
    Ok(PresetFragment {
        start_line: start,
        end_line: end,
        lines: lines[s..end].to_vec(),
        context_before: lines[ctx_start..s].to_vec(),
        context_after: lines[end..(end + CONTEXT_LINES).min(lines.len())].to_vec(),
    })
}

/// 读取一个本地文件并按 spec 捕获为 PresetFile。ranges 为空表示整文件模式。
pub fn capture_file(root: &Path, spec: &CapturePresetFile) -> AppResult<PresetFile> {
    let path = Path::new(&spec.path);
    let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let rel = path
        .strip_prefix(root)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| {
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| spec.path.clone())
        });

    let lines = split_lines(&content);
    let mut fragments = Vec::new();
    // 按起始行排序后捕获，保证应用时从上到下处理
    let mut ranges = spec.ranges.clone();
    ranges.sort_by_key(|r| r[0]);
    for r in &ranges {
        fragments.push(capture_fragment(&lines, r[0], r[1])?);
    }

    Ok(PresetFile {
        rel_path: rel,
        content,
        fragments,
    })
}

// 片段在目标文件中的落点
struct FragmentSpot {
    start: usize, // 0-based 起始下标
    end: usize,   // 0-based 结束下标（不含）
}

/// 在目标文件行中定位片段应替换的区域。
/// 优先用前后文锚点（行号漂移也能命中）；多处命中时取离捕获行号最近的；
/// 锚点全部失配时，若捕获行号处的内容与片段一致（已应用过）也视为命中。
fn locate_fragment(target: &[String], frag: &PresetFragment) -> Option<FragmentSpot> {
    let before = &frag.context_before;
    let after = &frag.context_after;

    if !before.is_empty() {
        let mut candidates = Vec::new();
        for b in find_all(target, before) {
            let region_start = b + before.len();
            if after.is_empty() {
                // 没有后锚点（片段在文件尾）：替换区域取原片段行数
                candidates.push(FragmentSpot {
                    start: region_start,
                    end: (region_start + frag.lines.len()).min(target.len()),
                });
                continue;
            }
            // 在前锚点之后找最近的后锚点，两锚点之间即为待替换区域
            let mut a = region_start;
            while a + after.len() <= target.len() {
                if seq_matches(target, a, after) {
                    candidates.push(FragmentSpot {
                        start: region_start,
                        end: a,
                    });
                    break;
                }
                a += 1;
            }
        }
        if !candidates.is_empty() {
            // 多处命中取离捕获时行号最近的，降低同构配置块误中的概率
            let want = frag.start_line.saturating_sub(1);
            candidates.sort_by_key(|c| c.start.abs_diff(want));
            return candidates.into_iter().next();
        }
    } else if !after.is_empty() {
        // 片段在文件头：只用后锚点，区域为文件开头到锚点
        if let Some(a) = find_all(target, after).into_iter().next() {
            return Some(FragmentSpot { start: 0, end: a });
        }
    }

    // 锚点失配的兜底：目标文件里能原样找到片段内容（说明已应用过或位置未变）
    if let Some(i) = find_all(target, &frag.lines).into_iter().next() {
        return Some(FragmentSpot {
            start: i,
            end: i + frag.lines.len(),
        });
    }
    None
}

// 按文件名查找时跳过的目录：版本控制元数据与构建产物
const SKIP_DIRS: &[&str] = &[
    ".svn", ".git", ".idea", ".vscode", "node_modules", "target", "dist", "build", "out",
];

// 预设文件在目标工作副本中的落点解析结果
enum DestResolution {
    Exact(std::path::PathBuf),
    // (绝对路径, 相对 root 的展示路径)：按文件名在不同位置找到
    Relocated(std::path::PathBuf, String),
    NotFound,
    Ambiguous(Vec<String>),
}

fn collect_by_name(dir: &Path, name: &std::ffi::OsStr, out: &mut Vec<std::path::PathBuf>, depth: usize) {
    if depth > 16 || out.len() > 50 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let dn = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&dn) {
                continue;
            }
            collect_by_name(&p, name, out, depth + 1);
        } else if p.file_name() == Some(name) {
            out.push(p);
        }
    }
}

// 从尾部数起与预设相对路径相同的连续组件数，用来在多个同名文件里挑最像的
fn suffix_score(candidate_rel: &str, preset_rel: &str) -> usize {
    let a: Vec<&str> = candidate_rel.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = preset_rel.split('/').filter(|s| !s.is_empty()).collect();
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

/// 解析预设文件在目标工作副本中的实际落点。
/// 精确相对路径优先；不存在时按文件名全树查找——公共预设套到目录结构
/// 不同的项目（如 Java 包路径差异）时仍能定位。唯一最优命中才采用，
/// 平分的多个候选视为多义，交给用户处理。
fn resolve_dest(root: &Path, rel_path: &str) -> DestResolution {
    let exact = root.join(rel_path);
    if exact.is_file() {
        return DestResolution::Exact(exact);
    }
    let Some(name) = std::path::Path::new(rel_path).file_name() else {
        return DestResolution::NotFound;
    };
    let mut found = Vec::new();
    collect_by_name(root, name, &mut found, 0);
    if found.is_empty() {
        return DestResolution::NotFound;
    }

    let rel_of = |p: &std::path::Path| -> String {
        p.strip_prefix(root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| p.display().to_string())
    };
    let mut scored: Vec<(usize, std::path::PathBuf, String)> = found
        .into_iter()
        .map(|p| {
            let rel = rel_of(&p);
            (suffix_score(&rel, rel_path), p, rel)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let best = scored[0].0;
    let top: Vec<_> = scored.into_iter().filter(|s| s.0 == best).collect();
    if top.len() == 1 {
        let (_, p, rel) = top.into_iter().next().unwrap();
        DestResolution::Relocated(p, rel)
    } else {
        DestResolution::Ambiguous(top.into_iter().map(|(_, _, rel)| rel).collect())
    }
}

fn conflict_plan(file: &PresetFile, detail: String) -> PresetApplyPlan {
    PresetApplyPlan {
        rel_path: file.rel_path.clone(),
        action: "conflict".into(),
        detail,
        old_lines: Vec::new(),
        new_lines: file.fragments.iter().flat_map(|f| f.lines.clone()).collect(),
    }
}

/// 计算一个 PresetFile 应用到目标工作副本后的写入计划。
/// 返回 (计划, 写入落点与新内容)；conflict / unchanged 时为 None 表示不写盘。
pub fn plan_file(
    root: &Path,
    file: &PresetFile,
) -> (PresetApplyPlan, Option<(std::path::PathBuf, String)>) {
    // 先解析落点：精确路径 → 按文件名定位 → 不存在/多义
    let (dest, located_note) = match resolve_dest(root, &file.rel_path) {
        DestResolution::Exact(p) => (Some(p), String::new()),
        DestResolution::Relocated(p, rel) => (Some(p), format!("已按文件名定位到 {rel}；")),
        DestResolution::NotFound => (None, String::new()),
        DestResolution::Ambiguous(candidates) => {
            return (
                conflict_plan(
                    file,
                    format!("目标里有多个同名文件，无法确定落点：{}", candidates.join("、")),
                ),
                None,
            );
        }
    };

    // 整文件模式：覆盖已有文件；完全找不到时按预设相对路径新建
    if file.fragments.is_empty() {
        let Some(dest) = dest else {
            return (
                PresetApplyPlan {
                    rel_path: file.rel_path.clone(),
                    action: "create".into(),
                    detail: "目标不存在，新建文件".into(),
                    old_lines: Vec::new(),
                    new_lines: Vec::new(),
                },
                Some((root.join(&file.rel_path), file.content.clone())),
            );
        };
        return match std::fs::read_to_string(&dest) {
            Ok(existing) if existing == file.content => (
                PresetApplyPlan {
                    rel_path: file.rel_path.clone(),
                    action: "unchanged".into(),
                    detail: format!("{located_note}内容已一致，无需写入"),
                    old_lines: Vec::new(),
                    new_lines: Vec::new(),
                },
                None,
            ),
            Ok(_) => (
                PresetApplyPlan {
                    rel_path: file.rel_path.clone(),
                    action: "overwrite".into(),
                    detail: format!("{located_note}整文件覆盖"),
                    old_lines: Vec::new(),
                    new_lines: Vec::new(),
                },
                Some((dest, file.content.clone())),
            ),
            Err(e) => (
                conflict_plan(file, format!("读取目标文件失败：{e}")),
                None,
            ),
        };
    }

    // 片段模式：目标文件必须存在（精确路径或按文件名定位到）
    let Some(dest) = dest else {
        return (
            conflict_plan(file, "目标文件不存在（按文件名也未找到），无法做行替换".into()),
            None,
        );
    };
    let existing = match std::fs::read_to_string(&dest) {
        Ok(t) => t,
        Err(e) => {
            return (conflict_plan(file, format!("读取目标文件失败：{e}")), None);
        }
    };

    let crlf = existing.contains("\r\n");
    let mut lines = split_lines(&existing);
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut replaced = 0usize;
    let mut missed = Vec::new();

    // 从下往上替换，避免前面的替换让后面片段的下标失效
    let mut spots: Vec<(usize, FragmentSpot)> = Vec::new();
    for (idx, frag) in file.fragments.iter().enumerate() {
        match locate_fragment(&lines, frag) {
            Some(spot) => spots.push((idx, spot)),
            None => missed.push(frag.start_line),
        }
    }
    spots.sort_by(|a, b| b.1.start.cmp(&a.1.start));
    for (idx, spot) in &spots {
        let frag = &file.fragments[*idx];
        let current = &lines[spot.start..spot.end];
        if current.len() == frag.lines.len()
            && current.iter().zip(&frag.lines).all(|(a, b)| lines_eq(a, b))
        {
            continue; // 已是预设内容
        }
        old_lines.extend(current.iter().cloned());
        new_lines.extend(frag.lines.iter().cloned());
        lines.splice(spot.start..spot.end, frag.lines.iter().cloned());
        replaced += 1;
    }

    if !missed.is_empty() {
        return (
            PresetApplyPlan {
                rel_path: file.rel_path.clone(),
                action: "conflict".into(),
                detail: format!(
                    "有 {} 个片段在目标文件中找不到落点（捕获时起始行：{}），文件未改动",
                    missed.len(),
                    missed
                        .iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                old_lines,
                new_lines,
            },
            None,
        );
    }

    if replaced == 0 {
        return (
            PresetApplyPlan {
                rel_path: file.rel_path.clone(),
                action: "unchanged".into(),
                detail: format!("{located_note}所有片段已是预设内容"),
                old_lines: Vec::new(),
                new_lines: Vec::new(),
            },
            None,
        );
    }

    let sep = if crlf { "\r\n" } else { "\n" };
    let new_content = lines.join(sep);
    (
        PresetApplyPlan {
            rel_path: file.rel_path.clone(),
            action: "patch".into(),
            detail: format!("{located_note}替换 {replaced} 处行片段"),
            old_lines,
            new_lines,
        },
        Some((dest, new_content)),
    )
}

/// 对整个预设做应用规划；dry_run=false 时把可写的文件落盘。
pub fn apply_preset(
    root: &Path,
    files: &[PresetFile],
    dry_run: bool,
) -> AppResult<Vec<PresetApplyPlan>> {
    if !root.is_dir() {
        return Err(AppError::InvalidPath(root.display().to_string()));
    }
    let mut plans = Vec::new();
    for file in files {
        let (plan, write) = plan_file(root, file);
        if !dry_run {
            if let Some((dest, content)) = write {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(AppError::Io)?;
                }
                std::fs::write(&dest, content).map_err(AppError::Io)?;
            }
        }
        plans.push(plan);
    }
    Ok(plans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag(lines: &[&str], start: usize, before: &[&str], after: &[&str]) -> PresetFragment {
        PresetFragment {
            start_line: start,
            end_line: start + lines.len() - 1,
            lines: lines.iter().map(|s| s.to_string()).collect(),
            context_before: before.iter().map(|s| s.to_string()).collect(),
            context_after: after.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn to_lines(s: &str) -> Vec<String> {
        split_lines(s)
    }

    #[test]
    fn locate_by_context_with_line_drift() {
        // 目标文件在片段上方插入了两行，行号漂移后仍按锚点命中
        let target = to_lines("a\nb\nx1\nx2\nkey: old\nc\nd");
        let f = frag(&["key: dev"], 3, &["a", "b"], &["c", "d"]);
        let spot = locate_fragment(&target, &f).expect("应能命中");
        assert_eq!((spot.start, spot.end), (2, 5));
    }

    #[test]
    fn locate_fragment_already_applied() {
        let target = to_lines("a\nkey: dev\nc");
        let f = frag(&["key: dev"], 2, &["zzz"], &["yyy"]);
        let spot = locate_fragment(&target, &f).expect("内容兜底应命中");
        assert_eq!((spot.start, spot.end), (1, 2));
    }

    #[test]
    fn locate_fragment_conflict() {
        let target = to_lines("completely\ndifferent\nfile");
        let f = frag(&["key: dev"], 2, &["a"], &["c"]);
        assert!(locate_fragment(&target, &f).is_none());
    }

    #[test]
    fn plan_patches_multiple_fragments() {
        let dir = std::env::temp_dir().join(format!("preset-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.yml"), "h1\nurl: prod\nh2\nh3\npwd: prod\ntail\n").unwrap();

        let file = PresetFile {
            rel_path: "app.yml".into(),
            content: String::new(),
            fragments: vec![
                frag(&["url: dev"], 2, &["h1"], &["h2"]),
                frag(&["pwd: dev"], 5, &["h3"], &["tail"]),
            ],
        };
        let (plan, write) = plan_file(&dir, &file);
        assert_eq!(plan.action, "patch");
        let (dest, content) = write.expect("应有写入计划");
        assert_eq!(dest, dir.join("app.yml"));
        assert_eq!(content, "h1\nurl: dev\nh2\nh3\npwd: dev\ntail\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn relocates_by_file_name_when_rel_path_differs() {
        // 预设来自项目 A 的包路径，应用到项目 B：同名文件在不同目录下，按文件名定位
        let dir = std::env::temp_dir().join(format!("preset-reloc-{}", std::process::id()));
        let nested = dir.join("src/main/java/com/b/projb");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Constants.java"), "a\nflag = prod;\nb\n").unwrap();

        let file = PresetFile {
            rel_path: "src/main/java/com/a/proja/Constants.java".into(),
            content: String::new(),
            fragments: vec![frag(&["flag = dev;"], 2, &["a"], &["b"])],
        };
        let (plan, write) = plan_file(&dir, &file);
        assert_eq!(plan.action, "patch");
        assert!(plan.detail.contains("已按文件名定位到"), "detail: {}", plan.detail);
        let (dest, content) = write.expect("应有写入计划");
        assert_eq!(dest, nested.join("Constants.java"));
        assert_eq!(content, "a\nflag = dev;\nb\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ambiguous_same_name_files_conflict() {
        let dir = std::env::temp_dir().join(format!("preset-ambig-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("m1")).unwrap();
        std::fs::create_dir_all(dir.join("m2")).unwrap();
        std::fs::write(dir.join("m1/app.yml"), "x\n").unwrap();
        std::fs::write(dir.join("m2/app.yml"), "x\n").unwrap();

        let file = PresetFile {
            rel_path: "conf/app.yml".into(),
            content: String::new(),
            fragments: vec![frag(&["k: v"], 1, &[], &["x"])],
        };
        let (plan, write) = plan_file(&dir, &file);
        assert_eq!(plan.action, "conflict");
        assert!(plan.detail.contains("多个同名文件"), "detail: {}", plan.detail);
        assert!(write.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
