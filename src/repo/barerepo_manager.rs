use actix_web::Error;
use git2::Repository;
use log::info;
use std::path::{Path, PathBuf};
use std::process::Command;
pub struct RepoManager {
    base_path: PathBuf,
}

impl RepoManager {
    // 构造函数：创建一个新的 RepoManager 实例
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        // 确保 base_path 目录存在，如果不存在则创建。unwrap() 会在失败时引起 panic。
        std::fs::create_dir_all(&path).unwrap();
        Self {
            base_path: path.as_ref().to_path_buf(),
        }
    }

    // 获取裸仓库的完整路径
    pub fn get_bare_repo_path(&self, repo_name: &str) -> PathBuf {
        info!("当前的裸仓库repo_name:{}", repo_name);
        self.base_path.join(repo_name)
    }

    pub fn get_repo(&self, repo_name: &str) -> Result<Repository, actix_web::Error> {
        let repo_path = self.get_bare_repo_path(repo_name);
        info!("当前的裸仓库的名称为：{:?}", repo_path);
        if !repo_path.exists() {
            return Err(actix_web::error::ErrorNotFound(format!(
                "Repository {} not found",
                repo_name
            )));
        }
        Repository::open_bare(&repo_path).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Failed to open repo: {}", e))
        })
    }

    // 获取仓库的引用信息（用于 info/refs 服务）
    pub fn get_refs(&self, repo_name: &str) -> Result<Vec<u8>, actix_web::Error> {
        let repo = self.get_repo(repo_name)?;

        let mut buf = Vec::new();
        // 协议头必须是这种格式
        buf.extend(b"001e# service=git-upload-pack\n");
        buf.extend(b"0000");

        // 添加引用列表
        let mut refs = Vec::new();

        // 添加HEAD引用
        let head = repo
            .head()
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

        // 检查HEAD是否有有效的目标
        if let Some(head_oid) = head.target() {
            refs.push(format!("{} HEAD\0multi_ack multi_ack_detailed thin-pack side-band side-band-64k ofs-delta shallow deepen-since deepen-not deepen-relative no-progress include-tag ofs-delta agent=git/2.40.0\n", head_oid));
        } else {
            // HEAD没有指向有效的提交，返回错误
            return Err(actix_web::error::ErrorInternalServerError(
                "Repository HEAD does not point to a valid commit".to_string(),
            ));
        }

        // 添加其他分支
        for reference in repo
            .references()
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
        {
            let ref_info =
                reference.map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
            if !ref_info.is_tag() {
                let name = ref_info.name().unwrap_or("");
                if let Some(oid) = ref_info.target() {
                    refs.push(format!("{} {}\n", oid, name));
                }
            }
        }

        // 格式化每条引用行
        for line in refs {
            let line_len = line.len() + 4; // 包括4字节长度前缀
            buf.extend(format!("{:04x}", line_len).as_bytes());
            buf.extend(line.as_bytes());
        }

        // 结束标记
        buf.extend(b"0000");
        Ok(buf)
    }

    // 检查裸仓库是否存在
    pub fn repo_exists(&self, repo_name: &str) -> bool {
        let path = self.get_bare_repo_path(repo_name);
        println!("Checking repo at: {:?}", path); // 添加调试日志
        path.exists()
    }

    // 处理 git-upload-pack 请求
    // 这里的注释解释了为什么不直接使用 git2 库的 upload_pack 方法，而是通过子进程调用 git-upload-pack。
    // git2::Repository 并未直接提供名为'upload_pack'的方法来处理通过任意流进行的 Git 协议。
    // 在 Rust 中实现 git-upload-pack 服务的一种常见方法是将 git-upload-pack 可执行文件作为子进程执行，并通过管道来传递输入和输出。
    pub fn handle_upload_pack(&self, repo_name: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        let repo_path = self.get_bare_repo_path(repo_name);

        // 创建临时文件存储输入数据
        let temp_dir = tempfile::tempdir()?;
        let input_path = temp_dir.path().join("upload_pack_input");
        std::fs::write(&input_path, input)?;

        // 执行git-upload-pack
        let output = Command::new("git")
            .arg("upload-pack")
            .arg("--stateless-rpc")
            .arg(&repo_path)
            .stdin(std::fs::File::open(&input_path)?)
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(actix_web::error::ErrorInternalServerError(format!(
                "git-upload-pack failed: {}",
                err_msg
            )));
        }

        // 确保输出以"0000"结尾
        let mut result = output.stdout;
        if !result.ends_with(b"0000") {
            result.extend(b"0000");
        }

        Ok(result)
    }
}
