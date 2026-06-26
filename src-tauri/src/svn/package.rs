use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult};
use crate::svn::info::svn_info;
use crate::svn::log::{svn_log, LogOptions};
use crate::svn::runner::run_svn;

// 资源类后缀（编译后落到 WEB-INF/classes，按原相对路径搬运）
const RESOURCE_EXTS: &[&str] = &[
    ".xml",
    ".properties",
    ".yml",
    ".yaml",
    ".json",
    ".ftl",
    ".vm",
    ".tld",
    ".dtd",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageOptions {
    pub requirement_name: String,
    pub requirement_desc: String,
    #[serde(default)]
    pub has_db: bool,
    #[serde(default)]
    pub has_url: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRevision {
    pub revision: u64,
    pub author: Option<String>,
    pub date: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageBuildResult {
    pub base_dir: String,
    pub front_dir: String,
    pub final_rest_dir: String,
    pub version: String,
    pub copied_count: usize,
    pub not_found: Vec<String>,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageZipResult {
    pub zip_path: String,
    pub size: u64,
}

fn source_url(svn_bin: &str, rest_path: &str) -> Option<String> {
    svn_info(svn_bin, rest_path).ok().map(|i| i.url)
}

/// 拉取 rest 模块最近若干条提交（含变更路径），供选择打包版本。
pub fn fetch_package_revisions(
    svn_bin: &str,
    rest_path: &str,
    limit: u32,
) -> AppResult<Vec<PackageRevision>> {
    let url = source_url(svn_bin, rest_path).unwrap_or_else(|| rest_path.to_string());
    let entries = svn_log(
        svn_bin,
        &LogOptions {
            target: &url,
            limit,
            revision_range: Some("HEAD:1"),
            search: None,
            author: None,
            date_from: None,
            date_to: None,
            with_paths: false,
        },
    )?;
    Ok(entries
        .into_iter()
        .map(|e| PackageRevision {
            revision: e.revision,
            author: e.author,
            date: e.date,
            message: e
                .message
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "(无描述)".into()),
        })
        .collect())
}

// 收集选中版本涉及的所有仓库变更路径
fn changed_paths_for_revisions(
    svn_bin: &str,
    rest_path: &str,
    revisions: &[u64],
) -> AppResult<Vec<String>> {
    if revisions.is_empty() {
        return Ok(Vec::new());
    }
    let url = source_url(svn_bin, rest_path).unwrap_or_else(|| rest_path.to_string());
    let min = revisions.iter().min().copied().unwrap();
    let max = revisions.iter().max().copied().unwrap();
    let range = format!("{min}:{max}");
    let entries = svn_log(
        svn_bin,
        &LogOptions {
            target: &url,
            limit: 1000,
            revision_range: Some(&range),
            search: None,
            author: None,
            date_from: None,
            date_to: None,
            with_paths: true,
        },
    )?;
    let set: std::collections::HashSet<u64> = revisions.iter().copied().collect();
    let mut paths = Vec::new();
    for e in entries {
        if !set.contains(&e.revision) {
            continue;
        }
        for p in e.paths {
            if matches!(p.action.as_str(), "A" | "M" | "R") {
                paths.push(p.path);
            }
        }
    }
    Ok(paths)
}

// 把源码仓库路径映射成编译产物里的相对路径。返回 (相对路径, 是否 java 类)
fn normalize_svn_path(svn_path: &str) -> Option<(String, bool)> {
    if !svn_path.contains("/rest/src") && !svn_path.contains("/rest/") {
        return None;
    }
    let rest_idx = svn_path.find("/rest")?;
    let after = &svn_path[rest_idx..];

    if after.ends_with(".java") {
        let rel = after.replace("/rest/src/main/java", "/rest/WEB-INF/classes");
        let rel = format!("{}.class", &rel[..rel.len() - 5]);
        return Some((rel, true));
    }
    if RESOURCE_EXTS.iter().any(|ext| after.ends_with(ext)) {
        let rel = after
            .replace("/rest/src/main/resources", "/rest/WEB-INF/classes")
            .replace("/rest/src/main/java", "/rest/WEB-INF/classes");
        return Some((rel, false));
    }
    if after.contains("/rest/src/main/") {
        let rel = after
            .replace("/rest/src/main/resources", "/rest/WEB-INF/classes")
            .replace("/rest/src/main/java", "/rest/WEB-INF/classes");
        return Some((rel, false));
    }
    Some((after.to_string(), false))
}

struct PackageDirs {
    base_dir: PathBuf,
    rest_dir: PathBuf,
    copy_dir: PathBuf,
    front_dir: PathBuf,
}

// 创建提供包目录结构 + 说明文档
fn create_package_structure(
    project_root: &Path,
    opts: &PackageOptions,
) -> Result<PackageDirs, String> {
    let project_name = project_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("项目")
        .to_string();
    let base_dir = project_root.join("提供包").join(&opts.requirement_name);
    let code_pkg = base_dir.join("后端").join("代码包");
    let rest_dir = code_pkg.join("rest");
    let copy_dir = code_pkg.join("copy");
    let front_dir = base_dir.join("前端");
    for d in [&rest_dir, &copy_dir, &front_dir] {
        fs::create_dir_all(d).map_err(|e| format!("创建目录失败：{e}"))?;
    }

    let desc = if opts.requirement_desc.trim().is_empty() {
        opts.requirement_name.clone()
    } else {
        opts.requirement_desc.clone()
    };
    let mut lines = vec![
        "【适用版本】".to_string(),
        format!("\t{project_name}"),
        "【需求】".to_string(),
        format!("\t{desc}"),
        "【操作】".to_string(),
        "\t1.备份原来的rest.war包和rest文件夹".to_string(),
        String::new(),
    ];
    let mut step = 2;
    if opts.has_db {
        let db_dir = base_dir.join("后端").join("数据库");
        fs::create_dir_all(&db_dir).map_err(|e| format!("创建数据库目录失败：{e}"))?;
        for sql in ["DDL.sql", "DML.sql"] {
            let p = db_dir.join(sql);
            if !p.exists() {
                let _ = fs::write(&p, "-- 在此编写 SQL 语句\n");
            }
        }
        lines.push(format!("\t{step}.在数据库执行 /后端/数据库/"));
        step += 1;
    }
    lines.push(format!(
        "\t{step}.将/后端/代码包下的rest包替换到服务器上（该包为增量包）\n"
    ));
    step += 1;
    lines.push(format!("\t{step}.启动服务，登陆档案系统验证"));
    step += 1;
    if opts.has_url {
        lines.push(format!("\t{step}.在浏览器执行URL:"));
    }
    let doc_path = base_dir.join("后端").join("说明文档.txt");
    fs::write(&doc_path, lines.join("\n")).map_err(|e| format!("写说明文档失败：{e}"))?;

    Ok(PackageDirs {
        base_dir,
        rest_dir,
        copy_dir,
        front_dir,
    })
}

// 找 Maven 编译产物：target 下最新的 *-SNAPSHOT（含 WEB-INF），否则 target/classes
fn find_maven_exploded_war(rest_path: &Path) -> Result<PathBuf, String> {
    let target = rest_path.join("target");
    if !target.is_dir() {
        return Err("target 目录不存在，请先在 IDEA 运行 mvn package".into());
    }
    let mut snapshots: Vec<PathBuf> = fs::read_dir(&target)
        .map_err(|e| format!("读取 target 失败：{e}"))?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n.contains("-SNAPSHOT"))
                    .unwrap_or(false)
        })
        .collect();
    snapshots.sort();
    snapshots.reverse();
    for s in snapshots {
        if s.join("WEB-INF").is_dir() {
            return Ok(s);
        }
    }
    let classes = target.join("classes");
    if classes.exists() {
        return Ok(classes);
    }
    Err("未找到 Maven 编译产物，请先在 IDEA 中运行 mvn package".into())
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn clear_dir(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_dir() {
            fs::remove_dir_all(&p)?;
        } else {
            fs::remove_file(&p)?;
        }
    }
    Ok(())
}

