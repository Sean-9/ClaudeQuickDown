//! Aniox 自动化安装器
//!
//! 负责 Node.js / Git 的版本检测、下载（含重试）、静默安装，
//! 以及 PATH 注册表注入和环境变量广播。

use futures::StreamExt;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};
use winreg::enums::*;
use winreg::RegKey;

const NODE_LTS_VERSION: &str = "v20.12.2";
const NODE_MIN_MAJOR: u32 = 18; // Claude-Code 最低要求
const GIT_VERSION: &str = "v2.44.0.windows.1";
const MSI_NAME: &str = "node_setup.msi";
const GIT_EXE_NAME: &str = "Git-2.44.0-64-bit.exe";
const MIN_MSI_SIZE_BYTES: u64 = 20_000_000;
const MIN_GIT_EXE_SIZE_BYTES: u64 = 40_000_000;
const DOWNLOAD_MAX_ATTEMPTS: u32 = 3;

// ---------------------------------------------------------------------------
// Node.js 版本检测
// ---------------------------------------------------------------------------

/// 解析 `node --version` 输出，返回 (major, minor, patch)
pub fn get_node_version() -> Option<(u32, u32, u32)> {
    let output = Command::new("node")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())?;

    let raw = String::from_utf8_lossy(&output.stdout);
    // 格式形如 "v20.12.2\n"
    let s = raw.trim().trim_start_matches('v');
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts[2].parse().ok()?;
    Some((major, minor, patch))
}

