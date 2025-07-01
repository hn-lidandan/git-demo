use crate::controller::git_controller::SepFileRequest;
use anyhow::{Context, Result, anyhow};
use git2::{
    BranchType, Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository, build::RepoBuilder,
};
use log::{error, info, warn};
use secrecy::{ExposeSecret, Secret};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

static TEST_REPOS: &str = "test_repos";
static BARE_REPOS: &str = "bare_repos";

pub fn init_repo(repo_name: String) -> Result<(), Box<dyn Error>> {
    let repo_name = format!("{}/{}", TEST_REPOS, repo_name);
    if Path::new(&repo_name).exists() {
        // 初始化仓库
        return Err(anyhow!("当前{}仓库已存在:", repo_name).into());
    }
    Repository::init(repo_name)?;
    info!("仓库已初始化！");

    Ok(())
}

pub fn clone_with_token(
    url: &str,
    repo_name: &Path,
    token: Secret<String>,
) -> Result<Repository, Box<dyn Error>> {
    // 验证 URL 格式
    if !url.starts_with("https://") {
        warn!("⚠️ 建议使用 HTTPS URL 进行令牌认证");
    }

    // 创建回调函数
    let mut callbacks = RemoteCallbacks::new();

    // 设置认证回调
    callbacks.credentials(move |_url, username, _allowed| {
        // 使用令牌进行认证
        // 用户名可以是 "token" 或任意非空字符串
        let username = username.unwrap_or("token");
        Cred::userpass_plaintext(username, token.expose_secret())
    });

    // 配置获取选项
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    // 配置仓库构建器
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);

    // 构建最终的克隆路径
    let full_clone_path = PathBuf::from(TEST_REPOS).join(repo_name);

    // 执行克隆
    match builder.clone(url, &full_clone_path) {
        Ok(repo) => {
            info!(
                "              成功克隆仓库: {} 到 {}",
                url,
                full_clone_path.display()
            );
            let _bare_repo = convert_to_bare(&full_clone_path, repo_name)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            Ok(repo)
        }
        Err(e) => {
            error!("克隆失败: {}", e.message());
            Err(anyhow!("Git 操作失败: {}", e.message()).into())
        }
    }
}

/// 递归查找目录下的所有 Git 仓库
pub fn search_all_repo(root_path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut repos = Vec::new();

    // 递归遍历目录
    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        // 忽略隐藏目录（如 .git）
        .filter(|e| !e.path().to_string_lossy().contains("/."))
    // 修正过滤条件
    {
        let path = entry.path();
        // 检查是否是 Git 仓库根目录
        if path.join(".git").exists() {
            // 获取仓库的目录名作为仓库名称
            if let Some(name) = path.strip_prefix(root_path).unwrap_or(path).to_str() {
                repos.push(name.to_string());
            }
        }
    }
    Ok(repos)
}

/// 获取远程仓库分支列表
pub fn fetch_remote_branches(
    url: &str,
    token: Secret<String>,
) -> Result<Vec<String>, Box<dyn Error>> {
    // 创建带认证的回调
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username, _allowed| {
        let username = username.unwrap_or("token");
        Cred::userpass_plaintext(username, token.expose_secret())
    });

    // 创建临时目录用于克隆
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path();

    // 配置获取选项
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    // 配置仓库构建器
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);

    // 执行浅克隆(只获取元数据)
    let repo = builder.clone(url, temp_path)?;

    // 获取远程分支
    let mut branches = Vec::new();
    for branch in repo.branches(Some(BranchType::Remote))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            // 移除"origin/"前缀
            // let clean_name = name.replace("origin/", "");
            // info!("分支:{}", clean_name);
            branches.push(name.to_string());
        }
    }

    Ok(branches)
}

