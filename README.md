# ClaudeQuickDown

> **Claude Code 国内一键安装器** — 双击即用，无需任何技术基础

---

## 这是什么？

Claude Code 是 Anthropic 推出的 AI 编程助手，可以在终端里帮你写代码、改 Bug、读项目。

官方安装流程需要手动配置 Node.js、npm、网络代理等，对非技术用户不友好。

**ClaudeQuickDown 做的事就一件**：你只需要双击、填一个 API Key，剩下的全自动搞定。

---

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10 / Windows 11（64位） |
| 网络 | 能访问互联网即可，无需挂代理 |
| 磁盘空间 | 约 500 MB |
| 权限 | 需要管理员权限（程序会自动弹出请求） |

---

## 使用方法

### 第一步：获取 API Key

前往 [https://console.anthropic.com](https://console.anthropic.com) 注册并创建 API Key。

Key 格式形如：`sk-ant-api03-xxxxxxxxxx`

> 如果你使用的是国内中转服务（非 Anthropic 官方），中转商会提供他们自己的 Key 和地址。

---

### 第二步：下载并运行

1. 在本页面的 [Releases](../../releases) 找到最新版本
2. 下载 `ClaudeQuickDown.exe`
3. **双击运行**
4. Windows 弹出「是否允许此应用更改设备？」→ 点击**是**

---

### 第三步：按提示操作

程序会显示如下界面，按提示填写即可：

```
╔══════════════════════════════════════════════════╗
║   ClaudeQuickDown  v1.0.0                        ║
║   Claude Code 国内一键安装器                     ║
╚══════════════════════════════════════════════════╝

本程序将自动完成以下安装：

  ①  Node.js v20.12.2   →   C:\Program Files\nodejs
  ②  Git 2.44.0         →   C:\Program Files\Git
  ③  Claude Code        →   C:\Users\你的用户名\AppData\Roaming\npm

按回车继续...

──────── 第 1 步 / 3：填写 API 信息 ──────────────

  API Key（必填）
  > sk-ant-api03-...          ← 在这里粘贴你的 Key

  API 中转地址（选填）
  > https://api.xxx.com       ← 如果有国内代理填这里，否则直接回车

  指定模型（选填，回车跳过）
  >                           ← 不确定就直接回车

──────── 第 2 步 / 3：确认安装 ────────────────────

  输入 y 开始安装，其他键退出
  > y
```

之后等待 5 ~ 15 分钟，程序会自动下载安装所有依赖。

---

### 第四步：开始使用

安装完成后，**重新打开一个终端**（搜索「CMD」或「PowerShell」），输入：

```
claude
```

看到欢迎界面即表示安装成功 🎉

---

## 常见问题

**Q：提示「Windows 已保护你的电脑」怎么办？**

点击「更多信息」→「仍要运行」。这是因为程序没有购买代码签名证书，属于正常现象。

---

**Q：安装过程卡住不动了？**

网络下载可能较慢，耐心等待。程序在下载失败时会自动重试最多 3 次，请不要手动关闭窗口。

---

**Q：我已经装过 Node.js / Git 了，会被卸载吗？**

不会。程序会先检测你已安装的版本：
- Node.js 版本 ≥ 18：直接跳过，不动你的环境
- Node.js 版本 < 18（太旧）：会提示并重新安装较新版本
- Git：已安装则直接跳过

---

**Q：安装在哪里？**

| 软件 | 默认安装位置 |
|------|-------------|
| Node.js | `C:\Program Files\nodejs` |
| Git | `C:\Program Files\Git` |
| Claude Code | `C:\Users\用户名\AppData\Roaming\npm` |
| API 配置 | 写入系统用户环境变量（可在「系统属性」里查看） |

---

**Q：如何卸载？**

- Node.js / Git：在「控制面板 → 程序和功能」正常卸载
- Claude Code：打开终端执行 `npm uninstall -g @anthropic-ai/claude-code`
- API Key 等环境变量：在「系统属性 → 高级 → 环境变量」里手动删除

---

**Q：API Key 安全吗？会上传到哪里吗？**

你的 Key 只会通过 `setx` 命令写入你本机的用户环境变量，程序本身不联网发送任何个人信息。你可以查看[源代码](./src/main.rs)自行验证。

---

## 开源协议

MIT License — 可自由使用、修改、分发。
