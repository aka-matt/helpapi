# helpapi 用户指南

> 一份面向使用者和二次开发者的完整中文使用文档。
>
> 项目名：**helpapi**（仓库内 CLI 二进制名为 `mock-api`）
> 语言：简体中文
> 适用版本：与本仓库 `main` 分支同步

---

## 目录

1. [项目简介](#1-项目简介)
2. [主要能力与适用场景](#2-主要能力与适用场景)
3. [架构总览](#3-架构总览)
4. [快速上手](#4-快速上手)
5. [编译与构建](#5-编译与构建)
6. [调试与测试](#6-调试与测试)
7. [CLI 命令详解](#7-cli-命令详解)
8. [配置文件（JSON）完整参考](#8-配置文件json完整参考)
9. [匹配器（match）详解](#9-匹配器match详解)
10. [动作（action）详解](#10-动作action详解)
11. [转换器（transform）详解](#11-转换器transform详解)
12. [HTTPS / TLS 配置](#12-https--tls-配置)
13. [TUI 交互界面](#13-tui-交互界面)
14. [可观测性与日志](#14-可观测性与日志)
15. [可复用的 WASM 引擎](#15-可复用的-wasm-引擎)
16. [常见问题与排错](#16-常见问题与排错)
17. [示例合集](#17-示例合集)

---

## 1. 项目简介

`helpapi` 是一个用 **Rust** 编写的 **Mock API CLI 工具**，同时具备 **WASM 发布能力**。它的核心目标是：

- 从一份 JSON 规则配置中加载 Mock 规则；
- 启动本地 HTTP / HTTPS 服务器；
- 将传入请求按规则匹配，并二选一：
  - **直接返回本地预设响应**（mock）；
  - **透明转发到上游 API**，并可在转发前后修改请求 / 响应的 Header、Query、Path 和 JSON Body；
- 提供 **Ratatui 终端界面**（TUI），实时查看请求列表、详情与配置状态；
- 将**核心规则匹配、配置校验、转换能力**编译成 WASM 包，供浏览器、Node.js 或其它宿主程序复用。

简单来说：**helpapi = 一个可热重载、可观测、可嵌入的 Mock + 反向代理 + JSON Patch 工具**。

---

## 2. 主要能力与适用场景

### 2.1 主要能力

| 能力 | 说明 |
| --- | --- |
| 静态 Mock 响应 | 根据 Method / Path / Header / Query / Body 匹配，返回固定 JSON、Text 或 Binary 内容 |
| 动态参数提取 | Path 中支持 `:param` 形式（如 `/users/:id`），可被响应模板引用 |
| 上游转发 | 命中转发规则后，将请求转发到 `upstream`，并执行请求 / 响应转换链 |
| Header / Query 改写 | 添加、移除、覆写请求和响应的 Header / Query 参数 |
| JSON Pointer 改写 | 基于 [RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901) 的 JSON Pointer 修改 JSON Body |
| 配置热重载 | 修改 JSON 文件后自动重新加载，无需重启服务（失败时保留旧配置） |
| HTTPS / JKS | 支持使用 JKS（Java KeyStore）证书库启动 HTTPS 服务 |
| Ratatui TUI | 本地调试时可在终端中查看请求列表、详情、过滤、清空、帮助 |
| WASM 复用 | 规则引擎单独打包，可在浏览器或 Node.js 中独立运行（不依赖网络） |

### 2.2 适用场景

- 前端联调时无需等待真实后端接口完成；
- 后端开发时构造可控的边界场景（错误码、超时、空数据、特殊 Header）；
- 编写集成测试时替代外部依赖，避免 “雪花” 测试结果；
- 在浏览器或 Node.js 中复用规则，对静态请求做即时决策；
- 演示、调试 API 路径时记录最近请求与对应决策。

---

## 3. 架构总览

### 3.1 Workspace 目录

```
helpapi/
├── Cargo.toml                      # 工作区根清单
├── crates/
│   ├── mock-core/                  # 纯业务逻辑（无 IO / 网络 / 终端依赖）
│   ├── mock-config/                # 配置模型、解析、校验、JSON Schema
│   ├── mock-http/                  # HTTP 服务器（Axum）+ 上游客户端（Reqwest）+ TLS
│   ├── mock-runtime/               # 生命周期、热重载、事件分发
│   ├── mock-cli/                   # 命令行入口 + Ratatui TUI
│   ├── mock-wasm/                  # WASM 绑定（供浏览器 / Node.js 使用）
│   └── mock-integration-tests/     # 集成测试
├── examples/                       # 可直接拿来跑的示例配置
├── docs/                           # 设计与规划文档
├── helpapi.md                      # 中文实施文档（架构、设计决策来源）
├── CLAUDE.md                       # 给 AI 助手的项目说明
└── user_guide.md                   # 本文件
```

### 3.2 Crate 依赖关系

```
mock-cli        ─┐
                 ├─→ mock-runtime ─→ mock-http ─┬─→ mock-core
mock-wasm ───────┘                               └─→ mock-config
```

**核心约束**：`mock-core` 不依赖任何网络、终端、CLI、WASM 运行时；它只处理纯数据和决策逻辑。

### 3.3 请求处理数据流

```
JSON 配置文件
   ↓ (mock-cli 读取)
mock-config 解析 + 校验
   ↓
mock-runtime 构建 Engine
   ↓
mock-http 监听 HTTP / HTTPS
   ↓
进入请求 → 转 RequestData → mock-core.decide
   ├── Decision::Mock       → 直接构造响应返回
   ├── Decision::Forward    → 上游请求 + request_transforms
                              → 上游响应 + response_transforms
                              → 返回给客户端
   └── Decision::Reject      → 返回错误响应
```

---

## 4. 快速上手

> 假设你已经克隆了仓库并进入根目录。

### 4.1 准备环境

- Rust 工具链 **1.85+**（推荐使用 `rustup`）；
- Linux / macOS / Windows 均可；
- WASM 构建额外需要 [`wasm-pack`](https://rustwasm.github.io/wasm-pack/)。

```bash
# 安装 Rust（如果还没有）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 wasm-pack（可选，仅在编译 WASM 时需要）
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### 4.2 用发布模式构建 CLI

```bash
cargo build --release
```

构建产物位于 `target/release/mock-api`（Windows 下为 `mock-api.exe`）。

### 4.3 启动一个内置示例

```bash
./target/release/mock-api run --config examples/basic.json
```

启动后默认监听 `127.0.0.1:8081`。测试一下：

```bash
curl -i http://127.0.0.1:8081/posts/1
```

你将得到来自上游 `https://jsonplaceholder.typicode.com` 的响应（因为示例中 `/posts/:id` 是转发规则）。

### 4.4 用 TUI 模式查看请求

```bash
./target/release/mock-api run --config examples/basic.json --mode tui
```

终端会进入 Ratatui 界面，实时显示请求列表、详情和快捷键提示。

### 4.5 验证 JSON 配置

```bash
./target/release/mock-api validate --config examples/transforms.json
```

合法时输出 `Config is valid`，否则打印结构化错误（含 JSON 路径）。

---

## 5. 编译与构建

### 5.1 常用构建命令

```bash
# 仅 CLI（debug，速度最快）
cargo build -p mock-cli

# 整个工作区（release）
cargo build --workspace --release

# 运行全部单元测试 + 集成测试
cargo test --workspace --all-features

# 格式化（CI 必跑）
cargo fmt --all --check

# Lint（CI 必跑，零警告策略）
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 生成文档
cargo doc --workspace --no-deps

# 依赖审计（需要 cargo-deny）
cargo deny check
```

### 5.2 CI 等价脚本

仓库内置 CI 等价命令：

```bash
cargo fmt --all --check \
  && cargo clippy --workspace --all-targets --all-features -- -D warnings \
  && cargo test --workspace --all-features \
  && cargo doc --workspace --no-deps \
  && cargo deny check
```

### 5.3 仅构建某个 crate

```bash
cargo build -p mock-core        # 纯核心
cargo build -p mock-wasm --target wasm32-unknown-unknown
cargo build -p mock-http        # 含 HTTP 服务
```

### 5.4 WASM 构建

```bash
# 通用：wasm-pack bundler 产物，适合 webpack / vite / rollup 等打包工具
wasm-pack build crates/mock-wasm --target bundler

# Node.js 专用
wasm-pack build crates/mock-wasm --target nodejs

# 直接通过 <script type="module"> 在浏览器使用
wasm-pack build crates/mock-wasm --target web
```

构建产物默认输出到 `crates/mock-wasm/pkg/`，包含：

- `helpapi_mock_wasm_bg.wasm`
- `helpapi_mock_wasm.js` / `.d.ts`
- `helpapi_mock_wasm_bg.js`（仅 bundler 模式）

### 5.5 交叉编译（可选）

```bash
# Linux x86_64
cargo build --release --target x86_64-unknown-linux-musl

# macOS（需在 macOS 上执行）
cargo build --release --target aarch64-apple-darwin

# Windows（需在 Windows 上执行）
cargo build --release --target x86_64-pc-windows-msvc
```

---

## 6. 调试与测试

### 6.1 运行单元测试

```bash
cargo test -p mock-core            # 核心引擎
cargo test -p mock-config          # 配置解析 / 校验
cargo test -p mock-http            # HTTP 集成测试
cargo test -p mock-runtime         # 运行时
cargo test -p mock-cli             # CLI 行为
cargo test -p mock-wasm            # WASM 绑定（host 测试）
cargo test --workspace             # 一次性全部
```

### 6.2 WASM 测试

```bash
wasm-pack test --node crates/mock-wasm
```

### 6.3 调试日志

`run` 子命令支持 `--log-format pretty|json`，并默认读取环境变量 `RUST_LOG`：

```bash
# 调试级别日志，结构化 JSON 输出
RUST_LOG=mock_api=debug,tower_http=info \
  ./target/release/mock-api run --config examples/basic.json --log-format json
```

可用日志等级：`trace, debug, info, warn, error`。

### 6.4 调试请求转换

当某个 mock 不生效或转换结果不符合预期时，按以下顺序排查：

1. **运行 `validate`**：先确认配置语法与语义正确；
   ```bash
   ./target/release/mock-api validate --config examples/your.json
   ```
2. **运行 `print-effective-config`**：查看 “经过默认值填充后” 的最终配置，确认你的字段被正确解析；
   ```bash
   ./target/release/mock-api print-effective-config --config examples/your.json
   ```
3. **打开 debug 日志**：观察匹配器是否命中、转换是否执行；
4. **启用 TUI**：在 `--mode tui` 下观察每条请求命中的规则 ID、是否报错、最终状态码；
5. **调整 `priority`**：当多条规则可能同时命中时，使用更高的 `priority` 强制优先；
6. **路径参数**：注意 `:param` 提取的键名为 param 名（如 `:id` → `path_params.id`）。

### 6.5 调试 WASM

- 在 Node.js 中打印 JSON 结果：`console.log(JSON.parse(engine.decide(reqJson)))`；
- 错误对象通常是 `Error("Config error: ...") / Compile error: ...")`，从消息前缀即可定位；
- 浏览器中可在 DevTools 控制台检查导入的 WASM 模块是否成功（`init()` 是否 resolve）。

---

## 7. CLI 命令详解

CLI 入口为 `mock-api`（见 `crates/mock-cli/src/main.rs`）。

### 7.1 全局说明

```bash
mock-api --help
mock-api <SUBCOMMAND> --help
```

支持的子命令：

| 子命令 | 作用 |
| --- | --- |
| `run` | 启动服务（前台运行，监听 `Ctrl+C`） |
| `validate` | 仅校验 JSON 配置，不启动服务 |
| `schema` | 生成 JSON Schema（用于编辑器自动补全） |
| `print-effective-config` | 打印应用默认值后的最终配置 |
| `version` | 打印版本号 |

### 7.2 `run`

```bash
mock-api run --config <PATH> [--log-format pretty|json] [--mode standard|tui]
```

| 参数 | 是否必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `--config, -c` | 是 | — | JSON 配置文件路径 |
| `--log-format` | 否 | `pretty` | `pretty` 人类可读；`json` 行式 JSON |
| `--mode` | 否 | `standard` | `standard` 前台输出日志；`tui` 进入 Ratatui 终端界面 |

行为：

- 解析并校验配置；
- 启动 HTTP / HTTPS 服务；
- 监听配置文件变更，保存即热重载（失败保留旧配置）；
- 收到 `Ctrl+C` 后优雅关闭。

### 7.3 `validate`

```bash
mock-api validate --config <PATH>
```

- 成功：打印 `Config is valid`，退出码 `0`；
- 失败：打印 `Config is invalid: <错误信息>`，退出码 `1`。

### 7.4 `schema`

```bash
mock-api schema --output <PATH>   # 写入文件
mock-api schema                    # 输出到 stdout
```

输出与 `mock-config` 一致的 JSON Schema，可用于编辑器扩展（如 VSCode 的 JSON Schema 引用）或 CI 中配置合规校验。

### 7.5 `print-effective-config`

```bash
mock-api print-effective-config --config <PATH>
```

读取配置 → 应用默认值 → 以格式化 JSON 输出。常用于调试 “为什么我的字段没生效”。

### 7.6 `version`

```bash
mock-api version
```

打印 `mock-api version <CARGO_PKG_VERSION>`。

---

## 8. 配置文件（JSON）完整参考

> 顶级结构定义见 `crates/mock-config/src/models.rs`。
> 语义校验规则见 `crates/mock-config/src/validate.rs`。

### 8.1 顶层结构

```json
{
  "version": 1,
  "server": { ... },
  "defaults": { ... },
  "routes": [ ... ],
  "unmatched": { ... } // 可选
}
```

| 字段 | 类型 | 必填 | 作用 |
| --- | --- | --- | --- |
| `version` | u32 | 是 | 配置格式版本，当前必须为 `1` |
| `server` | object | 是 | 监听地址 / 端口 / TLS |
| `defaults` | object | 是 | 全局默认值（超时、Body 大小） |
| `routes` | array | 是 | 路由规则列表（可为空数组） |
| `unmatched` | object | 否 | 未匹配时的兜底动作 |

### 8.2 `server`

```json
{
  "host": "127.0.0.1",
  "port": 8080,
  "keystore_file": "examples/test-keystore.jks",
  "keystore_password": "changeit",
  "key_password": "changeit"
}
```

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `host` | string | 是 | 监听地址；默认 `127.0.0.1`；公开监听建议 `0.0.0.0` 并自行评估风险 |
| `port` | u16 | 是 | 监听端口 |
| `keystore_file` | string | 否 | 启用 HTTPS：JKS 证书库路径；存在时启用 TLS |
| `keystore_password` | string | 当设置了 `keystore_file` 时必填 | JKS 完整性密码（明文） |
| `key_password` | string | 否 | 私钥密码；缺省回退到 `keystore_password` |

> 默认值：监听 `127.0.0.1`，端口 `8080`；不启用 TLS。

### 8.3 `defaults`

```json
{
  "upstream_timeout_ms": 10000,
  "max_body_bytes": 1048576
}
```

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `upstream_timeout_ms` | u64 | `10000` | 上游请求超时（毫秒） |
| `max_body_bytes` | usize | `1048576` (1 MiB) | 请求 / 响应 Body 最大字节数；超出返回 `413 Payload Too Large` |

### 8.4 `routes[]`

每条路由形如：

```json
{
  "id": "get-user",
  "priority": 100,
  "match_rule": { "method": "GET", "path": "/users/:id" },
  "action": { "type": "mock", "response": { ... } }
}
```

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | **必须全局唯一**，用于决策解释 |
| `priority` | i32 | 越大越先匹配；相同 `priority` 时按声明顺序；建议保持非负 |
| `match_rule` | object | 匹配条件，至少要有一种条件；详见 [第 9 节](#9-匹配器match详解) |
| `action` | object | 命中后执行的动作；详见 [第 10 节](#10-动作action详解) |

### 8.5 `action`

#### 8.5.1 `mock`

```json
{
  "type": "mock",
  "response": {
    "status": 200,
    "headers": { "Content-Type": "application/json" },
    "json_body": { "id": 1, "name": "Alice" },
    "text_body": null,
    "delay_ms": null
  }
}
```

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `status` | u16 | HTTP 状态码 |
| `headers` | object | 响应 Header（键名大小写不敏感） |
| `json_body` | object/null | JSON 响应体；与 `text_body` 至少设其一 |
| `text_body` | string/null | 纯文本响应体 |
| `delay_ms` | u64/null | 发送前的延迟毫秒数（用于模拟慢响应） |

#### 8.5.2 `forward`

```json
{
  "type": "forward",
  "upstream": "https://api.example.com",
  "request_transforms": [ ... ],
  "response_transforms": [ ... ]
}
```

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `upstream` | string | 是 | 上游基础 URL；必须以 `http://` 或 `https://` 开头，且包含 host |
| `request_transforms` | array | 否 | 转发前的请求转换链，按数组顺序依次执行 |
| `response_transforms` | array | 否 | 上游响应返回后的转换链 |

> 注意：转发时 `host / content-length / connection` 等 hop-by-hop Header 会被重算或剥离，避免上游出现歧义。

### 8.6 `unmatched`

兜底配置——当所有 `routes` 都没有匹配时使用。

```json
{
  "action": {
    "type": "mock",
    "response": {
      "status": 404,
      "headers": { "Content-Type": "application/json" },
      "json_body": { "error": "Not Found" }
    }
  }
}
```

也可设为 `forward`（全局 fallback upstream），但需要谨慎使用以免被误用为开放代理。

### 8.7 校验错误代码

| 代码 | 严重度 | 说明 | 建议修复 |
| --- | --- | --- | --- |
| `INVALID_VERSION` | Error | `version` 不为 `1` | 改为 `1` |
| `TLS_PASSWORD_WITHOUT_KEYSTORE` | Error | 设置了 TLS 密码但未设置 `keystore_file` | 添加 `keystore_file` 或移除 TLS 密码 |
| `MISSING_KEYSTORE_PASSWORD` | Error | 设置了 `keystore_file` 但未提供 `keystore_password` | 提供 `keystore_password` |
| `DUPLICATE_ROUTE_ID` | Error | `routes[].id` 重复 | 给每条规则一个唯一 ID |
| `NEGATIVE_PRIORITY` | Warning | `priority < 0` | 改成非负值 |
| `INVALID_UPSTREAM_URL` | Error | `upstream` 不是合法 URL | 用 `http://` 或 `https://` 开头 |
| `INVALID_JSON_POINTER` | Error | JSON Pointer 格式错误 | 按 RFC 6901 编写（必须以 `/` 开头） |
| `EMPTY_MATCH_RULE` | Warning | 没有任何匹配条件 | 至少添加 `method` 或 `path` |

---

## 9. 匹配器（match）详解

每条规则的 `match_rule` 是一个对象，包含以下可选字段，**至少需要一个**：

```json
{
  "method": "GET",
  "path": "/users/:id",
  "headers": { "X-Env": "dev" },
  "query": { "lang": "zh" },
  "body": { "json": { "type": "premium" } }
}
```

| 字段 | 类型 | 匹配规则 |
| --- | --- | --- |
| `method` | string | 精确匹配，大小写不敏感 |
| `path` | string | 路径模式，支持 `:param` 段、`*` 后缀通配 |
| `headers` | object | 所有出现的键值必须相等（大小写不敏感） |
| `query` | object | 所有出现的键值必须相等 |
| `body` | object | Body 匹配：`{ "json": {...} }` / `{ "text": "..." }` / `{ "binary": "..." }` |

### 9.1 Path 模式语法

- 字面量段：`/users` 只匹配 `/users`；
- 参数段：`/users/:id` 匹配 `/users/42`，并把 `id=42` 写入 `path_params`；
- 多参数：`/users/:user_id/posts/:post_id`；
- 后缀通配：`/static/*` 匹配 `/static/` 后任意路径；
- 根路径：`/` 匹配 `/`。

### 9.2 评分机制（仅在 WASM / mock-core 内部使用）

- 每个字面段 +10；
- 每个参数段 +15；
- 包含至少一个参数段 +20 特异性加成；
- 通配段不加分。

**优先级仍然由规则的 `priority` 字段决定**——评分仅在决策解释中可见。

### 9.3 多条件组合

所有给出的条件必须**全部成立**才算匹配（AND 语义）。例如：

```json
{
  "method": "POST",
  "path": "/users/:id",
  "headers": { "Authorization": "Bearer xxx" }
}
```

只匹配 POST + 指定路径 + 带正确 Header 的请求。

---

## 10. 动作（action）详解

### 10.1 `mock`

直接返回响应，可附带延迟模拟：

```json
{
  "type": "mock",
  "response": {
    "status": 201,
    "headers": { "Content-Type": "application/json" },
    "json_body": { "id": 4, "created": true },
    "delay_ms": 500
  }
}
```

### 10.2 `forward`

执行转发 + 转换链：

```json
{
  "type": "forward",
  "upstream": "https://api.example.com",
  "request_transforms": [
    { "type": "set_header", "name": "X-Forwarded-By", "value": "helpapi" }
  ],
  "response_transforms": [
    { "type": "set_json_pointer", "path": "/_mock", "value": true }
  ]
}
```

执行流程：

1. 解析客户端请求 → `RequestData`；
2. 按 `request_transforms` 顺序改写；
3. 用 `upstream` 拼装目标 URL，发起 HTTP 请求；
4. 收到上游响应 → 按 `response_transforms` 顺序改写；
5. 返回给客户端。

---

## 11. 转换器（transform）详解

转换器在 `mock-core/src/transform/spec.rs` 中以 snake_case tagged enum 形式定义：

```json
{ "type": "set_header", "name": "X-Foo", "value": "bar" }
```

支持的转换器：

| 类型 | 字段 | 作用 |
| --- | --- | --- |
| `set_header` | `name`, `value` | 添加 / 覆写 Header |
| `remove_header` | `name` | 移除 Header |
| `set_query` | `name`, `value` | 添加 / 覆写 Query 参数 |
| `remove_query` | `name` | 移除 Query 参数 |
| `set_json_pointer` | `path`, `value` | 在 JSON Body 上按 RFC 6901 设置值 |
| `remove_json_pointer` | `path` | 删除 JSON Body 上某路径 |
| `replace_body` | `body` | 整体替换 Body（`{ "json": ... }` / `{ "text": "..." }` / `{ "binary": "..." }` / `{ "empty": null }`） |
| `set_status` | `status` | 仅响应转换可用：改写状态码 |

### 11.1 JSON Pointer 注意事项

- 必须以 `/` 开头；
- 数组下标是数字：`/items/0`；
- 路径中如出现 `/` 或 `~`，需用 `~1` / `~0` 转义；
- 非 JSON Body（如 `text_body`）遇到 `set_json_pointer` / `remove_json_pointer` 会**直接报错**（默认严格模式）。

示例：

```json
{
  "type": "set_json_pointer",
  "path": "/user/profile/name",
  "value": "Alice"
}
```

### 11.2 完整示例：请求与响应转换

```json
{
  "id": "proxy-and-tag",
  "priority": 100,
  "match_rule": { "method": "POST", "path": "/orders" },
  "action": {
    "type": "forward",
    "upstream": "https://api.example.com",
    "request_transforms": [
      { "type": "set_header", "name": "X-Source", "value": "helpapi" },
      { "type": "remove_header", "name": "cookie" },
      { "type": "set_json_pointer", "path": "/source", "value": "local-cli" }
    ],
    "response_transforms": [
      { "type": "set_header", "name": "X-Proxied-By", "value": "helpapi" },
      { "type": "set_status", "status": 200 },
      { "type": "set_json_pointer", "path": "/_mock", "value": true }
    ]
  }
}
```

---

## 12. HTTPS / TLS 配置

helpapi 通过 JKS（Java KeyStore）文件启用 TLS。**密码以明文形式存储于配置文件中**，这是当前的设计选择。

### 12.1 启用步骤

1. 准备 `.jks` 文件（含私钥与证书）；
2. 在 `server` 中设置：
   ```json
   {
     "host": "0.0.0.0",
     "port": 8443,
     "keystore_file": "examples/test-keystore.jks",
     "keystore_password": "changeit",
     "key_password": "changeit"
   }
   ```
3. 启动：
   ```bash
   ./target/release/mock-api run --config examples/https.json
   ```
4. 使用 `curl -k` 跳过证书校验进行本地测试：
   ```bash
   curl -k https://localhost:8443/users/1
   ```

### 12.2 仓库自带示例

`examples/https.json` 与 `examples/test-keystore.jks`（自签名 `*.localhost` / `localhost` 泛域名证书，密码 `changeit`）已随仓库提供，可直接用于功能验证。

### 12.3 安全建议

- 生产环境请使用由受信 CA 签发的证书；
- 不要把含真实密码的配置文件提交到公共仓库；
- 不要把 `keystore_file` 指向能被外部写入的目录；
- 转发时配置项里**不能**携带 TLS 密码的转发（避免被代理泄漏）。

---

## 13. TUI 交互界面

通过 `mock-api run --mode tui` 进入。

### 13.1 布局

TUI 通常由以下面板组成（具体见 `crates/mock-cli/src/tui/widgets/`）：

- **状态栏（Status）**：显示运行状态、监听地址、规则数；
- **请求列表（Requests）**：最近请求的状态、方法、路径、耗时；
- **请求详情（Details）**：命中的规则、Action 类型、上游地址、错误信息；
- **帮助栏（Help）**：快捷键提示。

### 13.2 快捷键

| 按键 | 作用 |
| --- | --- |
| `q` | 退出（同时停止服务） |
| `r` | 重新加载配置文件 |
| `c` | 清空请求历史 |
| `j` / `↓` | 向下选择 |
| `k` / `↑` | 向上选择 |
| `/` | 进入过滤输入 |
| `Tab` | 切换面板 |
| `Enter` | 查看详情 |
| `?` | 切换帮助 |

### 13.3 敏感 Header 遮罩

TUI 与日志中默认会遮罩以下 Header：

- `authorization`
- `proxy-authorization`
- `cookie`
- `set-cookie`

显示为 `Authorization: Bearer ****`。

---

## 14. 可观测性与日志

- **请求 ID**：每个请求会生成唯一 ID，可在日志与 TUI 中关联；
- **结构化日志**：`--log-format json` 时输出行式 JSON；
- **环境变量过滤**：`RUST_LOG` 控制等级（默认 `info`）；
- **示例**：
  ```bash
  RUST_LOG=mock_api=debug \
    ./target/release/mock-api run --config examples/basic.json --log-format json
  ```

日志中典型事件类型：

- `ServerStarted { address }`
- `ConfigReloaded { rule_count }`
- `ConfigReloadFailed { message }`
- `RequestStarted { request_id, summary }`
- `RequestCompleted { request_id, result }`
- `RequestFailed { request_id, message }`

---

## 15. 可复用的 WASM 引擎

`mock-wasm` 是 helpapi 的“可嵌入版本”。它把 `mock-core` + `mock-config` 暴露为**只依赖字符串 JSON 的 API**，不接触网络、文件、终端。因此：

- 可以在浏览器里直接跑规则校验和请求决策；
- 可以在 Node.js 中复用规则构造测试替身；
- 可以被任何能加载 WASM 的宿主（甚至非 JavaScript 语言）调用。

### 15.1 WASM 的边界

WASM 中**包含**：

- JSON 配置解析与校验；
- 规则编译（`Engine::compile`）；
- 请求决策（`Engine::decide`）；
- 响应转换（`Engine::transform_upstream_response`）；
- 错误结构化输出。

WASM 中**不包含**：

- 文件系统访问；
- TCP / 端口监听；
- 真实 HTTP 客户端（请求由宿主发起后再回到 WASM 做响应转换）；
- 终端 UI；
- OS 信号、定时器。

### 15.2 怎么把 WASM 从项目中分离

WASM crate 是一个**自包含**的子 crate（`crates/mock-wasm/`），依赖关系：

```
mock-wasm
  ├── mock-core
  └── mock-config
```

要从主项目剥离（独立成新仓库 / 包）：

1. 把 `crates/mock-wasm/` 整个目录拷出；
2. 把 `crates/mock-core/` 与 `crates/mock-config/` 一并拷出；
3. 修改 `Cargo.toml` 中的依赖路径为相对路径或新仓库内路径：
   ```toml
   [dependencies]
   mock-core = { path = "../mock-core" }
   mock-config = { path = "../mock-config" }
   wasm-bindgen = "0.2"
   ```
4. 添加 `package.json` 与 npm 元数据（参考原 `crates/mock-wasm/package.json`）；
5. 使用 `wasm-pack build` 输出 `pkg/` 目录，作为 npm 包发布。

> 注意：WASM 端不依赖任何网络、文件系统，所以可以独立成单独的 npm 包。

### 15.3 编译为 WASM

```bash
# 安装 wasm-pack（一次性）
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# 三种 target 任选
wasm-pack build crates/mock-wasm --target bundler  # 默认，推荐
wasm-pack build crates/mock-wasm --target nodejs
wasm-pack build crates/mock-wasm --target web
```

输出目录：

```
crates/mock-wasm/pkg/
├── helpapi_mock_wasm_bg.wasm
├── helpapi_mock_wasm.js
├── helpapi_mock_wasm.d.ts
└── ...
```

### 15.4 WASM 接口（来自 `crates/mock-wasm/src/lib.rs` 与 `engine.rs`）

所有 API 都通过 `wasm-bindgen` 暴露，输入输出均为 **JSON 字符串**。

#### 15.4.1 `validate_config(configJson: string): string`

校验配置文件。

- 成功：返回 `{"ok": true}`；
- 失败：返回 `{"ok": false, "error": "<消息>"}`。

#### 15.4.2 `WasmMockEngine`

##### `new WasmMockEngine(configJson: string)`

构造引擎：内部完成解析 + 校验 + 编译。失败抛出 `Error`。

##### `engine.decide(requestJson: string): string`

输入一个 `RequestData` 的 JSON，输出一个 `Decision` 的 JSON。

##### `engine.transform_response(contextJson: string, responseJson: string): string`

输入 `MatchedRequestContext` JSON + 上游 `ResponseData` JSON，输出转换后的 `ResponseData` JSON。

### 15.5 数据结构 JSON 形状

#### `RequestData`

```json
{
  "method": "GET",
  "path": "/users/42",
  "query": [["lang", "zh"]],
  "headers": [["authorization", "Bearer xxx"]],
  "body": "empty",
  "path_params": { "id": "42" }
}
```

`body` 取值：

- `"empty"` —— 空 Body；
- `"<纯文本>"` —— 字符串形式，等价于 `text_body`；
- `{"text": "..."}` —— 文本；
- `{"json": { ... }}` —— JSON 值；
- `{"binary": "base64..."}` —— 二进制；
- 也可以直接传原生字符串。

#### `ResponseData`

```json
{
  "status": 200,
  "headers": [["content-type", "application/json"]],
  "body": { "json": { "ok": true } },
  "delay_ms": null,
  "path_params": {}
}
```

#### `Decision`

`Decision` 是带 tag 的枚举，对应三种变体（见 `mock-core/src/decision.rs`）：

```json
{
  "Mock": {
    "response": { ... },
    "metadata": { "route_id": "get-user", "elapsed_us": 0 }
  }
}
```

```json
{
  "Forward": {
    "plan": {
      "url": "https://api.example.com/users/42",
      "method": "GET",
      "headers": [...],
      "body": "empty",
      "timeout_ms": 10000,
      "context": { "route_id": "proxy", "path_params": {}, "matched_rules": [] }
    },
    "metadata": { "route_id": "proxy", "elapsed_us": 0 }
  }
}
```

```json
{
  "Reject": {
    "response": { "status": 404, ... },
    "metadata": { "route_id": "..." }
  }
}
```

#### `MatchedRequestContext`

```json
{
  "route_id": "get-user",
  "path_params": { "id": "42" },
  "matched_rules": ["method: GET", "path: /users/:id"]
}
```

### 15.6 错误对象（来自 `mock-wasm/src/error.rs`）

WASM 错误以 `JsValue` 字符串形式抛出，前缀标明类型：

- `Config error: ...`
- `Compile error: ...`
- `Decision error: ...`
- `Transform error: ...`
- `Serialization error: ...`

宿主可以用 `try/catch` 捕获后展示。

### 15.7 在 Node.js 中使用（完整示例）

```javascript
import init, { WasmMockEngine, validate_config } from 'helpapi-mock-wasm';

await init(); // 初始化 WASM

const configJson = JSON.stringify({
  version: 1,
  server: { host: '127.0.0.1', port: 8080 },
  defaults: { upstream_timeout_ms: 10000, max_body_bytes: 1048576 },
  routes: [
    {
      id: 'get-user',
      priority: 100,
      match_rule: { method: 'GET', path: '/users/:id' },
      action: {
        type: 'mock',
        response: { status: 200, json_body: { id: 1, name: 'Alice' } },
      },
    },
  ],
});

// 1) 校验配置
const validation = validate_config(configJson);
console.log('validation:', validation);

// 2) 创建引擎
const engine = new WasmMockEngine(configJson);

// 3) 决策
const requestJson = JSON.stringify({
  method: 'GET',
  path: '/users/42',
  query: [],
  headers: [],
  body: 'empty',
});

const decisionJson = engine.decide(requestJson);
console.log('decision:', JSON.parse(decisionJson));

// 4) 响应转换（仅在 forward 场景）
const contextJson = JSON.stringify({
  route_id: 'proxy',
  path_params: {},
  matched_rules: [],
});

const responseJson = JSON.stringify({
  status: 200,
  headers: [],
  body: { json: { hello: 'world' } },
  delay_ms: null,
  path_params: {},
});

const transformed = engine.transform_response(contextJson, responseJson);
console.log('transformed:', JSON.parse(transformed));
```

### 15.8 在浏览器中使用

```html
<script type="module">
  import init, { WasmMockEngine, validate_config } from './pkg/helpapi_mock_wasm.js';
  await init();

  const config = `{
    "version": 1,
    "server": { "host": "127.0.0.1", "port": 8080 },
    "defaults": { "upstream_timeout_ms": 10000, "max_body_bytes": 1048576 },
    "routes": []
  }`;

  const ok = JSON.parse(validate_config(config));
  console.log('config ok:', ok);

  const engine = new WasmMockEngine(config);
  const decision = engine.decide(JSON.stringify({
    method: 'GET', path: '/', query: [], headers: [], body: 'empty'
  }));
  console.log('decision:', JSON.parse(decision));
</script>
```

### 15.9 在其他程序中使用

任何能加载 WebAssembly 的环境都可以复用这套引擎，例如：

- **Node.js**：见 15.7；
- **浏览器**：见 15.8；
- **Deno**：直接 import wasm-pack 生成的 `helpapi_mock_wasm.js` 即可；
- **Go**：`wasmtime-go` 加载 `.wasm`，调用导出的 `validate_config` / `WasmMockEngine.decide` 等；
- **Python**：`wasmer-python` 或 `wasmtime-py`；
- **C / C++**：`wasmtime-cpp` 或 `wasi-sdk` 编译时静态嵌入；
- **Java**：`wasmtime-java` 或 `Chicory`。

调用模式统一：

1. 实例化模块；
2. 调用 `validate_config(ptr, len)`；
3. 创建 `WasmMockEngine`（在 Rust 端即 `WasmMockEngine::new(config_json)`）；
4. 反复调用 `decide` / `transform_response`，传入 JSON 字符串。

> 由于输入输出都是 JSON 字符串，跨语言集成几乎没有摩擦。

---

## 16. 常见问题与排错

### 16.1 启动报 “port already in use”

改 `server.port`，或关闭占用端口的进程：

```bash
lsof -i :8081
```

### 16.2 配置文件错误：版本不对

`INVALID_VERSION` 错误，请把 `version` 设为 `1`。

### 16.3 JSON Pointer 报错

`INVALID_JSON_POINTER`：检查是否以 `/` 开头、是否含非法字符。

### 16.4 上游 URL 不合法

`INVALID_UPSTREAM_URL`：必须以 `http://` 或 `https://` 开头。

### 16.5 启用了 TLS 但报 “missing keystore password”

`MISSING_KEYSTORE_PASSWORD`：在 `server` 中同时设置 `keystore_file` 与 `keystore_password`。

### 16.6 转发未生效

- 确认 `match_rule` 至少有一个条件；
- 调整 `priority` 让该规则优先匹配；
- 打开 debug 日志，确认 `RequestCompleted` 中的 `route_id` 是否为你期望的 ID。

### 16.7 WASM 端 “Compile error”

通常是 JSON 不合法或校验失败；先调用 `validate_config` 验证。

### 16.8 TUI 显示异常

- 确认终端支持 ANSI 与 UTF-8；
- 终端窗口至少 80×24；
- Linux/macOS 下大多数终端可用；Windows 推荐使用 Windows Terminal。

---

## 17. 示例合集

仓库内置的示例：

| 文件 | 说明 |
| --- | --- |
| `examples/basic.json` | 基础 Mock + 上游转发示例 |
| `examples/proxy.json` | 纯转发代理示例（jsonplaceholder / httpbin） |
| `examples/transforms.json` | 各种 Header / JSON / 延迟 / Body 转换示例 |
| `examples/https.json` | HTTPS + JKS 证书库示例（监听 8443） |

启动示例：

```bash
# 基础
./target/release/mock-api run --config examples/basic.json

# 纯代理
./target/release/mock-api run --config examples/proxy.json

# 转换演示
./target/release/mock-api run --config examples/transforms.json

# HTTPS
./target/release/mock-api run --config examples/https.json
```

---

## 附录 A：CLI 速查

```bash
mock-api run --config <PATH> [--log-format pretty|json] [--mode standard|tui]
mock-api validate --config <PATH>
mock-api schema [--output <PATH>]
mock-api print-effective-config --config <PATH>
mock-api version
```

## 附录 B：JSON 配置速查

```json
{
  "version": 1,
  "server": {
    "host": "127.0.0.1",
    "port": 8080,
    "keystore_file": null,
    "keystore_password": null,
    "key_password": null
  },
  "defaults": {
    "upstream_timeout_ms": 10000,
    "max_body_bytes": 1048576
  },
  "routes": [
    {
      "id": "example",
      "priority": 100,
      "match_rule": {
        "method": "GET",
        "path": "/users/:id",
        "headers": { "X-Env": "dev" },
        "query": { "lang": "zh" }
      },
      "action": {
        "type": "mock",
        "response": {
          "status": 200,
          "headers": { "Content-Type": "application/json" },
          "json_body": { "id": 1, "name": "Alice" },
          "delay_ms": 0
        }
      }
    }
  ],
  "unmatched": {
    "action": {
      "type": "mock",
      "response": { "status": 404, "json_body": { "error": "Not Found" } }
    }
  }
}
```

## 附录 C：WASM 速查

```javascript
import init, { WasmMockEngine, validate_config } from 'helpapi-mock-wasm';

await init();

const ok = JSON.parse(validate_config(configJson));
if (!ok.ok) throw new Error(ok.error);

const engine = new WasmMockEngine(configJson);

const decisionJson = engine.decide(requestJson);
const decision = JSON.parse(decisionJson);

if (decision.Mock) {
  /* 直接使用 decision.Mock.response */
} else if (decision.Forward) {
  /* 宿主发起 upstream 请求，再用 transform_response */
  const transformed = engine.transform_response(
    JSON.stringify(decision.Forward.plan.context),
    JSON.stringify(upstreamResponse),
  );
} else if (decision.Reject) {
  /* 返回 decision.Reject.response */
}
```

---

> 文档结束。如需更新本文档，请保持与 `crates/mock-config/src/models.rs` / `mock-core/src/transform/spec.rs` / `mock-wasm/src/lib.rs` 的最新定义同步。