pub fn check_path(filerequest: &SepFileRequest) -> Result<PathBuf, Box<dyn Error>> {
    if filerequest.file_path.contains("..") || Path::new(&filerequest.file_path).is_absolute() {
        warn!("检测到路径遍历攻击: {}", filerequest.file_path);
        // return HttpResponse::BadRequest().body("Invalid file path");
        return Err(anyhow!("Invalid file path{}", filerequest.repo_name).into());
    }
    let repo_path = std::path::PathBuf::from("test_repos").join(&filerequest.repo_name);
    //打开仓库
    let repo = Repository::open(&repo_path)?;
    //获取当前分支
    let head = repo.head()?;
    let branch_name = head
        .shorthand()
        .ok_or_else(|| anyhow!("无法获取当前分支名称"))?;
    info!("当前分支: {}", branch_name);
    // 检查分支名称是否匹配
    if branch_name != &filerequest.branch_name {
        return Err(anyhow!(
            "分支不匹配: 当前分支 {}, 请求分支 {}",
            branch_name,
            &filerequest.branch_name
        )
        .into());
    }
    //文件完整路径
    let full_path = std::path::PathBuf::from("test_repos")
        .join(&filerequest.repo_name)
        .join(&filerequest.file_path);

    Ok(full_path)
}

// 从私有远程仓库拉取更新
pub fn pull_with_token(repo_path: &str, token: Secret<String>) -> Result<String, Box<dyn Error>> {
    let full_repo_path = format!("{}/{}", TEST_REPOS, repo_path);
    // 打开本地仓库
    let repo = Repository::open(&full_repo_path)?;
    info!("仓库路径: {}", full_repo_path);

    // 获取当前分支名称
    let head = repo.head()?;
    let branch_name = head
        .shorthand()
        .ok_or_else(|| anyhow!("无法获取当前分支名称"))?;
    info!("当前分支: {}", branch_name);

    // 获取远程仓库
    let mut remote = repo.find_remote("origin")?;

    // 设置认证回调
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username, _allowed| {
        Cred::userpass_plaintext(username.unwrap_or("token"), token.expose_secret())
    });

    // 配置fetch选项
    let mut fetch_options = FetchOptions::new();
    fetch_options
        .remote_callbacks(callbacks)
        .download_tags(git2::AutotagOption::All);

    // 执行fetch (使用正确的引用规格)
    let refspec = format!(
        "+refs/heads/{}:refs/remotes/origin/{}",
        branch_name, branch_name
    );
    remote.fetch(&[&refspec], Some(&mut fetch_options), None)?;
    info!("成功获取远程更新");

    // 获取FETCH_HEAD提交
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

    // 准备强制checkout选项
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder
        .force()
        .remove_untracked(true)
        .update_index(true)
        .use_theirs(true)
        .recreate_missing(true); // 关键修复：确保创建缺失文件

    // 分析合并情况
    let analysis = repo.merge_analysis(&[&fetch_commit])?;
    info!("合并分析结果: {:?}", analysis);

    if analysis.0.is_up_to_date() {
        let message = "已经是最新版本，无需更新".to_string();
        info!("{}", message);
        Ok(message)
    } else if analysis.0.is_fast_forward() {
        info!("执行快进合并");

        // 更新本地分支引用
        let refname = format!("refs/heads/{}", branch_name);
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "Fast-Forward")?;

        // 重置HEAD并检出
        repo.set_head(&refname)?;
        repo.checkout_head(Some(&mut checkout_builder))?;

        // 额外确保工作目录同步
        let mut index = repo.index()?;
        index.read(true)?;
        index.write()?;

        let message = "成功更新工作目录".to_string();
        info!("{}", message);
        let work_repo_path = Path::new(TEST_REPOS).join(repo_path); // test_repos/zss
        let bare_repo_path = Path::new(BARE_REPOS).join(format!("{}.git", repo_path)); // bare_repos/zss.git

        //同步裸仓库
        let _ = sync_bare_repo(&work_repo_path, &bare_repo_path);
        Ok(message)
    } else {
        return Err(anyhow!("需要手动解决合并冲突").into());
    }
}

pub fn sync_bare_repo(repo_path: &Path, bare_path: &Path) -> Result<(), Box<dyn Error>> {
    info!(
        "开始同步裸仓库: {} -> {}",
        repo_path.display(),
        bare_path.display()
    );

    // 确保裸仓库目录存在
    if let Some(parent) = bare_path.parent() {
        fs::create_dir_all(parent).context("创建裸仓库父目录失败")?;
    }

    // 同步到裸仓库
    match sync_to_bare_repo(&repo_path, bare_path) {
        Ok(_) => info!("✅ 成功同步到裸仓库"),
        Err(e) => {
            warn!("⚠️ 同步到裸仓库失败: {}", e);
            return Err(e);
        }
    }

    // 优化裸仓库
    match optimize_bare_repo(bare_path) {
        Ok(_) => info!("✅ 成功优化裸仓库"),
        Err(e) => {
            warn!("⚠️ 优化裸仓库失败: {}", e);
            // 优化失败不影响同步结果，只记录警告
        }
    }

    info!("裸仓库同步完成: {}", bare_path.display());
    Ok(())
}

