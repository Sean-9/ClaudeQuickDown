//! NPM 镜像源测速模块
//!
//! 并发探测多个镜像源，返回响应最快的节点 URL。
//! 同时提供镜像 URL → npm registry URL 的映射。

use futures::future;
use reqwest::Client;
use std::time::Duration;

const MIRRORS: &[&str] = &[
    "https://registry.npmmirror.com",
    "https://mirrors.cloud.tencent.com/npm/",
    "https://mirrors.huaweicloud.com/repository/npm/",
];

const TIMEOUT_SECS: u64 = 3;
const FALLBACK: &str = "https://registry.npmmirror.com";

async fn check_mirror(url: &str) -> Result<String, ()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|_| ())?;

    client
        .head(url)
        .send()
        .await
        .map_err(|_| ())
        .and_then(|resp| {
            if resp.status().is_success() {
                Ok(url.to_string())
            } else {
                Err(())
            }
        })
}

pub async fn get_fastest_mirror() -> String {
    let futures: Vec<_> = MIRRORS
        .iter()
        .map(|url| Box::pin(check_mirror(url)))
        .collect();

    match future::select_ok(futures).await {
        Ok((result, _)) => result,
        Err(_) => FALLBACK.to_string(),
    }
}

/// 将测速得到的镜像 URL 映射为 npm --registry 参数值
pub fn mirror_to_npm_registry(mirror_url: &str) -> &'static str {
    if mirror_url.contains("huaweicloud") {
        "https://mirrors.huaweicloud.com/repository/npm"
    } else if mirror_url.contains("cloud.tencent") {
        "https://mirrors.cloud.tencent.com/npm/"
    } else {
        "https://registry.npmmirror.com"
    }
}
