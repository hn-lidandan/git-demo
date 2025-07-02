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
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    
    // 初始化仓库管理器
    let repo_manager = Arc::new(RepoManager::new("bare_repos"));

    // 加载TLS配置
    let tls_config = load_rustls_config(
        "/etc/letsencrypt/live/git-demo.duckdns.org/fullchain.pem", 
        "/etc/letsencrypt/live/git-demo.duckdns.org/privkey.pem"
    ).map_err(|e| {
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

    // 启动HTTP服务器
    let http_server = HttpServer::new(app_factory.clone())
        .bind(("0.0.0.0", 80))?
        .run();

    // 启动HTTPS服务器并强制使用HTTP/1.1
    let https_server = HttpServer::new(app_factory)
        .bind_rustls(("0.0.0.0", 443), tls_config)?
        .client_disconnect_timeout(std::time::Duration::from_secs(10))
        // .http1() // 强制使用HTTP/1.1
        .run();

    // 同时运行两个服务器（使用显式类型注解） 
    tokio::try_join!(http_server, https_server)?;
    // https_server.await?;
    Ok(())
}
