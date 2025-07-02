use anyhow::{Context, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, Item, read_one};  // 使用 read_one 函数
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use log::{info};
use rustls::version::{TLS12}; 

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
    // 创建TLS配置 - 使用兼容性更好的设置
    let mut config = ServerConfig::builder()
        .with_safe_default_cipher_suites() // 使用安全默认值
        .with_safe_default_kx_groups()     // 使用安全默认值
        .with_protocol_versions(&[&TLS12])? // 只使用 TLS 1.2 提高兼容性
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .context("创建TLS配置失败")?;

    // 同时支持 HTTP/2 和 HTTP/1.1
    config.alpn_protocols = vec![
        b"http/1.1".to_vec()  // 只支持 HTTP/1.1 提高兼容性
    ];
    info!("已设置ALPN协议为HTTP/1.1");
    
    Ok(config)
}
