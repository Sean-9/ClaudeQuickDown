mod installer;
mod mirror;

use std::env;
use std::io::{self, BufRead, Write};
use std::process::Command;

const VERSION: &str = "1.0.0";

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    show_banner();

    // ── 欢迎 + 安装清单 ────────────────────────────────────────────────────
    let npm_global = std::path::PathBuf::from(
        env::var("APPDATA").unwrap_or_else(|_| r"C:\Users\用户\AppData\Roaming".into()),
    )
    .join("npm");

    println!("本程序将自动完成以下安装：\n");
    println!(r"  ①  Node.js v20.12.2   →   C:\Program Files\nodejs");
    println!(r"  ②  Git 2.44.0         →   C:\Program Files\Git");
    println!("  ③  Claude Code        →   {}", npm_global.display());
    println!();
    println!("安装过程全程静默，无需手动点击任何弹窗。");
    println!("预计耗时：5 ~ 15 分钟（视网速而定）\n");

    press_enter_to_continue();

    // ── 第 1 步：收集 API 信息 ─────────────────────────────────────────────
    section_header("第 1 步 / 3", "填写 API 信息");

    let api_key  = collect_api_key();
    let base_url = collect_optional(
        "API 中转地址（国内代理用户填写，其余直接回车跳过）",
        "示例：https://api.example.com",
    );
    let model = collect_optional(
        "指定模型（选填，回车跳过使用 Claude 默认）",
        "示例：claude-sonnet-4-5",
    );

    // ── 第 2 步：确认 ──────────────────────────────────────────────────────
    section_header("第 2 步 / 3", "确认安装信息");

    println!("  API Key    : {}", mask_key(&api_key));
    println!(
        "  中转地址   : {}",
        if base_url.is_empty() { "（未填，直连 Anthropic）".into() } else { base_url.clone() }
    );
    println!(
        "  模型       : {}",
        if model.is_empty() { "（默认）".into() } else { model.clone() }
    );
    println!();

    if !confirm("以上信息无误，开始安装？[y/N] ") {
        println!("\n已取消。");
        return;
    }

    // ── 第 3 步：安装 ──────────────────────────────────────────────────────
    section_header("第 3 步 / 3", "正在安装");

    // 3-1 测速
    step(1, 5, "测速国内镜像节点");
    let fastest_mirror = mirror::get_fastest_mirror().await;
    ok(&format!("最优镜像：{}", fastest_mirror));

    // 3-2 Node.js
    step(2, 5, "检测 / 安装 Node.js");
    if installer::is_node_sufficient() {
        let ver = installer::get_node_version_str().unwrap_or_default();
        ok(&format!("{} 已满足要求（≥ v18），跳过", ver));
    } else {
        match installer::get_node_version_str() {
            Some(v) => println!("     版本 {} 过低，将重新安装...", v),
            None    => println!("     未检测到 Node.js，开始安装..."),
        }
        match installer::install_node_executor(&fastest_mirror).await {
            Ok(_)  => { installer::refresh_environment(); ok("Node.js 安装完成"); }
            Err(e) => { fail(&format!("Node.js 安装失败：{}", e)); return wait_enter(); }
        }
    }

    // 3-3 Git
    step(3, 5, "检测 / 安装 Git");
    if installer::is_git_installed() {
        ok("Git 已安装，跳过");
    } else {
        match installer::install_git_executor(&fastest_mirror).await {
            Ok(_)  => { installer::refresh_environment(); ok("Git 安装完成"); }
            Err(e) => warn(&format!("Git 安装失败（非必须，继续）：{}", e)),
        }
    }

    // 3-4 Claude Code
    step(4, 5, "安装 Claude Code");
    let npm_global_str = npm_global.to_string_lossy().to_string();
    let registry      = mirror::mirror_to_npm_registry(&fastest_mirror);
    println!("     使用 NPM 镜像：{}", registry);

    let npm_path = format!("{};{}", npm_global_str, env::var("PATH").unwrap_or_default());
    let result = Command::new("cmd")
        .args([
            "/C", "npm", "install", "-g", "@anthropic-ai/claude-code",
            &format!("--registry={}", registry),
        ])
        .env("PATH", &npm_path)
        .output();

    match result {
        Ok(o) if o.status.success() => ok("Claude Code 安装完成"),
        Ok(o) => {
            fail(&format!("NPM 安装失败：\n{}", String::from_utf8_lossy(&o.stderr)));
            return wait_enter();
        }
        Err(e) => {
            fail(&format!("NPM 执行异常：{}", e));
            return wait_enter();
        }
    }

    // PATH 注入 + 广播
    let _ = installer::inject_npm_path_to_registry(&npm_global_str);
    installer::broadcast_environment_change();
    installer::refresh_environment();

    // 3-5 写入环境变量 + 验证
    step(5, 5, "写入环境变量 & 验证");

    set_env_var("ANTHROPIC_API_KEY", &api_key);
    if !base_url.is_empty() { set_env_var("ANTHROPIC_BASE_URL", &base_url); }
    if !model.is_empty()    { set_env_var("ANTHROPIC_MODEL", &model); }

    // 跳过新手引导
    let profile = env::var("USERPROFILE").unwrap_or_default();
    let _ = std::fs::write(
        std::path::PathBuf::from(&profile).join(".claude.json"),
        r#"{"hasCompletedOnboarding": true}"#,
    );

    let verified = Command::new("cmd")
        .args(["/C", "claude", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if verified {
        ok("claude 命令验证通过");
    } else {
        warn("claude 命令暂时不可用，重新打开终端后应自动生效");
    }

    // ── 完成 ───────────────────────────────────────────────────────────────
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  🎉  全部完成！");
    println!();
    println!("  重新打开一个终端（CMD / PowerShell），输入：");
    println!("      claude");
    println!("  即可开始使用 Claude Code。");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    wait_enter();
}

// ---------------------------------------------------------------------------
// UI 工具函数
// ---------------------------------------------------------------------------

fn show_banner() {
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   ClaudeQuickDown  v{}                        ║", VERSION);
    println!("║   Claude Code 国内一键安装器                     ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
}

fn section_header(step: &str, title: &str) {
    println!();
    println!("──────────────────────────────────────────────────");
    println!("  {}：{}", step, title);
    println!("──────────────────────────────────────────────────");
    println!();
}

fn step(n: u8, total: u8, msg: &str) {
    println!("\n  [{}/{}] {}...", n, total, msg);
}

fn ok(msg: &str)   { println!("     ✅  {}", msg); }
fn warn(msg: &str) { println!("     ⚠️   {}", msg); }
fn fail(msg: &str) { eprintln!("     ❌  {}", msg); }

fn press_enter_to_continue() {
    print!("按回车继续，Ctrl+C 退出...");
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).unwrap();
    println!();
}

fn prompt(label: &str, hint: &str) -> String {
    if !hint.is_empty() {
        println!("  {}", hint);
    }
    print!("  {} > ", label);
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).unwrap();
    s.trim().to_string()
}

fn collect_api_key() -> String {
    println!("  API Key（必填）");
    println!("  获取地址：https://console.anthropic.com\n");
    loop {
        let v = prompt("API Key", "");
        if !v.is_empty() {
            println!();
            return v;
        }
        println!("  ⚠️  API Key 不能为空，Claude Code 无法在没有 Key 的情况下运行。\n");
    }
}

fn collect_optional(label: &str, hint: &str) -> String {
    println!("  {}", label);
    let v = prompt("", hint);
    println!();
    v
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".into();
    }
    format!("{}...{}", &key[..6], &key[key.len()-4..])
}

fn confirm(prompt_str: &str) -> bool {
    print!("  {}", prompt_str);
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).unwrap();
    s.trim().eq_ignore_ascii_case("y")
}

fn set_env_var(key: &str, value: &str) {
    match Command::new("cmd").args(["/C", "setx", key, value]).output() {
        Ok(o) if o.status.success() => ok(&format!("{} 已写入", key)),
        Ok(o) => warn(&format!("{} 写入失败：{}", key, String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => warn(&format!("{} 写入失败：{}", key, e)),
    }
}

fn wait_enter() {
    println!("\n按回车键关闭窗口...");
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
}