// 把 Maven 产物拷进 rest_dir：有 WEB-INF 直接搬，否则把 classes 包进 WEB-INF/classes
fn copy_maven_output(maven_source: &Path, rest_dir: &Path) -> Result<(), String> {
    clear_dir(rest_dir).map_err(|e| format!("清理 rest 目录失败：{e}"))?;
    let web_inf = maven_source.join("WEB-INF");
    if web_inf.is_dir() {
        for entry in fs::read_dir(maven_source).map_err(|e| format!("读取产物失败：{e}"))? {
            let entry = entry.map_err(|e| format!("读取产物项失败：{e}"))?;
            let from = entry.path();
            let to = rest_dir.join(entry.file_name());
            if from.is_dir() {
                copy_dir_all(&from, &to).map_err(|e| format!("拷贝目录失败：{e}"))?;
            } else {
                fs::copy(&from, &to).map_err(|e| format!("拷贝文件失败：{e}"))?;
            }
        }
    } else {
        let dst_classes = rest_dir.join("WEB-INF").join("classes");
        copy_dir_all(maven_source, &dst_classes).map_err(|e| format!("拷贝 classes 失败：{e}"))?;
    }
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

// 从全量 rest_dir 中挑出本次变更涉及的 class/资源，复制到 copy_dir（增量包）
fn extract_incremental_files(
    changed_paths: &[String],
    rest_dir: &Path,
    copy_dir: &Path,
) -> (usize, Vec<String>) {
    // target_rel -> Option<base_class_name>
    let mut targets: Vec<(String, Option<String>)> = Vec::new();
    for sp in changed_paths {
        if let Some((rel, is_java)) = normalize_svn_path(sp) {
            let base = if is_java {
                Path::new(&rel)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n.trim_end_matches(".class").to_string())
            } else {
                None
            };
            // 去重
            if !targets.iter().any(|(r, _)| r == &rel) {
                targets.push((rel, base));
            }
        }
    }
    if targets.is_empty() {
        return (0, vec!["未找到 /rest/src 相关的变更".into()]);
    }

    let mut not_found: std::collections::HashSet<String> =
        targets.iter().map(|(r, _)| r.clone()).collect();
    let mut copied = 0usize;

    let mut files = Vec::new();
    collect_files(rest_dir, &mut files);

    for file_path in files {
        let file_str = file_path.to_string_lossy().replace('\\', "/");
        let Some(rest_idx) = file_str.find("/rest") else {
            continue;
        };
        let path_for_match = &file_str[rest_idx..];
        let filename = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        for (target_rel, base_class) in &targets {
            let mut matched = false;
            let is_exact = path_for_match == target_rel;
            let is_inner = base_class
                .as_ref()
                .map(|b| filename.starts_with(&format!("{b}$")))
                .unwrap_or(false);
            if is_exact {
                matched = true;
            } else if is_inner {
                matched = true;
            } else if path_for_match.starts_with(target_rel.as_str())
                && path_for_match.ends_with(".class")
                && !target_rel.ends_with(".class")
            {
                matched = true;
            }
            if matched {
                let rel_to_rest = path_for_match
                    .strip_prefix("/rest")
                    .unwrap_or(path_for_match)
                    .trim_start_matches('/');
                let dest = copy_dir.join(rel_to_rest);
                if let Some(parent) = dest.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if !dest.exists() {
                    if fs::copy(&file_path, &dest).is_ok() {
                        copied += 1;
                    }
                }
                if is_exact || is_inner {
                    not_found.remove(target_rel);
                }
                break;
            }
        }
    }

    let mut remaining: Vec<String> = not_found.into_iter().collect();
    remaining.sort();
    (copied, remaining)
}

