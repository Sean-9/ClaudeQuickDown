# ClaudeQuickDown

> **Claude Code 国内一键安装器** — 双击即用，无需任何技术基础

---

## 这是什么？

Claude Code 是 Anthropic 推出的 AI 编程助手，可以在终端里帮你写代码、改 Bug、读项目。

官方安装流程需要手动配置 Node.js、npm、网络代理等，对非技术用户不友好。

**ClaudeQuickDown 做的事就一件**：你只需要双击、填 API Key（可跳过），剩下的全自动搞定。

---

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10 / Windows 11（64位） |
| 网络 | 能访问互联网即可，国内镜像加速 |
| 磁盘空间 | 约 500 MB |
| 权限 | 需要管理员权限（程序自动请求） |

---

## 下载

在本页面的 [Releases](../../releases) 下载 `ClaudeQuickDown.exe`，双击运行即可。

> Windows 弹出「Windows 已保护你的电脑」→ 点击「仍要运行」即可（未购买代码签名证书，正常现象）。

---

## 工作流程

程序启动时自动检测当前状态，走对应路径：

```
启动
  │
  ├─ Claude Code 未安装 → 完整安装流程（路径A）
  │
  ├─ 已安装 + 未配置 API Key → 直接跳到 API 配置（路径B）
  │
  └─ 已安装 + 已配置 → 显示当前配置，y 修改 / 回车退出（路径C）
```

### 路径 A：全新安装

```
╔══════════════════════════════════════════════════╗
║   ClaudeQuickDown  v1.0.6                        ║
║   Claude Code 国内一键安装器                     ║
╚══════════════════════════════════════════════════╝

本程序将自动完成以下安装：

  ①  Node.js v20.12.2   →   C:\Program Files\nodejs
  ②  Git 2.44.0         →   C:\Program Files\Git
  ③  Claude Code        →   C:\Users\用户名\AppData\Roaming\npm

安装过程全程静默，无需手动点击任何弹窗。
预计耗时：5 ~ 15 分钟（视网速而定）

确认开始安装？[y/N]
```

安装步骤：测速镜像 → Node.js → Git → Claude Code → 验证

安装完成后可选安装 **cc-switch**（API Key 可视化管理工具，支持 DeepSeek / Claude / GPT 等多平台切换）。

### 路径 B / C：API 配置

```
──────────────────────────────────────────────────
  填写 API 信息
──────────────────────────────────────────────────

  支持以下平台，选一个填入即可：

  平台                    Key 格式示例                         获取 / 文档
  ──────────────────────────────────────────────────────────────
  Anthropic (Claude)      sk-ant-api03-xxxxxxxx                console.anthropic.com
  DeepSeek [推荐]         sk-xxxxxxxxxxxxxxxx                  platform.deepseek.com
    └ DeepSeek Base URL   https://api.deepseek.com/anthropic  api-docs.deepseek.com/zh-cn
  OpenAI / GPT           sk-proj-xxxxxxxx                      platform.openai.com
  智谱 AI (GLM)          xxxxxxxxxxxxxxxxxxxxxxxx              open.bigmodel.cn
  月之暗面 (Kimi)        sk-xxxxxxxxxxxxxxxx                  platform.moonshot.cn
  阿里 百炼 (通义)       sk-xxxxxxxxxxxxxxxx                  bailian.console.aliyun.com

  API Key（回车跳过）
  > sk-xxxxxxxxxxxxxxxx

  Base URL / 中转地址（官方 Claude 直连则回车跳过）
  > https://api.deepseek.com/anthropic
```

三项均可回车跳过，跳过时打印 `setx` 命令提示，方便以后手动补填。

> **cc-switch 提示**：如果检测到 `cc-switch` 正在托管配置，程序会提示你直接在 cc-switch 中添加 DeepSeek provider，避免两边配置互相覆盖。

**配置写入三重保险**：
1. 注册表（HKEY_CURRENT_USER\Environment）— 新开终端永久生效
2. 当前进程环境变量 — 立即生效
3. `~/.claude/config.json` — Claude Code 直接读取，无需重启

---

## 安装完成后

**重新打开一个终端**（CMD 或 PowerShell），输入：

```
claude
```

看到欢迎界面即表示安装成功。

---

## 常见问题

**Q：提示「Windows 已保护你的电脑」怎么办？**

点击「更多信息」→「仍要运行」。这是因为程序没有购买代码签名证书，属于正常现象。

---

**Q：安装过程卡住不动了？**

网络下载可能较慢，耐心等待。程序在下载失败时会自动重试最多 3 次，请不要手动关闭窗口。

---

**Q：我已经装过 Node.js / Git 了，会被卸载吗？**

不会。程序会先检测已安装版本：
- Node.js ≥ 18：直接跳过
- Node.js < 18：提示后重新安装
- Git：已安装则跳过

---

**Q：DeepSeek 推荐用什么配置？**

| 字段 | 值 |
|------|-----|
| API Key | DeepSeek 平台生成的 Key（`sk-...`） |
| Base URL | `https://api.deepseek.com/anthropic` |
| 模型 | `deepseek-chat` 或 `deepseek-reasoner` |

---

**Q：安装在哪里？**

| 软件 | 安装位置 |
|------|---------|
| Node.js | `C:\Program Files\nodejs` |
| Git | `C:\Program Files\Git` |
| Claude Code | `C:\Users\用户名\AppData\Roaming\npm` |
| API 配置 | 用户环境变量（注册表） + `~/.claude/config.json` |

---

**Q：如何卸载？**

- Node.js / Git：在「控制面板 → 程序和功能」正常卸载
- Claude Code：终端执行 `npm uninstall -g @anthropic-ai/claude-code`
- API 配置：在「系统属性 → 高级 → 环境变量」里手动删除，或编辑 `~/.claude/config.json`

---

**Q：API Key 安全吗？**

你的 Key 只写入本机注册表和环境变量，程序本身不联网发送任何个人信息。可查看源代码自行验证。

---

## 使用协议

仅限个人学习、研究与交流使用，禁止任何形式的商业用途。