// 同步到裸仓库
fn sync_to_bare_repo(source_repo_path: &Path, bare_path: &Path) -> Result<(), Box<dyn Error>> {
    // 1. 打开源仓库（工作仓库）
    let source_repo = Repository::open(source_repo_path)?;

    // 2. 确保裸仓库存在
    if !bare_path.exists() {
        info!("创建新的裸仓库: {}", bare_path.display());
        Repository::init_bare(bare_path)?;
    }

    // 3. 将裸仓库添加为源仓库的远程
    let bare_url = format!("file://{}", bare_path.canonicalize()?.to_str().unwrap());
    let mut remote = match source_repo.find_remote("bare_sync") {
        Ok(r) => {
            info!("找到现有远程 'bare_sync'");
            r
        }
        Err(_) => {
            info!("创建新的远程 'bare_sync'");
            source_repo.remote("bare_sync", &bare_url)?
        }
    };

    // 4. 配置推送选项
    let mut callbacks = RemoteCallbacks::new();
    callbacks.push_update_reference(|refname, status| {
        if status.is_none() {
            info!("✅ 成功推送引用: {}", refname);
            Ok(())
        } else {
            warn!("❌ 推送引用失败: {}", refname);
            Err(git2::Error::from_str("推送引用失败"))
        }
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    // 5. 从源仓库推送到裸仓库
    info!("开始推送引用到裸仓库: {}", bare_path.display());

    // 获取所有本地分支和标签
    let mut refspecs = Vec::new();

    // 推送所有本地分支
    for branch in source_repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            let refspec = format!("refs/heads/{}:refs/heads/{}", name, name);
            refspecs.push(refspec);
            info!("准备推送分支: {}", name);
        }
    }

    // 推送所有标签
    for tag_name in source_repo.tag_names(None)?.iter() {
        if let Some(tag_name) = tag_name {
            let refspec = format!("refs/tags/{}:refs/tags/{}", tag_name, tag_name);
            refspecs.push(refspec);
            info!("准备推送标签: {}", tag_name);
        }
    }

    if refspecs.is_empty() {
        warn!("⚠️ 没有找到任何引用需要推送");
        return Ok(());
    }

    // 转换为字符串数组
    let refspec_strs: Vec<&str> = refspecs.iter().map(|s| s.as_str()).collect();
    remote.push(&refspec_strs, Some(&mut push_options))?;

    info!("✅ 成功同步到裸仓库");
    Ok(())
}

// 优化裸仓库
fn optimize_bare_repo(bare_path: &Path) -> Result<(), Box<dyn Error>> {
    let repo = Repository::open_bare(bare_path).context("无法打开裸仓库进行优化")?;

    info!("开始优化裸仓库: {}", bare_path.display());

    // 获取当前分支名
    let head = repo.head()?;
    let current_branch = head
        .shorthand()
        .ok_or_else(|| anyhow!("无法获取当前分支名称"))?;

    // 更新引用日志
    if let Some(head_target) = head.target() {
        let refname = format!("refs/heads/{}", current_branch);
        repo.reference(&refname, head_target, true, "Optimization")
            .context("更新引用日志失败")?;
        info!("✅ 更新引用日志完成: {}", refname);
    } else {
        warn!("⚠️ 裸仓库HEAD没有指向有效提交，跳过引用日志更新");
    }

    info!("✅ 裸仓库优化完成");
    Ok(())
}

/// 获取本地仓库的所有分支（本地 + 远程）
pub fn list_branches(repo_path: &String) -> Result<Vec<String>, Box<dyn Error>> {
    // 打开本地仓库
    let repo = Repository::open(repo_path)?;
    let mut branches = Vec::new();

    // 1. 获取本地分支（refs/heads/）
    let local_branches = repo.branches(Some(BranchType::Local))?;
    print!("准备遍历");
    for branch_result in local_branches {
        let (branch, _branch_type) = branch_result?;
        let branch_name = branch.name()?.unwrap_or("unnamed-local-branch");
        print!("Found branch: {}", branch_name);
        branches.push(branch_name.to_string());
    }

    Ok(branches)
}

