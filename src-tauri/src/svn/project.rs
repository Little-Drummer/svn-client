use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, AppResult};
use crate::models::{Project, ProjectBranch, ProjectModule, WorkingCopyEntry};

// 与前端 utils.ts 保持一致：环境层（分支）目录名
const ENV_FOLDERS: &[&str] = &["develop", "test", "produce"];
// 已知模块目录名，仅用于排序权重；个人分支等未知名称按字母序排在已知模块之后
const MODULE_ORDER: &[&str] = &["front", "rest", "database", "updatesql"];

fn split_path(path: &str) -> Vec<String> {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn is_env_folder(name: &str) -> bool {
    ENV_FOLDERS.contains(&name.to_lowercase().as_str())
}

/// 从本地路径推断 (项目, 环境, 模块)。
/// 优先取 `work/` 之后的层级；否则回退到「倒数第二段是环境目录」的形态，
/// 既能识别 `/Volumes/.../work/项目/develop/rest`，也能识别未放在 work 下的同构目录。
fn classify_path(path: &str) -> Option<(String, String, Option<String>)> {
    let parts = split_path(path);
    if parts.is_empty() {
        return None;
    }

    let tail: Vec<String> = if let Some(idx) = parts.iter().position(|p| p.eq_ignore_ascii_case("work")) {
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

    match second {
        Some(env) if is_env_folder(&env) => Some((project, env, third)),
        // 缺少环境层时归入「默认」，第二段视为模块
        Some(module) => Some((project, "默认".to_string(), Some(module))),
        None => Some((project, "默认".to_string(), None)),
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
        let Some((project, env, module)) = classify_path(&wc.path) else {
            continue;
        };
        let module_name = module.unwrap_or_else(|| {
            split_path(&wc.path)
                .last()
                .cloned()
                .unwrap_or_else(|| wc.path.clone())
        });
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

/// 扫描一个项目根目录，找出其下所有工作副本路径。
/// 识别两种布局：
///   - 项目/{develop,test,produce}/{rest,database,updatesql,个人分支...}
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
            // 环境目录本身不是工作副本，向下一层找模块 / 个人分支
            for sub in fs::read_dir(&path).map_err(AppError::Io)? {
                let sub_path = sub.map_err(AppError::Io)?.path();
                if sub_path.is_dir() && is_working_copy(&sub_path) {
                    found.push(sub_path);
                }
            }
        } else if is_working_copy(&path) {
            found.push(path);
        }
    }

    // 用户直接选中了单个工作副本目录的情况
    if found.is_empty() && is_working_copy(root) {
        found.push(root.to_path_buf());
    }

    Ok(found)
}
