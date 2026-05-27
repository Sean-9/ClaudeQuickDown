fn main() {
    // 仅在编译 Windows 目标时嵌入 UAC manifest
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("ClaudeQuickDown.manifest");
        // 可选：设置文件属性（任务管理器 / 属性面板可见）
        res.set("ProductName",      "ClaudeQuickDown");
        res.set("FileDescription",  "Claude Code 国内一键安装器");
        res.set("LegalCopyright",   "MIT License");
        res.set("ProductVersion",   "1.0.0");
        res.compile().expect("winres 编译失败");
    }
}