pub fn convert_to_bare(source: &Path, repo_name: &Path) -> Result<PathBuf> {
    // 1. 准备目标路径
    let bare_root = Path::new(BARE_REPOS);
    fs::create_dir_all(bare_root).context("创建裸仓库目录失败")?;

    let bare_path = bare_root.join(format!("{}.git", repo_name.to_string_lossy()));

    // 2. 清理可能存在的旧仓库
    if bare_path.exists() {
        fs::remove_dir_all(&bare_path).context("清理已有裸仓库失败")?;
        info!("清理已有裸仓库失败");
    }
    // 3. 创建裸仓库目录结构
    info!("创建裸仓库目录结构");
    fs::create_dir(&bare_path)?;
    // 4. 使用硬链接迁移Git对象
    info!("使用硬链接迁移Git对象");
    link_git_objects(source, &bare_path)?;
    // 5. 迁移其他Git元数据
    info!("迁移其他Git元数据");
    migrate_git_metadata(source, &bare_path)?;
    // 6. 配置为裸仓库
    info!("配置为裸仓库");
    configure_bare_repo(&bare_path)?;
    Ok(bare_path)
}

/// 使用硬链接迁移Git对象数据库
fn link_git_objects(source: &Path, dest: &Path) -> Result<()> {
    let source_objects = source.join(".git/objects");
    let dest_objects = dest.join("objects");

    // 创建目标objects目录
    fs::create_dir_all(&dest_objects)?;

    // 递归处理objects子目录
    for entry in fs::read_dir(&source_objects)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // 处理pack目录
            if path.file_name() == Some("pack".as_ref()) {
                link_pack_files(&path, &dest_objects.join("pack"))?;
            } else {
                // 创建子目录硬链接
                let dest_dir = dest_objects.join(path.file_name().unwrap());
                fs::create_dir(&dest_dir)?;

                for file in fs::read_dir(&path)? {
                    let file = file?;
                    let src_file = file.path();
                    let dest_file = dest_dir.join(file.file_name());

                    fs::hard_link(&src_file, &dest_file)
                        .context(format!("创建硬链接失败: {:?} -> {:?}", src_file, dest_file))?;
                }
            }
        }
    }

    Ok(())
}

/// 特殊处理pack文件（避免复制大文件）
fn link_pack_files(source_pack: &Path, dest_pack: &Path) -> Result<()> {
    fs::create_dir_all(dest_pack)?;

    for entry in fs::read_dir(source_pack)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest_pack.join(entry.file_name());

        // 创建硬链接
        fs::hard_link(&src_path, &dest_path)
            .context(format!("链接pack文件失败: {:?}", src_path))?;
    }

    Ok(())
}

/// 迁移Git元数据
fn migrate_git_metadata(source: &Path, dest: &Path) -> Result<()> {
    let git_dir = source.join(".git");

    // 迁移关键文件
    let critical_files = &[
        "HEAD",
        "config",
        "description",
        "info",
        "refs",
        "packed-refs",
    ];

    for file in critical_files {
        let src = git_dir.join(file);
        let dst = dest.join(file);

        if src.exists() {
            if src.is_dir() {
                copy_dir(&src, &dst)?;
            } else {
                fs::copy(&src, &dst)?;
            }
        }
    }

    // 特殊处理hooks（可选）
    let hooks_src = git_dir.join("hooks");
    if hooks_src.exists() {
        copy_dir(&hooks_src, &dest.join("hooks"))?;
    }

    Ok(())
}

/// 配置为裸仓库
fn configure_bare_repo(repo_path: &Path) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let mut config = repo.config()?;

    // 设置为裸仓库
    config.set_bool("core.bare", true)?;

    // 优化配置
    config.set_bool("gc.auto", true)?;
    config.set_bool("repack.writeBitmaps", true)?;
    config.set_bool("receive.autogc", true)?;
    config.set_str("receive.denyNonFastForwards", "true")?;

    // 设置共享权限（多用户环境）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o2775);
        fs::set_permissions(repo_path, permissions)?;
    }

    Ok(())
}

/// 递归复制目录
fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let dest = dst.join(name);

        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}
