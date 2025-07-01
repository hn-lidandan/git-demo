use crate::repo::barerepo_manager::RepoManager;
use actix_web::Error;
use actix_web::web::Data;
use actix_web::{HttpResponse, Responder, get, post, web};
use std::sync::Arc;

#[get("/{repo_name}/info/refs")]
async fn info_refs(
    repo_name: web::Path<String>,
    // query: web::Query<HashMap<String, String>>,
    repo_manager: Data<Arc<RepoManager>>,
) -> Result<HttpResponse, Error> {
    let full_path = repo_name.into_inner();
    print!("当前路径为：{}", full_path);

    if !repo_manager.repo_exists(&full_path) {
        print!("报错  没有找到");
        return Ok(HttpResponse::NotFound().body("Repository not found"));
    }

    let refs_data = repo_manager
        .get_refs(&full_path)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    //print!("refs_data:{:?}", refs_data);
    Ok(HttpResponse::Ok()
        .content_type("application/x-git-upload-pack-advertisement")
        .insert_header(("Cache-Control", "no-cache"))
        .body(refs_data))
}

#[post("/{repo_name}/git-upload-pack")]
async fn upload_pack(
    repo_name: web::Path<String>,
    body: web::Bytes,
    repo_manager: Data<Arc<RepoManager>>,
) -> Result<HttpResponse, Error> {
    let repo_name_str = repo_name.into_inner();
    if !repo_manager.repo_exists(&repo_name_str) {
        print!("{}没有找到", repo_name_str);
        return Ok(HttpResponse::NotFound().body("Repository not found"));
    }

    let pack_data = repo_manager
        .handle_upload_pack(&repo_name_str, &body)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    // print!("pack_data:{:?}",pack_data);
    Ok(HttpResponse::Ok()
        .content_type("application/x-git-upload-pack-result")
        .body(pack_data))
}

#[get("/{repo_name}/HEAD")]
async fn head_ref(
    repo_name: web::Path<String>,
    repo_manager: web::Data<Arc<RepoManager>>,
) -> Result<impl Responder, Error> {
    let repo = repo_manager
        .get_repo(&repo_name)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    let head = repo
        .head()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    print!("head为:{:?}", head.name().unwrap_or(""));
    Ok(web::Bytes::from(head.name().unwrap_or("").to_string()))
}