// 从 rest/version 读项目版本号前缀（形如 1.6），读不到回退 1.0
fn project_version(rest_path: &Path) -> String {
    let version_file = rest_path.join("version");
    let Ok(content) = fs::read_to_string(&version_file) else {
        return "1.0".into();
    };
    let content = content.trim();
    let parts: Vec<&str> = content.splitn(3, '.').collect();
    if parts.len() >= 2 {
        let major: String = parts[0].chars().take_while(|c| c.is_ascii_digit()).collect();
        let minor: String = parts[1].chars().take_while(|c| c.is_ascii_digit()).collect();
        if !major.is_empty() && !minor.is_empty() {
            return format!("{major}.{minor}");
        }
    }
    "1.0".into()
}

// 生成并写入增量包内的 version 文件：1.x.YYMMDD(REV)
fn write_version_file(rest_path: &Path, revision: u64, copy_dir: &Path) -> Result<String, String> {
    let ver = project_version(rest_path);
    let date = chrono::Local::now().format("%y%m%d").to_string();
    let version_str = format!("{ver}.{date}({revision})");
    let dest = copy_dir.join("WEB-INF").join("classes").join("version");
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 version 目录失败：{e}"))?;
    }
    fs::write(&dest, format!("{version_str}\n")).map_err(|e| format!("写 version 失败：{e}"))?;
    Ok(version_str)
}

