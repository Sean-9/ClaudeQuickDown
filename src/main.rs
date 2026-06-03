mod installer;
mod mirror;

use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const VERSION: &str = "1.0.0";

// ---------------------------------------------------------------------------
// 启动状态
// ---------------------------------------------------------------------------

struct CurrentState {
    claude_version: Option<String>,   // None = 未安装
    api_key:        Option<String>,   // None = 未配置
    base_url:       Option<String>,
    model:          Option<String>,
}

impl CurrentState {
    fn detect() -> Self {
        let claude_version = detect_claude_version();
        let (api_key, base_url, model) = read_api_config();
        Self { claude_version, api_key, base_url, model }
    }

    fn is_installed(&self) -> bool {
        self.claude_version.is_some()
    }

    fn is_api_configured(&self) -> bool {
        self.api_key.as_deref().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    set_utf8_console();
    show_banner();

    let state = CurrentState::detect();

    if !state.is_installed() {
        // ── 路径 A：全新安装 ──────────────────────────────────────────────
        run_full_install().await;
    } else if !state.is_api_configured() {
        // ── 路径 B：已装但没配 API ────────────────────────────────────────
        println!("  ✅ 检测到 Claude Code 已安装（{}）",
            state.claude_version.as_deref().unwrap_or("版本未知"));
        println!("  ⚠️  尚未配置 API Key，无法正常使用。\n");
        run_api_config_only();
    } else {
        // ── 路径 C：已装已配，询问是否修改 ───────────────────────────────
        show_current_config(&state);
        if confirm("  输入 y 修改配置，回车退出 [y/N] ") {
            println!();
            run_api_config_only();
        } else {
            println!("\n  未做任何修改，再见！");
        }
    }

    wait_enter();
}

// ---------------------------------------------------------------------------
// 路径 A：完整安装流程
// ---------------------------------------------------------------------------

async fn run_full_install() {
    let npm_global = PathBuf::from(
        env::var("APPDATA").unwrap_or_else(|_| r"C:\Users\用户\AppData\Roaming".into()),
    ).join("npm");

    println!("本程序将自动完成以下安装：\n");
    println!(r"  ①  Node.js v20.12.2   →   C:\Program Files\nodejs");
    println!(r"  ②  Git 2.44.0         →   C:\Program Files\Git");
    println!("  ③  Claude Code        →   {}", npm_global.display());
    println!();
    println!("安装过程全程静默，无需手动点击任何弹窗。");
    println!("预计耗时：5 ~ 15 分钟（视网速而定）\n");

    if !confirm("确认开始安装？[y/N] ") {
        println!("\n已取消。");
        return;
    }

    println!();
    section_header("正在安装");

    step(1, 5, "测速国内镜像节点");
    let mirror = mirror::get_fastest_mirror().await;
    ok(&format!("最优镜像：{}", mirror));

    step(2, 5, "检测 / 安装 Node.js");
    if installer::is_node_sufficient() {
        ok(&format!("{} 已满足要求，跳过",
            installer::get_node_version_str().unwrap_or_default()));
    } else {
        match installer::get_node_version_str() {
            Some(v) => println!("     版本 {} 过低，将重新安装...", v),
            None    => println!("     未检测到 Node.js，开始下载..."),
        }
        match installer::install_node_executor(&mirror).await {
            Ok(_)  => { installer::refresh_environment(); ok("Node.js 安装完成"); }
            Err(e) => { fail(&format!("Node.js 安装失败：{}", e)); return; }
        }
    }

    step(3, 5, "检测 / 安装 Git");
    if installer::is_git_installed() {
        ok("Git 已安装，跳过");
    } else {
        match installer::install_git_executor(&mirror).await {
            Ok(_)  => { installer::refresh_environment(); ok("Git 安装完成"); }
            Err(e) => warn(&format!("Git 安装失败（非必须）：{}", e)),
        }
    }

    step(4, 5, "安装 Claude Code");
    let npm_global_str = npm_global.to_string_lossy().to_string();
    let registry = mirror::mirror_to_npm_registry(&mirror);
    println!("     使用 NPM 镜像：{}", registry);

    // Node.js 刚装完，路径可能还没进当前进程 PATH
    // 显式把 Node.js 安装目录 + npm global 目录都加进来
    let nodejs_dir = r"C:\Program Files\nodejs";
    let current_path = env::var("PATH").unwrap_or_default();
    let npm_path = format!("{};{};{}", nodejs_dir, npm_global_str, current_path);

    // 同步更新当前进程 PATH，让后续 Command 也能找到 node/npm
    env::set_var("PATH", &npm_path);

    // npm 在 Windows 上是 .cmd 脚本，必须通过 cmd /C 调用
    match Command::new("cmd")
        .args(["/C", "npm", "install", "-g", "@anthropic-ai/claude-code",
               &format!("--registry={}", registry)])
        .env("PATH", &npm_path)
        .output()
    {
        Ok(o) if o.status.success() => ok("Claude Code 安装完成"),
        Ok(o) => { fail(&format!("NPM 安装失败：\n{}", String::from_utf8_lossy(&o.stderr))); return; }
        Err(e) => { fail(&format!("NPM 执行异常：{}", e)); return; }
    }

    let _ = installer::inject_npm_path_to_registry(&npm_global_str);
    installer::broadcast_environment_change();
    installer::refresh_environment();

    step(5, 5, "验证安装结果");
    match detect_claude_version() {
        Some(v) => ok(&format!("claude 验证通过：{}", v)),
        None    => warn("claude 命令暂时不可用，重新打开终端后应自动生效"),
    }
    write_onboarding_flag();

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  ✅  Node.js、Git、Claude Code 全部安装完成！");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // cc-switch 可选安装
    println!();
    if confirm_ccswitch() {
        install_ccswitch(&mirror).await;
    }

    // API 配置
    println!();
    section_header("配置 API 信息（可跳过，之后重新打开本程序随时补填）");
    run_api_config_only();
}

// ---------------------------------------------------------------------------
// 路径 B / 共用：仅做 API 配置
// ---------------------------------------------------------------------------

fn run_api_config_only() {
    // 检测 cc-switch 是否在托管，如果是则给出专项提示
    if is_ccswitch_managing() {
        warn_ccswitch_managing();
        if !confirm("  仍要继续写入本程序的配置？[y/N] ") {
            println!();
            println!("  已跳过。建议按上方步骤在 cc-switch 里配置。");
            return;
        }
        println!();
    }
    section_header("填写 API 信息");
    show_platform_table();

    let api_key  = prompt_optional("  API Key（回车跳过）");
    let base_url = prompt_optional(
        "  Base URL / 中转地址（官方 Claude 直连则回车跳过）\n  DeepSeek 示例：https://api.deepseek.com/anthropic"
    );
    let model = prompt_optional(
        "  指定模型（选填，回车跳过）\n  示例：deepseek-chat  /  claude-sonnet-4-5  /  gpt-4o"
    );

    apply_api_config(&api_key, &base_url, &model);
}

// ---------------------------------------------------------------------------
// 路径 C：显示当前配置
// ---------------------------------------------------------------------------

fn show_current_config(state: &CurrentState) {
    println!("  ✅ 检测到 Claude Code 已安装（{}）\n",
        state.claude_version.as_deref().unwrap_or("版本未知"));
    println!("  当前 API 配置：");
    println!("    API Key  : {}", mask_or_empty(state.api_key.as_deref()));
    println!("    Base URL : {}", state.base_url.as_deref().unwrap_or("（未设置）"));
    println!("    模型     : {}", state.model.as_deref().unwrap_or("（未设置，使用默认）"));
    println!();
}

// ---------------------------------------------------------------------------
// 检测当前状态
// ---------------------------------------------------------------------------

fn detect_claude_version() -> Option<String> {
    Command::new("cmd")
        .args(["/C", "claude", "--version"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 从 ~/.claude/config.json 和环境变量读取当前 API 配置
fn read_api_config() -> (Option<String>, Option<String>, Option<String>) {
    // 优先读 ~/.claude/config.json
    let profile = env::var("USERPROFILE").unwrap_or_default();
    let config_path = PathBuf::from(&profile).join(".claude").join("config.json");

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        let api_key  = extract_json_string(&content, "ANTHROPIC_API_KEY");
        let base_url = extract_json_string(&content, "ANTHROPIC_BASE_URL");
        let model    = extract_json_string(&content, "ANTHROPIC_MODEL");
        if api_key.is_some() {
            return (api_key, base_url, model);
        }
    }

    // 兜底：读环境变量
    let api_key  = env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty());
    let base_url = env::var("ANTHROPIC_BASE_URL").ok().filter(|s| !s.is_empty());
    let model    = env::var("ANTHROPIC_MODEL").ok().filter(|s| !s.is_empty());
    (api_key, base_url, model)
}

/// 从 JSON 字符串里提取指定 key 的值（简单字符串解析，避免引入 serde_json）
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let pos = json.find(&needle)?;
    let after = json[pos + needle.len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    let val = &after[..end];
    if val.is_empty() { None } else { Some(val.to_string()) }
}

fn mask_or_empty(val: Option<&str>) -> String {
    match val {
        None | Some("") => "（未设置）".to_string(),
        Some(v) if v.len() <= 8 => "****".to_string(),
        Some(v) => format!("{}...{}", &v[..6], &v[v.len()-4..]),
    }
}

// ---------------------------------------------------------------------------
// API 配置写入
// ---------------------------------------------------------------------------

fn apply_api_config(api_key: &str, base_url: &str, model: &str) {
    if api_key.is_empty() && base_url.is_empty() && model.is_empty() {
        println!();
        println!("  跳过了 API 配置。");
        println!("  之后想配置时，重新打开本程序即可直接跳到此步骤。");
        return;
    }

    println!();
    println!("  正在写入配置（注册表 + ~/.claude/config.json）...\n");

    // 1. 写注册表（新开终端永久生效）
    if !api_key.is_empty()  { write_user_env_var("ANTHROPIC_API_KEY",  api_key); }
    if !base_url.is_empty() { write_user_env_var("ANTHROPIC_BASE_URL", base_url); }
    if !model.is_empty()    { write_user_env_var("ANTHROPIC_MODEL",    model); }

    // 2. 当前进程立即生效
    if !api_key.is_empty()  { env::set_var("ANTHROPIC_API_KEY",  api_key); }
    if !base_url.is_empty() { env::set_var("ANTHROPIC_BASE_URL", base_url); }
    if !model.is_empty()    { env::set_var("ANTHROPIC_MODEL",    model); }

    // 3. 写 ~/.claude/config.json
    match write_claude_config_json(api_key, base_url, model) {
        Ok(p)  => ok(&format!("~/.claude/config.json 写入成功\n       路径：{}", p.display())),
        Err(e) => warn(&format!("~/.claude/config.json 写入失败：{}", e)),
    }

    println!();
    println!("  配置完成！重新打开终端后即可使用 claude 命令。");
}

fn write_user_env_var(key: &str, value: &str) {
    match (|| -> Result<(), Box<dyn std::error::Error>> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env_key = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
        env_key.set_value(key, &value.to_string())?;
        installer::broadcast_environment_change();
        Ok(())
    })() {
        Ok(_)  => ok(&format!("注册表 {} 写入成功", key)),
        Err(e) => warn(&format!("注册表 {} 写入失败：{}", key, e)),
    }
}

fn write_claude_config_json(
    api_key: &str,
    base_url: &str,
    model: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let profile = env::var("USERPROFILE")?;
    let claude_dir = PathBuf::from(&profile).join(".claude");
    std::fs::create_dir_all(&claude_dir)?;
    let config_path = claude_dir.join("config.json");

    // 读取已有内容，保留非 ANTHROPIC_ 字段
    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut entries: Vec<(String, String)> = vec![];
    let mut in_env = false;
    for line in existing.lines() {
        let t = line.trim();
        if t.contains("\"env\"") && t.contains('{') { in_env = true; continue; }
        if in_env && (t == "}" || t == "},") { in_env = false; continue; }
        if in_env {
            if let Some(pos) = t.find(':') {
                let k = t[..pos].trim().trim_matches('"').to_string();
                let v = t[pos+1..].trim().trim_end_matches(',')
                    .trim().trim_matches('"').to_string();
                if !k.starts_with("ANTHROPIC_") {
                    entries.push((k, v));
                }
            }
        }
    }

    if !api_key.is_empty()  { entries.push(("ANTHROPIC_API_KEY".into(),  api_key.into())); }
    if !base_url.is_empty() { entries.push(("ANTHROPIC_BASE_URL".into(), base_url.into())); }
    if !model.is_empty()    { entries.push(("ANTHROPIC_MODEL".into(),    model.into())); }

    let env_lines: Vec<String> = entries.iter()
        .map(|(k, v)| format!("    \"{}\": \"{}\"", k, v))
        .collect();
    let json = format!("{{\n  \"env\": {{\n{}\n  }}\n}}\n", env_lines.join(",\n"));
    std::fs::write(&config_path, &json)?;
    Ok(config_path)
}

fn write_onboarding_flag() {
    let profile = env::var("USERPROFILE").unwrap_or_default();
    let path = PathBuf::from(&profile).join(".claude.json");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let json = if existing.trim().starts_with('{') && existing.trim().len() > 2 {
        if existing.contains("hasCompletedOnboarding") { existing }
        else {
            existing.trim_end().trim_end_matches('}').to_string()
                + ",\n  \"hasCompletedOnboarding\": true\n}"
        }
    } else {
        "{\"hasCompletedOnboarding\": true}\n".to_string()
    };
    let _ = std::fs::write(&path, json);
}

// ---------------------------------------------------------------------------
// cc-switch
// ---------------------------------------------------------------------------

fn confirm_ccswitch() -> bool {
    println!("──────────────────────────────────────────────────");
    println!("  可选：安装 cc-switch（API Key 可视化管理工具）");
    println!("──────────────────────────────────────────────────");
    println!();
    println!("  cc-switch 可以帮你：");
    println!("  • 保存并随时切换多个 API Key");
    println!("  • 可视化管理中转地址和模型配置");
    println!("  • 官网：ccswitch.io");
    println!();
    confirm("  是否同时安装 cc-switch？[y/N] ")
}

async fn install_ccswitch(mirror_url: &str) {
    println!();
    println!("  正在获取 cc-switch 最新版本...");
    let version = fetch_ccswitch_version().await.unwrap_or_else(|| "v3.15.0".to_string());
    println!("  版本：{}", version);
    let ver = version.trim_start_matches('v');
    let url = format!(
        "https://github.com/farion1231/cc-switch/releases/download/{}/CC-Switch_{}_x64_en-US.msi",
        version, ver
    );
    let temp = std::env::temp_dir().join("cc-switch-setup.msi");
    println!("  下载中...");
    match installer::download_file_public(&url, &temp).await {
        Ok(_) => {
            println!("  正在启动安装程序...");
            match Command::new("msiexec").args(["/i", temp.to_str().unwrap(), "/passive"]).spawn() {
                Ok(_)  => ok("cc-switch 安装程序已启动"),
                Err(e) => warn(&format!("无法启动：{}，请手动前往 ccswitch.io 下载", e)),
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        Err(e) => {
            warn(&format!("下载失败：{}", e));
            println!("  请手动前往 https://github.com/farion1231/cc-switch/releases 下载");
        }
    }
    let _ = mirror_url; // 暂未用于 cc-switch 加速，保留参数供后续扩展
}

async fn fetch_ccswitch_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build().ok()?;
    let text = client
        .get("https://api.github.com/repos/farion1231/cc-switch/releases/latest")
        .header("User-Agent", "ClaudeQuickDown")
        .send().await.ok()?
        .text().await.ok()?;
    let tag = text.split("\"tag_name\":").nth(1)?.split('"').nth(1)?.to_string();
    Some(tag)
}


// ---------------------------------------------------------------------------
// cc-switch 状态检测
// ---------------------------------------------------------------------------

/// 检测 cc-switch 是否正在托管 Claude Code 配置
/// 判断依据：~/.claude.json 里存在 "primaryApiKey": "any"
fn is_ccswitch_managing() -> bool {
    let profile = env::var("USERPROFILE").unwrap_or_default();
    let path = PathBuf::from(&profile).join(".claude.json");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    // cc-switch 写入的标志字段
    content.contains("\"primaryApiKey\"") && content.contains("\"any\"")
}

fn warn_ccswitch_managing() {
    println!();
    println!("  ┌─────────────────────────────────────────────────┐");
    println!("  │  [!!] 检测到 cc-switch 正在托管 Claude Code      │");
    println!("  └─────────────────────────────────────────────────┘");
    println!();
    println!("  cc-switch 在后台运行时，会持续将它管理的 provider");
    println!("  配置写回 ~/.claude/config.json，覆盖本程序的设置。");
    println!();
    println!("  ── 推荐做法 ──────────────────────────────────────");
    println!("  在 cc-switch 中添加并激活你的 DeepSeek provider：");
    println!("    1. 打开 cc-switch");
    println!("    2. 点击「添加 Provider」");
    println!("    3. 类型选 Custom（Anthropic 兼容）");
    println!("    4. 填入 API Key 和 Base URL");
    println!("       Base URL: https://api.deepseek.com/anthropic");
    println!("    5. 点击激活");
    println!();
    println!("  ── 或者 ──────────────────────────────────────────");
    println!("  退出 cc-switch 托盘后，本程序的写入将持续生效。");
    println!();
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

fn step(n: u8, total: u8, msg: &str) { println!("\n  [{}/{}] {}...", n, total, msg); }
fn ok(msg: &str)   { println!("     [OK]  {}", msg); }
fn warn(msg: &str) { println!("     [!!]  {}", msg); }
fn fail(msg: &str) { eprintln!("     [ERR] {}", msg); }

fn show_platform_table() {
    println!("  支持以下平台，选一个填入即可：\n");
    println!("  {:<24} {:<42} {}", "平台", "Key 格式示例", "获取 / 文档");
    println!("  {}", "─".repeat(90));
    println!("  {:<24} {:<42} {}", "Anthropic (Claude)",         "sk-ant-api03-xxxxxxxx",              "console.anthropic.com");
    println!("  {:<24} {:<42} {}", "DeepSeek [推荐]",            "sk-xxxxxxxxxxxxxxxx",                "platform.deepseek.com");
    println!("  {:<24} {:<42} {}", "  └ DeepSeek Base URL",      "https://api.deepseek.com/anthropic", "api-docs.deepseek.com/zh-cn");
    println!("  {:<24} {:<42} {}", "OpenAI / GPT",               "sk-proj-xxxxxxxx",                  "platform.openai.com");
    println!("  {:<24} {:<42} {}", "智谱 AI (GLM)",              "xxxxxxxx.xxxxxxxxxxxxxxxx",          "open.bigmodel.cn");
    println!("  {:<24} {:<42} {}", "月之暗面 (Kimi)",            "sk-xxxxxxxxxxxxxxxx",                "platform.moonshot.cn");
    println!("  {:<24} {:<42} {}", "阿里 百炼 (通义)",           "sk-xxxxxxxxxxxxxxxx",                "bailian.console.aliyun.com");
    println!();
}

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

fn set_utf8_console() {
    let _ = Command::new("cmd").args(["/C", "chcp", "65001"]).output();
}

fn wait_enter() {
    println!("\n按回车键关闭窗口...");
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
}