/// 返回原始版本字符串（如 "v20.12.2"），用于打印
pub fn get_node_version_str() -> Option<String> {
    Command::new("node")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// 当前 Node.js 是否满足最低版本要求（major >= NODE_MIN_MAJOR）
pub fn is_node_sufficient() -> bool {
    get_node_version()
        .map(|(major, _, _)| major >= NODE_MIN_MAJOR)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 旧版 Node.js 残留清理（仅在真正需要重装时调用）
// ---------------------------------------------------------------------------

fn clean_nodejs_registry() {
    println!("   [清理] 正在扫描旧版 Node.js 注册表残留...");
    clean_nodejs_in_path(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    );
    clean_nodejs_in_path(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products",
    );
    clean_nodejs_in_path(
        HKEY_CURRENT_USER,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    );
    println!("   [清理] 完成");
}

fn clean_nodejs_in_path(hkey: winreg::HKEY, path: &str) {
    let root = RegKey::predef(hkey);
    if let Ok(key) = root.open_subkey(path) {
        for name in key.enum_keys().filter_map(|k| k.ok()) {
            if let Ok(subkey) = key.open_subkey(&name) {
                if let Ok(display) = subkey.get_value::<String, _>("DisplayName") {
                    if display.contains("Node.js") {
                        println!("   [清理] 发现残留: {}", display);
                        if let Ok(ps) = subkey.get_value::<String, _>("ProductCode") {
                            // 等待卸载完成，再继续安装新版本
                            let _ = Command::new("msiexec")
                                .args(["/x", &ps, "/qn", "/norestart"])
                                .spawn()
                                .and_then(|mut c| c.wait());
                            println!("   [清理] 已卸载: {}", ps);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 环境刷新（跨进程 PATH 生效）
// ---------------------------------------------------------------------------

/// 从注册表读取系统 + 用户 Path，合并后写入当前进程，
/// 使得后续 Command::new("node") / "npm" 等能找到新安装的工具。
pub fn refresh_environment() {
    let system_path = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment")
        .and_then(|k| k.get_value::<String, _>("Path"))
        .unwrap_or_default();

    let user_path = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Environment")
        .and_then(|k| k.get_value::<String, _>("Path"))
        .unwrap_or_default();

    let merged = match (system_path.is_empty(), user_path.is_empty()) {
        (true, _) => user_path.clone(),
        (_, true) => system_path.clone(),
        _ => format!("{};{}", system_path, user_path),
    };

    std::env::set_var("PATH", &merged);
    println!(
        "   [PATH] 已刷新（系统 {} chars，用户 {} chars）",
        system_path.len(),
        user_path.len()
    );
}

// ---------------------------------------------------------------------------
// 镜像 URL 映射
// ---------------------------------------------------------------------------

fn mirror_url_to_nodejs_base(mirror_url: &str) -> String {
    if mirror_url.contains("huaweicloud") {
        "https://mirrors.huaweicloud.com/nodejs".to_string()
    } else if mirror_url.contains("cloud.tencent") {
        "https://mirrors.cloud.tencent.com/nodejs-release".to_string()
    } else {
        "https://npmmirror.com/mirrors/node".to_string()
    }
}

fn mirror_url_to_git_base(mirror_url: &str) -> String {
    if mirror_url.contains("huaweicloud") {
        "https://mirrors.huaweicloud.com/git-for-windows".to_string()
    } else {
        "https://npmmirror.com/mirrors/git-for-windows".to_string()
    }
}

fn build_nodejs_url(base: &str, version: &str) -> String {
    format!("{}/{}/node-{}-x64.msi", base, version, version)
}

fn build_git_url(base: &str, version: &str) -> String {
    format!("{}/{}/{}", base, version, GIT_EXE_NAME)
}

// ---------------------------------------------------------------------------
// 下载（带进度，每 5% 打印一次）
// ---------------------------------------------------------------------------

pub async fn download_file_public(
    url: &str,
    dest: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    if total_size == 0 {
        return Err("镜像服务器返回文件大小为 0，文件可能不存在".into());
    }

    println!(
        "\n   [下载] 开始（预计 {:.1} MB）...",
        total_size as f64 / 1024.0 / 1024.0
    );

    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_pct: i32 = -1;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;

        let pct = ((downloaded as f64 / total_size as f64) * 100.0) as i32;
        if pct / 5 != last_pct / 5 {
            print!(
                "\r   [下载] {:3}% ({:.1}/{:.1} MB)    ",
                pct,
                downloaded as f64 / 1024.0 / 1024.0,
                total_size as f64 / 1024.0 / 1024.0
            );
            last_pct = pct;
        }
    }
    println!(); // 换行
    Ok(())
}

/// 带重试的下载：失败后指数退避（2s → 4s），最多 DOWNLOAD_MAX_ATTEMPTS 次
async fn download_with_retry(
    url: &str,
    dest: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut last_err: Box<dyn std::error::Error + Send + Sync> =
        "未知错误".to_string().into();

    for attempt in 1..=DOWNLOAD_MAX_ATTEMPTS {
        // 每次重试前先删掉残留文件
        let _ = tokio::fs::remove_file(dest).await;

        match download_file_public(url, dest).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                if attempt < DOWNLOAD_MAX_ATTEMPTS {
                    let wait_secs = 2u64.pow(attempt - 1) * 2; // 2s, 4s
                    eprintln!(
                        "\n   ⚠️  第 {}/{} 次下载失败（{}），{}s 后重试...",
                        attempt, DOWNLOAD_MAX_ATTEMPTS, last_err, wait_secs
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }
            }
        }
    }

    Err(format!(
        "下载失败（已重试 {} 次）：{}",
        DOWNLOAD_MAX_ATTEMPTS, last_err
    )
    .into())
}

// ---------------------------------------------------------------------------
// Node.js 安装
// ---------------------------------------------------------------------------

/// 下载并静默安装 Node.js LTS（仅在版本不满足要求时调用）
pub async fn install_node_executor(
    mirror_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 清理旧版（等待卸载完成后再装新版）
    clean_nodejs_registry();

    let base = mirror_url_to_nodejs_base(mirror_url);
    let url = build_nodejs_url(&base, NODE_LTS_VERSION);
    println!("   🌐 下载源：{}", url);

    let msi_path = std::env::temp_dir().join(MSI_NAME);
    download_with_retry(&url, &msi_path).await?;

    let size = tokio::fs::metadata(&msi_path).await?.len();
    if size < MIN_MSI_SIZE_BYTES {
        let _ = tokio::fs::remove_file(&msi_path).await;
        return Err(format!(
            "文件大小异常：{:.1} MB（预期 > {:.1} MB）",
            size as f64 / 1024.0 / 1024.0,
            MIN_MSI_SIZE_BYTES as f64 / 1024.0 / 1024.0
        )
        .into());
    }

    println!("   🔧 正在安装 Node.js {}...", NODE_LTS_VERSION);
    let status = Command::new("msiexec")
        .args([
            "/i",
            msi_path.to_str().unwrap(),
            "/quiet",
            "/norestart",
        ])
        .spawn()?
        .wait()?;

    let _ = tokio::fs::remove_file(&msi_path).await;

    if !status.success() {
        return Err(format!("安装进程退出码 {}", status.code().unwrap_or(-1)).into());
    }

    println!("   ✅ Node.js 安装成功");
    Ok(())
}

// ---------------------------------------------------------------------------
// Git 安装
// ---------------------------------------------------------------------------

pub fn is_git_installed() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 下载并全自动静默安装 Git for Windows（仅在未安装时调用）
pub async fn install_git_executor(
    mirror_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let base = mirror_url_to_git_base(mirror_url);
    let url = build_git_url(&base, GIT_VERSION);
    println!("   🌐 下载源：{}", url);

    let git_exe_path = std::env::temp_dir().join(GIT_EXE_NAME);
    download_with_retry(&url, &git_exe_path).await?;

    let size = tokio::fs::metadata(&git_exe_path).await?.len();
    if size < MIN_GIT_EXE_SIZE_BYTES {
        let _ = tokio::fs::remove_file(&git_exe_path).await;
        return Err(format!(
            "文件大小异常：{:.1} MB（预期 > {:.1} MB）",
            size as f64 / 1024.0 / 1024.0,
            MIN_GIT_EXE_SIZE_BYTES as f64 / 1024.0 / 1024.0
        )
        .into());
    }

    println!("   🔧 正在安装 Git {}...", GIT_VERSION);
    let status = Command::new(&git_exe_path)
        .args([
            "/VERYSILENT",
            "/NORESTART",
            "/SUPPRESSMSGBOXES",  // 禁止任何弹窗（避免 PATH 过长弹窗卡住流程）
            "/TASKS=!modifypath", // 不让 Git 安装器自己动 PATH，我们手动加
        ])
        .spawn()?
        .wait()?;

    let _ = tokio::fs::remove_file(&git_exe_path).await;

    if !status.success() {
        return Err(format!("安装进程退出码 {}", status.code().unwrap_or(-1)).into());
    }

    // Git 安装器没有修改 PATH，我们手动把 Git 的 cmd 目录加进去
    let git_cmd_dir = r"C:\Program Files\Git\cmd".to_string();
    let git_bin_dir = r"C:\Program Files\Git\bin".to_string();
    println!("   [PATH] 手动注入 Git 路径...");
    let _ = inject_npm_path_to_registry(&git_cmd_dir);
    let _ = inject_npm_path_to_registry(&git_bin_dir);

    println!("   ✅ Git 安装成功");
    Ok(())
}

// ---------------------------------------------------------------------------
// NPM PATH 硬注入
// ---------------------------------------------------------------------------

pub fn inject_npm_path_to_registry(
    npm_path: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
    let current_path: String = env_key.get_value("Path").unwrap_or_default();

    let normalized_npm = npm_path.trim_end_matches(|c| c == '\\' || c == '/');
    let already_exists = current_path.split(';').filter(|p| !p.is_empty()).any(|p| {
        p.trim_end_matches(|c| c == '\\' || c == '/').eq_ignore_ascii_case(normalized_npm)
    });

    let new_path = if already_exists {
        println!("   [PATH] 已存在，无需重复写入");
        return Ok(());
    } else if current_path.is_empty() {
        npm_path.to_string()
    } else {
        format!("{};{}", current_path, npm_path)
    };

    env_key.set_value("Path", &new_path)?;
    println!(
        "   [PATH] 注入完成（共 {} 个路径）",
        new_path.split(';').filter(|p| !p.is_empty()).count()
    );
    Ok(())
}

pub fn broadcast_environment_change() {
    let env_ptr: Vec<u16> = std::ffi::OsStr::new("Environment")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(env_ptr.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }
    println!("   [广播] 环境变量变更已通知系统");
}