/// 执行增量包构建（不含 zip，便于中途放入前端包）。返回构建结果 + 过程日志。
pub fn run_package_build(
    svn_bin: &str,
    rest_path: &str,
    opts: &PackageOptions,
    revisions: &[u64],
) -> Result<PackageBuildResult, String> {
    if opts.requirement_name.trim().is_empty() {
        return Err("需求名称不能为空".into());
    }
    let rest = PathBuf::from(rest_path);
    if !rest.is_dir() {
        return Err(format!("rest 目录不存在：{rest_path}"));
    }
    // 项目根：rest 的上级（develop/rest -> develop？不，提供包应放项目根）。
    // 与脚本一致，提供包放在 rest 所属项目目录下，这里取 rest 的祖父目录（develop/rest -> 项目根）。
    let project_root = rest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| rest.clone());

    let mut log = Vec::new();
    log.push("[1/4] 创建提供包目录结构...".into());
    let dirs = create_package_structure(&project_root, opts)?;
    log.push(format!("提供包：{}", dirs.base_dir.display()));

    log.push("[2/4] 复制 Maven 编译产物...".into());
    let maven = find_maven_exploded_war(&rest)?;
    copy_maven_output(&maven, &dirs.rest_dir)?;
    log.push(format!("来源：{}", maven.display()));

    log.push("[3/4] 提取增量文件...".into());
    let changed = changed_paths_for_revisions(svn_bin, rest_path, revisions)
        .map_err(|e| format!("拉取变更路径失败：{e}"))?;
    let (copied_count, not_found) = if changed.is_empty() {
        log.push("未提供版本或无变更路径，跳过增量提取。".into());
        (0, Vec::new())
    } else {
        let (c, nf) = extract_incremental_files(&changed, &dirs.rest_dir, &dirs.copy_dir);
        log.push(format!("已复制 {c} 个增量文件"));
        if !nf.is_empty() {
            log.push(format!("未匹配到 {} 个文件", nf.len()));
        }
        (c, nf)
    };

    log.push("[4/4] 写入 version 并清理目录...".into());
    let latest_rev = revisions.iter().max().copied().unwrap_or(0);
    let version = write_version_file(&rest, latest_rev, &dirs.copy_dir)?;
    // 删除全量 rest，把 copy 重命名为 rest —— 最终包只保留增量文件 + version
    if dirs.rest_dir.exists() {
        fs::remove_dir_all(&dirs.rest_dir).map_err(|e| format!("删除全量 rest 失败：{e}"))?;
    }
    fs::rename(&dirs.copy_dir, &dirs.rest_dir).map_err(|e| format!("重命名 copy->rest 失败：{e}"))?;
    log.push(format!("版本：{version}"));

    Ok(PackageBuildResult {
        base_dir: dirs.base_dir.to_string_lossy().to_string(),
        front_dir: dirs.front_dir.to_string_lossy().to_string(),
        final_rest_dir: dirs.rest_dir.to_string_lossy().to_string(),
        version,
        copied_count,
        not_found,
        log,
    })
}

/// 把提供包目录打成 ZIP：{需求名} - YYYYMMDDHHMMSS.zip，放在 base_dir 同级。
pub fn package_zip(base_dir: &str, requirement_name: &str) -> Result<PackageZipResult, String> {
    let base = PathBuf::from(base_dir);
    if !base.is_dir() {
        return Err(format!("打包目录不存在：{base_dir}"));
    }
    let now = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let zip_name = format!("{requirement_name} - {now}.zip");
    let parent = base.parent().ok_or("无法定位上级目录")?;
    let zip_path = parent.join(&zip_name);
    let base_name = base
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("package")
        .to_string();

    let file = fs::File::create(&zip_path).map_err(|e| format!("创建 ZIP 失败：{e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut files = Vec::new();
    collect_files(&base, &mut files);
    for file_path in files {
        if file_path.file_name().and_then(|s| s.to_str()) == Some(".DS_Store") {
            continue;
        }
        let rel = file_path
            .strip_prefix(&base)
            .map_err(|e| format!("计算相对路径失败：{e}"))?;
        let arcname = format!("{base_name}/{}", rel.to_string_lossy().replace('\\', "/"));
        zip.start_file(arcname, options)
            .map_err(|e| format!("写入 ZIP 条目失败：{e}"))?;
        let data = fs::read(&file_path).map_err(|e| format!("读取文件失败：{e}"))?;
        zip.write_all(&data).map_err(|e| format!("写入 ZIP 数据失败：{e}"))?;
    }
    zip.finish().map_err(|e| format!("完成 ZIP 失败：{e}"))?;

    let size = fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
    Ok(PackageZipResult {
        zip_path: zip_path.to_string_lossy().to_string(),
        size,
    })
}

/// 写入分支 rest/version 文件并提交（svn add 若未版本化 + svn commit）。
pub fn commit_version(svn_bin: &str, rest_path: &str, version_content: &str) -> AppResult<String> {
    let rest = PathBuf::from(rest_path);
    if !rest.is_dir() {
        return Err(AppError::InvalidPath(rest_path.to_string()));
    }
    let version_file = rest.join("version");
    fs::write(&version_file, format!("{}\n", version_content.trim()))?;
    let version_file_str = version_file.to_string_lossy().to_string();

    // 未版本化则先 add
    let status = run_svn(svn_bin, &["status", &version_file_str])?;
    if status.stdout.trim_start().starts_with('?') {
        run_svn(svn_bin, &["add", &version_file_str])?;
    }
    let message = format!("版本号：{}", version_content.trim());
    let out = run_svn(
        svn_bin,
        &[
            "commit",
            "--non-interactive",
            &version_file_str,
            "-m",
            &message,
        ],
    )?;
    let combined = [out.stdout.trim(), out.stderr.trim()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    Ok(if combined.is_empty() {
        "svn commit 完成".into()
    } else {
        combined
    })
}
