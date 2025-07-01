use crate::controller::barerepo_controller::{head_ref, info_refs, upload_pack};
use crate::service::git_service;
use actix_files::NamedFile;
use actix_web::web;
use actix_web::{Error, HttpRequest, HttpResponse, Responder, get, post};
use anyhow::{Result, anyhow};
use log::info;
use log::warn;
use secrecy::Secret;
use serde::{Deserialize, Serialize};
use std;

#[derive(Debug, Deserialize, Serialize)]
struct CloneRequest {
    url: String,
    path: String,
}
#[derive(Debug, Deserialize, Serialize)]
struct PullRequest {
    // url: String,
    path: String,
}

// 定义根路径处理器
#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello World!")
}

#[post("/clone_pri")]
async fn clone_pri(params: web::Json<CloneRequest>) -> impl Responder {
    let request = params.into_inner();
    match get_token() {
        Ok(token) => {
            let path = std::path::Path::new(&request.path);
            match git_service::clone_with_token(&request.url, path, token) {
                Ok(_) => HttpResponse::Ok().body("Repository cloned successfully"),
                Err(e) => HttpResponse::InternalServerError()
                    .body(format!("Failed to clone repository: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Failed to get token: {}", e)),
    }
}

#[post("/pull_pri")]
async fn pull_pri(params: web::Json<PullRequest>) -> impl Responder {
    let request = params.into_inner();
    match get_token() {
        Ok(token) => match git_service::pull_with_token(&request.path, token) {
            Ok(meassage) => HttpResponse::Ok().body(meassage),
            Err(e) => HttpResponse::InternalServerError().body(format!("拉取更新失败：{}", e)),
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("获取token失败:{}", e)),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct BranchRequest {
    url: String,       // 远程仓库地址
    repo_name: String, // 仓库名
}

#[post("/fetch_remote_branches")]
async fn fetch_remote_branches(params: web::Json<BranchRequest>) -> impl Responder {
    let request = params.into_inner();
    match get_token() {
        Ok(token) => match git_service::fetch_remote_branches(&request.url, token) {
            Ok(branches) => HttpResponse::Ok().json(branches),
            Err(e) => HttpResponse::InternalServerError().body(format!("获取远程分支失败: {}", e)),
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("获取token失败: {}", e)),
    }
}

#[post("/clone_pub")]
async fn clone_pub(params: web::Json<CloneRequest>) -> impl Responder {
    let request: CloneRequest = params.into_inner();
    print!("request:{:?}", request);
    let url = &request.url;
    let _repo = match git2::Repository::clone(url, request.path) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to clone: {}", e),
    };
    HttpResponse::Ok().body("Repository cloned successfully")
}

#[get("/search_all_repo")]
async fn search_all_repo() -> Result<HttpResponse, Error> {
    let result = git_service::search_all_repo("test_repos/")?;

    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize)]
struct BranchQuery {
    repo_name: String,
}
#[get("/search_all_branch/{repo_name}")]
async fn search_all_branch(params: web::Query<BranchQuery>) -> Result<HttpResponse, Error> {
    let branch_query = params.into_inner();
    print!("{:?}", branch_query.repo_name);
    let list = git_service::list_branches(&branch_query.repo_name);

    Ok(HttpResponse::Ok().json(format!("{:?}", list)))
}

#[derive(Deserialize)]
struct RepoQuery {
    repo_name: String,
}

#[get("/init_repo")]
async fn init_repo(repo_params: web::Query<RepoQuery>) -> impl Responder {
    match git_service::init_repo(repo_params.repo_name.clone()) {
        Ok(_) => HttpResponse::Ok().json("Repository initialized successfully"),
        Err(e) => HttpResponse::InternalServerError()
            .body(format!("Failed to initialize repository: {}", e)),
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SepFileRequest {
    pub repo_name: String,
    pub branch_name: String,
    pub file_path: String,
}

#[get("/download")]
async fn get_specify_file(req: HttpRequest, params: web::Query<SepFileRequest>) -> impl Responder {
    let spefilerequest = params.into_inner();

    //  构建完整文件路径
    let full_path = match git_service::check_path(&spefilerequest) {
        Ok(path) => path,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    info!("文件完整路径：{:?}", &full_path);
    //  尝试打开文件
    match NamedFile::open_async(&full_path).await {
        Ok(mut file) => {
            let filename = full_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            info!("打开文件");
            file = file.use_last_modified(true).set_content_disposition(
                actix_web::http::header::ContentDisposition::attachment(filename),
            );
            info!("返回文件");
            return file.into_response(&req);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("文件出错");
            return HttpResponse::InternalServerError().body(e.to_string());
        }
        Err(_) => return HttpResponse::InternalServerError().body("Failed to read file"),
    }
}

pub fn path_config(service_config: &mut web::ServiceConfig) {
    let stu_scope = web::scope("")
        .service(hello)
        .service(clone_pri)
        .service(clone_pub)
        .service(get_specify_file)
        .service(fetch_remote_branches)
        .service(pull_pri)
        .service(search_all_repo)
        .service(search_all_branch)
        .service(init_repo)
        .service(upload_pack)
        .service(head_ref)
        .service(info_refs);
    service_config.service(stu_scope);
}

/// 安全获取访问令牌
fn get_token() -> Result<Secret<String>> {
    // 1. 尝试从环境变量获取
    if let Ok(token) = std::env::var("GIT_TOKEN") {
        return Ok(Secret::new(token));
    }

    // 2. 尝试从 .env 文件获取（开发环境）
    #[cfg(debug_assertions)]
    {
        dotenv::dotenv().ok();
        if let Ok(token) = std::env::var("GIT_TOKEN") {
            warn!("⚠️ 从 .env 文件获取令牌 - 仅限开发环境使用");
            return Ok(Secret::new(token));
        }
    }
    // 3. 所有尝试失败
    Err(anyhow!("无法获取访问令牌。请设置 GIT_TOKEN 环境变量"))
}

#[derive(Deserialize, Serialize)]
pub struct FormParams {
    pub repo_name: String,
    pub branch_name: String,
    pub commit_message: String,
}
