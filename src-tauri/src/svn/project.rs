use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, AppResult};
use crate::models::{Project, ProjectBranch, ProjectModule, WorkingCopyEntry};

// 与前端 utils.ts 保持一致：环境层（分支）目录名
const ENV_FOLDERS: &[&str] = &["develop", "test", "produce"];
// 已知模块目录名，仅用于排序权重；个人分支等未知名称按字母序排在已知模块之后
const MODULE_ORDER: &[&str] = &["front", "rest", "database", "updatesql"];
// 数据库目录历史上存在 databse 拼写，聚合时统一映射到 database。
const MODULE_ALIASES: &[(&str, &str)] = &[("databse", "database")];

fn split_path(path: &str) -> Vec<String> {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn is_env_folder(name: &str) -> bool {
    ENV_FOLDERS.contains(&name.to_lowercase().as_str())
}

fn normalize_module_name(name: String) -> String {
    let lower = name.to_lowercase();
    MODULE_ALIASES
        .iter()
        .find_map(|(alias, canonical)| {
            if lower == *alias {
                Some((*canonical).to_string())
            } else {
                None
            }
        })
        .unwrap_or(name)
}

fn decode_percent_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn svn_url_parts(url: &str) -> Vec<String> {
    let raw_path = if let Some(rest) = url.strip_prefix('^') {
        rest
    } else if let Some((_scheme, rest)) = url.split_once("://") {
        rest.split_once('/').map(|(_, path)| path).unwrap_or("")
    } else {
        url
    };
    decode_percent_path(raw_path)
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn classify_remote_url(url: Option<&str>) -> Option<(String, String, Option<String>)> {
    let parts = svn_url_parts(url?);
    let env_idx = parts.iter().position(|p| is_env_folder(p))?;
    let env = parts.get(env_idx)?.clone();
    let module = parts.get(env_idx + 1).cloned().map(normalize_module_name);

    // 优先取环境目录前一段作为项目名，兼容 ^/项目/develop/database 这类仓库路径。
    // 若 URL 直接从 ^/develop/database 开始，则项目名留空，交给本地路径补齐。
    let project = if env_idx > 0 {
        parts[env_idx - 1].clone()
    } else {
        String::new()
    };
    Some((project, env, module))
}

/// 从本地路径推断 (项目, 环境, 模块)。
/// 优先取 `work/` 之后的层级；否则回退到「倒数第二段是环境目录」的形态，
/// 既能识别 `/Volumes/.../work/项目/develop/rest`，也能识别未放在 work 下的同构目录。
fn classify_path(path: &str) -> Option<(String, String, Option<String>)> {
    let parts = split_path(path);
    if parts.is_empty() {
        return None;
    }

    let tail: Vec<String> =
        if let Some(idx) = parts.iter().position(|p| p.eq_ignore_ascii_case("work")) {
            if idx + 1 < parts.len() {
                parts[idx + 1..].to_vec()
            } else {
                return None;
            }
        } else if parts.len() >= 3 && is_env_folder(&parts[parts.len() - 2]) {
            parts[parts.len() - 3..].to_vec()
        } else {
            return None;
        };

    let project = tail.first()?.clone();
    let second = tail.get(1).cloned();
    let third = tail.get(2).cloned();

    match (second, third) {
        (Some(env), module) if is_env_folder(&env) => {
            Some((project, env, module.map(normalize_module_name)))
        }
        // 项目/1.0bugfix/rest 这类自定义分支要保留分支名；只有 项目/rest 才归入「默认」。
        (Some(branch), Some(module)) => {
            Some((project, branch, Some(normalize_module_name(module))))
        }
        // 缺少环境层时归入「默认」，第二段视为模块。
        (Some(module), None) => Some((
            project,
            "默认".to_string(),
            Some(normalize_module_name(module)),
        )),
        (None, _) => Some((project, "默认".to_string(), None)),
    }
}

fn classify_working_copy(wc: &WorkingCopyEntry) -> Option<(String, String, Option<String>)> {
    let by_path = classify_path(&wc.path);
    let by_relative = classify_remote_url(wc.relative_url.as_deref());
    let by_url = classify_remote_url(wc.url.as_deref());
    let by_remote = by_relative.or(by_url);

    match (by_path, by_remote) {
        // 本地路径只有“默认”层时，用 SVN URL 补齐 develop/test/produce，避免同级 database/updatesql 漏合并方向。
        (
            Some((path_project, path_env, path_module)),
            Some((remote_project, remote_env, remote_module)),
        ) if path_env == "默认" && is_env_folder(&remote_env) => Some((
            if remote_project.is_empty() {
                path_project
            } else {
                remote_project
            },
            remote_env,
            remote_module.or(path_module),
        )),
        (Some(path), _) => Some(path),
        (None, Some((project, env, module))) if !project.is_empty() => Some((project, env, module)),
        (None, _) => None,
    }
}

fn env_rank(env: &str) -> usize {
    match env.to_lowercase().as_str() {
        "develop" => 0,
        "test" => 1,
        "produce" => 2,
        "默认" => 3,
        _ => 50,
    }
}

fn module_rank(module: &str) -> usize {
    MODULE_ORDER
        .iter()
        .position(|m| *m == module.to_lowercase())
        .unwrap_or(50)
}

/// 把扁平工作副本列表聚合成 项目 → 环境(分支) → 模块 的结构，供合并/打包定位各分支。
/// 无法识别项目结构的工作副本（不在 work 下、也不符合 项目/环境/模块 形态）会被跳过。
pub fn group_working_copies(copies: &[WorkingCopyEntry]) -> Vec<Project> {
    // project -> env -> modules，BTreeMap 保证名称稳定有序
    let mut map: BTreeMap<String, BTreeMap<String, Vec<ProjectModule>>> = BTreeMap::new();

    for wc in copies {
        let Some((project, env, module)) = classify_working_copy(wc) else {
            continue;
        };
        let module_name = module.unwrap_or_else(|| {
            split_path(&wc.path)
                .last()
                .cloned()
                .unwrap_or_else(|| wc.path.clone())
        });
        let module_name = normalize_module_name(module_name);
        map.entry(project)
            .or_default()
            .entry(env)
            .or_default()
            .push(ProjectModule {
                module: module_name,
                working_copy_id: wc.id.clone(),
                path: wc.path.clone(),
                url: wc.url.clone(),
            });
    }

    map.into_iter()
        .map(|(name, envs)| {
            let mut branches: Vec<ProjectBranch> = envs
                .into_iter()
                .map(|(environment, mut modules)| {
                    modules.sort_by(|a, b| {
                        module_rank(&a.module)
                            .cmp(&module_rank(&b.module))
                            .then_with(|| a.module.cmp(&b.module))
                    });
                    ProjectBranch {
                        environment,
                        modules,
                    }
                })
                .collect();
            branches.sort_by(|a, b| {
                env_rank(&a.environment)
                    .cmp(&env_rank(&b.environment))
                    .then_with(|| a.environment.cmp(&b.environment))
            });
            Project { name, branches }
        })
        .collect()
}

fn is_working_copy(dir: &Path) -> bool {
    // SVN 1.7+ 每个工作副本根只有一个 .svn 元数据目录
    dir.join(".svn").is_dir()
}

fn push_working_copy(found: &mut Vec<PathBuf>, path: PathBuf) {
    if !found.iter().any(|x| x == &path) {
        found.push(path);
    }
}

/// 扫描一个项目根目录，找出其下所有工作副本路径。
/// 识别两种布局：
///   - 项目/{develop,test,produce}/{rest,database,updatesql,个人分支...}
///   - 项目/{自定义分支}/{模块}
///   - 项目/{模块}        （无环境层，模块直接挂在项目根下）
/// 若根目录本身就是工作副本，则只返回它自己。
pub fn scan_project_dir(root: &Path) -> AppResult<Vec<PathBuf>> {
    if !root.is_dir() {
        return Err(AppError::InvalidPath(root.display().to_string()));
    }

    let mut found = Vec::new();
    for entry in fs::read_dir(root).map_err(AppError::Io)? {
        let path = entry.map_err(AppError::Io)?.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if name == ".svn" {
            continue;
        }

        if is_env_folder(name) {
            // 标准环境目录本身不是工作副本，向下一层找模块 / 个人分支
            for sub in fs::read_dir(&path).map_err(AppError::Io)? {
                let sub_path = sub.map_err(AppError::Io)?.path();
                if sub_path.is_dir() && is_working_copy(&sub_path) {
                    push_working_copy(&mut found, sub_path);
                }
            }
        } else if is_working_copy(&path) {
            push_working_copy(&mut found, path);
        } else {
            // 自定义分支目录（如 1.0bugfix/rest）也向下一层扫描工作副本。
            for sub in fs::read_dir(&path).map_err(AppError::Io)? {
                let sub_path = sub.map_err(AppError::Io)?.path();
                if sub_path.is_dir() && is_working_copy(&sub_path) {
                    push_working_copy(&mut found, sub_path);
                }
            }
        }
    }

    // 数据库脚本仓库有时与项目根目录同级，不在项目目录内部；选中项目根扫描时一并纳入。
    if let Some(parent) = root.parent() {
        for sibling in ["database", "databse", "updatesql"] {
            let p = parent.join(sibling);
            if p == root || !p.is_dir() {
                continue;
            }
            if is_working_copy(&p) {
                push_working_copy(&mut found, p);
                continue;
            }
            if let Ok(children) = fs::read_dir(&p) {
                for child in children.flatten() {
                    let child_path = child.path();
                    if child_path.is_dir() && is_working_copy(&child_path) {
                        push_working_copy(&mut found, child_path);
                    }
                }
            }
        }
    }

    // 用户直接选中了单个工作副本目录的情况
    if found.is_empty() && is_working_copy(root) {
        push_working_copy(&mut found, root.to_path_buf());
    }

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{classify_remote_url, classify_working_copy};
    use crate::models::WorkingCopyEntry;

    fn wc(path: &str, relative_url: Option<&str>, url: Option<&str>) -> WorkingCopyEntry {
        WorkingCopyEntry {
            id: "1".to_string(),
            path: path.to_string(),
            url: url.map(|s| s.to_string()),
            repository_root: None,
            revision: None,
            last_seen_at: None,
            relative_url: relative_url.map(|s| s.to_string()),
            display_name: None,
            available: true,
        }
    }

    #[test]
    fn classify_custom_branch_under_work() {
        let got = classify_working_copy(&wc("/data/work/北京市属机关/1.0bugfix/rest", None, None));
        assert_eq!(
            got,
            Some((
                "北京市属机关".to_string(),
                "1.0bugfix".to_string(),
                Some("rest".to_string()),
            ))
        );
    }

    #[test]
    fn classify_direct_module_as_default() {
        let got = classify_working_copy(&wc("/data/work/北京市属机关/rest", None, None));
        assert_eq!(
            got,
            Some((
                "北京市属机关".to_string(),
                "默认".to_string(),
                Some("rest".to_string()),
            ))
        );
    }

    #[test]
    fn classify_standard_environment() {
        let got = classify_working_copy(&wc("/data/work/北京市属机关/develop/rest", None, None));
        assert_eq!(
            got,
            Some((
                "北京市属机关".to_string(),
                "develop".to_string(),
                Some("rest".to_string()),
            ))
        );
    }

    #[test]
    fn classify_database_sibling_with_remote_env() {
        let got = classify_working_copy(&wc(
            "/data/work/北京市属机关/database",
            Some("^/北京市属机关/develop/database"),
            None,
        ));
        assert_eq!(
            got,
            Some((
                "北京市属机关".to_string(),
                "develop".to_string(),
                Some("database".to_string()),
            ))
        );
    }

    #[test]
    fn classify_databse_alias_as_database() {
        let got = classify_working_copy(&wc(
            "/data/work/北京市属机关/databse",
            Some("^/北京市属机关/test/databse"),
            None,
        ));
        assert_eq!(
            got,
            Some((
                "北京市属机关".to_string(),
                "test".to_string(),
                Some("database".to_string()),
            ))
        );
    }

    #[test]
    fn classify_percent_encoded_remote_url() {
        let got = classify_remote_url(Some("^/%E5%8C%97%E4%BA%AC/develop/updatesql"));
        assert_eq!(
            got,
            Some((
                "北京".to_string(),
                "develop".to_string(),
                Some("updatesql".to_string()),
            ))
        );
    }

    #[test]
    fn classify_remote_without_project_uses_local_project() {
        let got = classify_working_copy(&wc(
            "/data/work/北京市属机关/database",
            Some("^/develop/database"),
            None,
        ));
        assert_eq!(
            got,
            Some((
                "北京市属机关".to_string(),
                "develop".to_string(),
                Some("database".to_string()),
            ))
        );
    }
}
