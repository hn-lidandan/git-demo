mod auth;
mod config;
mod controller;
mod repo;
mod service;
use crate::config::tls_config::load_rustls_config;
use repo::barerepo_manager::RepoManager;
use std::sync::Arc;

use actix_web::web;
use actix_web::{App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 加载环境变量
    dotenv::dotenv().ok();
    // println!("GIT_SERVER_TOKEN={}", std::env::var("GIT_SERVER_TOKEN").unwrap_or_default());
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug) // 设置最低日志级别
        .init();
    // 初始化仓库管理器
    let repo_manager = Arc::new(RepoManager::new("bare_repos"));


    // 启动 HTTP 服务器
    // HttpServer::new(move || {
    //     App::new()
    //         .wrap(auth::token_auth::TokenAuthMiddleware)// 应用 Token 认证中间件
    //         .app_data(web::Data::new(repo_manager.clone())) // 共享 RepoManager
    //         .route("/", web::get().to(|| async { "Git Server Running" }))
    //         .configure(controller::git_controller::path_config) // 注册路由
    // })
    // .bind(("0.0.0.0", 8080))? // 绑定端口
    // .run()
    // .await

    // 加载TLS配置
    let tls_config = load_rustls_config("certs/cert.pem", "certs/key.pem").map_err(|e| {
        log::error!("Failed to load TLS config: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, "TLS config error")
    })?;

    // 创建App工厂
    let app_factory = move || {
        App::new()
            .wrap(auth::token_auth::TokenAuthMiddleware)
            .app_data(web::Data::new(repo_manager.clone()))
            .route("/", web::get().to(|| async { "Git Server Running" }))
            .configure(controller::git_controller::path_config)
    };

    // 同时启动HTTP和HTTPS服务器
    let http_server = HttpServer::new(app_factory.clone())
        .bind(("0.0.0.0", 8080))?
        .run();

    let https_server = HttpServer::new(app_factory)
        .bind_rustls(("0.0.0.0", 8443), tls_config)?
        .run();

    // 同时运行两个服务器
    tokio::try_join!(http_server, https_server)?;

    Ok(())
}
