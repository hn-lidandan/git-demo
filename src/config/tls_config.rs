use anyhow::{Context, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, Item, read_one};  // 使用 read_one 函数
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
    let cert_chain = certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();
    
    // 加载私钥 - 使用 read_one 函数
    let key_file = File::open(key_path).context("打开私钥文件失败")?;
    info!("加载私钥");
    let mut key_reader = BufReader::new(key_file);
    
    // 使用 read_one 自动检测格式
    let key = match read_one(&mut key_reader)? {
        Some(Item::PKCS8Key(key)) => PrivateKey(key),
        Some(Item::RSAKey(key)) => PrivateKey(key),
        Some(Item::ECKey(key)) => PrivateKey(key),
        _ => anyhow::bail!("无法识别的私钥格式"),
    };
    
    info!("创建TLS配置");
    // 创建TLS配置
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .context("创建TLS配置失败")?;

    Ok(config)
}
