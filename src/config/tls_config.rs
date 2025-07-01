use anyhow::{Context, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
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
    let cert_file = File::open(cert_path).context("Failed to open certificate file")?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();
    
    // 加载私钥
    let key_file = File::open(key_path).context("Failed to open private key file")?;
    info!("加载私钥");
    let mut key_reader = BufReader::new(key_file);
    let key = match pkcs8_private_keys(&mut key_reader)?.into_iter().next() {
        Some(key) => PrivateKey(key),
        None => anyhow::bail!("No private key found"),
    };
    info!("创建TLS配置");
    // 创建TLS配置
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .context("Failed to create TLS configuration")?;

    Ok(config)
}
