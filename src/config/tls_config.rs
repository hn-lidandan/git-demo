use anyhow::{Context, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{Item, read_one};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use log::{info};


pub fn load_rustls_config(
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
) -> Result<ServerConfig> {
    info!("进入证书加载");
    
    // 加载证书链
    let cert_file = File::open(cert_path).context("打开证书文件失败")?;
    let mut cert_reader = BufReader::new(cert_file);
    let mut cert_chain = vec![];
    
    // 确保加载完整证书链
    while let Ok(Some(item)) = rustls_pemfile::read_one(&mut cert_reader) {
        if let Item::X509Certificate(cert) = item {
            cert_chain.push(Certificate(cert));
        }
    }
    
    // 加载私钥
    let key_file = File::open(key_path).context("打开私钥文件失败")?;
    info!("加载私钥");
    let mut key_reader = BufReader::new(key_file);
    
    let key = match read_one(&mut key_reader)? {
        Some(Item::PKCS8Key(key)) => PrivateKey(key),
        Some(Item::RSAKey(key)) => PrivateKey(key),
        Some(Item::ECKey(key)) => PrivateKey(key),
        _ => anyhow::bail!("无法识别的私钥格式"),
    };
    
    info!("创建TLS配置");
    // 创建TLS配置 - 使用兼容模式
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .context("创建TLS配置失败")?;

    // 不设置 ALPN，让客户端选择
    info!("不设置ALPN协议，使用客户端协商");
    
    Ok(config)
}
