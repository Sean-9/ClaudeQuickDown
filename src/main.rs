mod installer;
mod mirror;

use std::env;
use std::io::{self, BufRead, Write};
use std::process::Command;

const VERSION: &str = "1.0.0";

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
    println!("预计耗时：5 ~ 15 分钟（视网速而定）");
    println!();

    if !confirm("确认开始安装？[y/N] ") {
        println!("\n已取消。");
        return wait_enter();
    }

    // ── 安装阶段 ───────────────────────────────────────────────────────────
    section_header("安装进度");

    // 1. 测速
    step(1, 5, "测速国内镜像节点");
    let fastest_mirror = mirror::get_fastest_mirror().await;
    ok(&format!("最优镜像：{}", fastest_mirror));

    // 2. Node.js
    step(2, 5, "检测 / 安装 Node.js");
    if installer::is_node_sufficient() {
        let ver = installer::get_node_version_str().unwrap_or_default();
        ok(&format!("{} 已满足要求（≥ v18），跳过", ver));
    } else {
        match installer::get_node_version_str() {
            Some(v) => println!("     版本 {} 过低，将重新安装...", v),
            None    => println!("     未检测到 Node.js，开始下载安装..."),
        }
        match installer::install_node_executor(&fastest_mirror).await {
            Ok(_)  => { installer::refresh_environment(); ok("Node.js 安装完成"); }
            Err(e) => { fail(&format!("Node.js 安装失败：{}", e)); return wait_enter(); }
        }
    }

    // 3. Git
    step(3, 5, "检测 / 安装 Git");
    if installer::is_git_installed() {
        ok("Git 已安装，跳过");
    } else {
        println!("     未检测到 Git，开始下载安装...");
        match installer::install_git_executor(&fastest_mirror).await {
            Ok(_)  => { installer::refresh_environment(); ok("Git 安装完成"); }
            Err(e) => warn(&format!("Git 安装失败（非必须，继续）：{}", e)),
        }
    }

    // 4. Claude Code
    step(4, 5, "安装 Claude Code");
    let npm_global_str = npm_global.to_string_lossy().to_string();
    let registry       = mirror::mirror_to_npm_registry(&fastest_mirror);
    println!("     使用 NPM 镜像：{}", registry);

    let npm_path = format!("{};{}", npm_global_str, env::var("PATH").unwrap_or_default());
    match Command::new("cmd")
        .args([
            "/C", "npm", "install", "-g", "@anthropic-ai/claude-code",
            &format!("--registry={}", registry),
        ])
        .env("PATH", &npm_path)
        .output()
    {
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

    // 5. 验证
    step(5, 5, "验证安装结果");
    let verified = Command::new("cmd")
        .args(["/C", "claude", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if verified {
        ok("claude 命令验证通过 ✓");
    } else {
        warn("claude 命令暂时不可用，重新打开终端后应自动生效");
    }

    // 跳过新手引导
    let profile = env::var("USERPROFILE").unwrap_or_default();
    let _ = std::fs::write(
        std::path::PathBuf::from(&profile).join(".claude.json"),
        r#"{"hasCompletedOnboarding": true}"#,
    );

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  ✅  Node.js、Git、Claude Code 已全部安装完成！");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // ── API 配置（装完再收集，可跳过）────────────────────────────────────
    println!();
    section_header("配置 API 信息（可跳过，之后随时补填）");
    setup_api_interactive();

    // ── 完成 ───────────────────────────────────────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   🎉  全部完成！                                 ║");
    println!("║                                                  ║");
    println!("║   重新打开一个终端（CMD / PowerShell），输入：   ║");
    println!("║       claude                                     ║");
    println!("║   即可开始使用 Claude Code。                     ║");
    println!("╚══════════════════════════════════════════════════╝");
    wait_enter();
}

// ---------------------------------------------------------------------------
// API 配置交互
// ---------------------------------------------------------------------------

fn setup_api_interactive() {
    // 平台参考表
    println!("  支持以下平台，选一个填入即可：\n");
    println!("  {:<22} {:<36} {}", "平台", "Key 格式示例", "获取地址");
    println!("  {}", "─".repeat(90));
    println!("  {:<22} {:<36} {}", "Anthropic (Claude)",  "sk-ant-api03-xxxxxxxx",         "console.anthropic.com");
    println!("  {:<22} {:<36} {}", "OpenAI / GPT",        "sk-proj-xxxxxxxx",              "platform.openai.com");
    println!("  {:<22} {:<36} {}", "DeepSeek",            "sk-xxxxxxxxxxxxxxxx",           "platform.deepseek.com");
    println!("  {:<22} {:<36} {}", "智谱 AI (GLM/ChatGLM)","xxxxxxxx.xxxxxxxxxxxxxxxx",    "open.bigmodel.cn");
    println!("  {:<22} {:<36} {}", "月之暗面 (Kimi)",     "sk-xxxxxxxxxxxxxxxx",           "platform.moonshot.cn");
    println!("  {:<22} {:<36} {}", "阿里 百炼 (通义)",    "sk-xxxxxxxxxxxxxxxx",           "bailian.console.aliyun.com");
    println!();

    let api_key = prompt_optional("  API Key（回车跳过）");
    let base_url = prompt_optional(
        "  中转 / Base URL（使用官方直连则回车跳过）\n  示例：https://api.example.com",
    );
    let model = prompt_optional(
        "  指定模型（选填，回车跳过）\n  示例：claude-sonnet-4-5  /  deepseek-chat  /  gpt-4o",
    );

    println!();

    let mut wrote_any = false;
    if !api_key.is_empty()  { set_env_var("ANTHROPIC_API_KEY",  &api_key);  wrote_any = true; }
    if !base_url.is_empty() { set_env_var("ANTHROPIC_BASE_URL", &base_url); wrote_any = true; }
    if !model.is_empty()    { set_env_var("ANTHROPIC_MODEL",    &model);    wrote_any = true; }

    if !wrote_any {
        println!("  （已跳过，之后需要配置时可运行以下命令）");
        println!();
        println!(r#"  setx ANTHROPIC_API_KEY  "你的Key""#);
        println!(r#"  setx ANTHROPIC_BASE_URL "中转地址（可选）""#);
        println!(r#"  setx ANTHROPIC_MODEL    "模型名称（可选）""#);
    } else {
        ok("环境变量已写入，重新打开终端后生效");
    }
}

// ---------------------------------------------------------------------------
// UI 工具
// ---------------------------------------------------------------------------

fn show_banner() {
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   ClaudeQuickDown  v{}                        ║", VERSION);
    println!("║   Claude Code 国内一键安装器                     ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
}

fn section_header(title: &str) {
    println!("──────────────────────────────────────────────────");
    println!("  {}", title);
    println!("──────────────────────────────────────────────────");
    println!();
}

fn step(n: u8, total: u8, msg: &str) {
    println!("\n  [{}/{}] {}...", n, total, msg);
}

fn ok(msg: &str)   { println!("     ✅  {}", msg); }
fn warn(msg: &str) { println!("     ⚠️   {}", msg); }
fn fail(msg: &str) { eprintln!("     ❌  {}", msg); }

fn confirm(prompt_str: &str) -> bool {
    print!("  {}", prompt_str);
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).unwrap();
    s.trim().eq_ignore_ascii_case("y")
}

fn prompt_optional(label: &str) -> String {
    println!("{}", label);
    print!("  > ");
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).unwrap();
    println!();
    s.trim().to_string()
}

fn set_env_var(key: &str, value: &str) {
    match Command::new("cmd").args(["/C", "setx", key, value]).output() {
        Ok(o) if o.status.success() =>
            ok(&format!("{} 已写入", key)),
        Ok(o) =>
            warn(&format!("{} 写入失败：{}", key, String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) =>
            warn(&format!("{} 写入失败：{}", key, e)),
    }
}

fn wait_enter() {
    println!("\n按回车键关闭窗口...");
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
}
