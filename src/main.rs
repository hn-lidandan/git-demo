// minimal_server.rs
use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use rustls::{ServerConfig, Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::io::BufReader;
use std::fs::File;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello World!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 加载TLS配置
    let cert_file = File::open("/etc/letsencrypt/live/git-demo.duckdns.org/fullchain.pem").unwrap();
    let key_file = File::open("/etc/letsencrypt/live/git-demo.duckdns.org/privkey.pem").unwrap();

    let cert_chain = certs(&mut BufReader::new(cert_file))
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();
    
    let mut keys = pkcs8_private_keys(&mut BufReader::new(key_file)).unwrap();
    let key = PrivateKey(keys.remove(0));

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .unwrap();

    // 启动最小化服务
    HttpServer::new(|| App::new().service(hello))
        .bind_rustls("0.0.0.0:8443", config)?
        .run()
        .await
}
