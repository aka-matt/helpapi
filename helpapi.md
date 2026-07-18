# Rust Mock API CLI 工具实施文档

## 0. 名称

这个程序叫 `helpapi`

## 1. 文档目标

本文档描述一个使用 Rust 开发的 Mock API CLI 工具的实施方案。工具需要具备以下能力：

- 从 JSON 配置文件加载 Mock 规则；
- 启动本地 HTTP Mock 服务；
- 将部分请求转发到真实上游 API；
- 在转发前修改请求 Headers、Query、Path 或 Payload；
- 在返回客户端前修改响应 Headers、状态码或 Payload；
- 使用 Ratatui 提供终端 UI；
- 将规则匹配、配置校验和请求转换能力编译为 WASM，供浏览器、Node.js 或其他宿主程序复用；
- 保持 CLI、HTTP 网络层和 WASM 核心之间的职责边界清晰。

本文档以可逐步交付的方式组织，优先实现最小可用版本，再增加配置热重载、脚本化转换、录制回放等高级能力。

---

## 2. 总体架构原则

### 2.1 核心原则

项目采用“共享核心 + 宿主适配器”的架构：

- `mock-core` 负责纯业务逻辑；
- `mock-config` 负责配置模型、解析和校验；
- `mock-http` 负责原生 HTTP Server 和上游转发；
- `mock-runtime` 负责服务生命周期、状态和事件调度；
- `mock-cli` 负责命令行参数、文件读取和 Ratatui；
- `mock-wasm` 负责把核心能力暴露为稳定的 WASM API。

原生 CLI 不通过 WASM 调用核心逻辑，而是直接依赖 Rust library。WASM 只是核心能力面向其他宿主的发布形式。

### 2.2 WASM 边界

WASM 中包含：

- JSON 配置解析；
- 配置校验；
- 请求规则匹配；
- Mock 响应生成；
- Header、Query 和 Payload 转换；
- 转发决策生成；
- 响应转换规则执行。

WASM 中不包含：

- Ratatui；
- 终端事件处理；
- 本地文件读取；
- 文件监听；
- TCP 端口监听；
- TLS；
- 真实 HTTP 请求发送；
- 操作系统信号处理。

### 2.3 数据流

```text
JSON 配置文件
    ↓
mock-cli 读取文件
    ↓
mock-config 解析与校验
    ↓
mock-runtime 创建规则引擎
    ↓
mock-http 接收请求
    ↓
mock-core 匹配规则并产生 Decision
    ├── MockResponse：直接返回本地响应
    ├── ForwardPlan：修改后转发到上游
    └── Reject：拒绝或返回配置错误
    ↓
mock-http 执行网络操作
    ↓
mock-core 应用响应转换
    ↓
返回客户端并发送事件到 Ratatui
```

---

## 3. Workspace 目录设计 （建议）

建议采用 Cargo Workspace：

```text
mock-api/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── rustfmt.toml
├── deny.toml
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── crates/
│   ├── mock-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── mock-config/
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── mock-http/
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── mock-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── mock-cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   └── mock-wasm/
│       ├── Cargo.toml
│       └── src/
├── examples/
│   ├── basic.json
│   ├── proxy.json
│   └── transforms.json
├── schemas/
│   └── mock-api.schema.json
├── tests/
│   ├── integration/
│   └── fixtures/
└── scripts/
    ├── build-wasm.sh
    └── release.sh
```

根目录 `Cargo.toml`：

```toml
[workspace]
resolver = "2"
members = [
    "crates/mock-core",
    "crates/mock-config",
    "crates/mock-http",
    "crates/mock-runtime",
    "crates/mock-cli",
    "crates/mock-wasm",
]

[workspace.package]
edition = "2024"
license = "MIT"
rust-version = "1.85"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
url = "2"
uuid = { version = "1", features = ["v4", "serde"] }
```

版本号仅作为实施示例。落地时应统一检查工具链与依赖的兼容版本。

---

## 4. Crate 职责和依赖关系

```text
mock-cli
  └── mock-runtime
        ├── mock-http
        │     ├── mock-core
        │     └── mock-config
        ├── mock-core
        └── mock-config

mock-wasm
  ├── mock-core
  └── mock-config
```

依赖必须尽量保持单向，避免 `mock-core` 反向依赖网络层、CLI 或 WASM 绑定。

### 4.1 `mock-core`

职责：

- 定义宿主无关的请求和响应模型；
- 编译配置规则；
- 匹配请求；
- 生成 Mock 响应；
- 生成转发计划；
- 执行请求和响应转换；
- 提供可预测、可测试的纯逻辑 API。

建议模块：

```text
mock-core/src/
├── lib.rs
├── engine.rs
├── decision.rs
├── request.rs
├── response.rs
├── matcher/
│   ├── mod.rs
│   ├── path.rs
│   ├── header.rs
│   ├── query.rs
│   └── body.rs
├── transform/
│   ├── mod.rs
│   ├── headers.rs
│   ├── query.rs
│   ├── json.rs
│   └── template.rs
└── error.rs
```

核心接口示例：

```rust
pub struct Engine {
    rules: Vec<CompiledRule>,
}

impl Engine {
    pub fn compile(config: Config) -> Result<Self, CompileError>;

    pub fn decide(
        &self,
        request: RequestData,
    ) -> Result<Decision, EngineError>;

    pub fn transform_upstream_response(
        &self,
        context: &MatchedRequestContext,
        response: ResponseData,
    ) -> Result<ResponseData, EngineError>;
}
```

`Decision` 建议设计为：

```rust
pub enum Decision {
    Mock {
        response: ResponseData,
        metadata: DecisionMetadata,
    },
    Forward {
        plan: ForwardPlan,
        metadata: DecisionMetadata,
    },
    Reject {
        response: ResponseData,
        metadata: DecisionMetadata,
    },
}
```

### 4.2 `mock-config`

职责：

- 定义 JSON 配置结构；
- 从字符串或字节解析配置；
- 进行语义校验；
- 输出结构化错误；
- 生成 JSON Schema；
- 处理配置版本升级。

建议接口：

```rust
pub fn parse_json(input: &str) -> Result<Config, ParseError>;

pub fn validate(config: &Config) -> ValidationReport;

pub fn parse_and_validate(input: &str) -> Result<Config, ConfigError>;
```

不要在该 crate 中提供 `load_from_path` 作为核心 API。文件系统读取应由 CLI 或其他宿主完成。

配置错误应包含：

- 错误代码；
- JSON 路径；
- 人类可读消息；
- 可选建议；
- 严重级别。

```rust
pub struct ValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub severity: Severity,
}
```

### 4.3 `mock-http`

职责：

- 启动 HTTP Server；
- 将框架请求转换成 `RequestData`；
- 调用核心引擎；
- 执行真实上游请求；
- 把核心响应转换成 HTTP 响应；
- 处理超时、连接错误、TLS 和 body 大小限制；
- 发送请求生命周期事件。

候选技术栈：

- Tokio：异步运行时；
- Axum：Server 路由与服务入口；
- Hyper：底层 HTTP 能力；
- Reqwest：上游客户端；
- Tower：中间件、超时和并发限制。

建议先使用 Axum + Reqwest，以减少首版开发成本。后续只有在需要精细控制连接池、代理或流式传输时，再直接使用 Hyper。

接口示例：

```rust
pub struct HttpServer {
    shutdown: ShutdownHandle,
}

pub async fn start_server(
    config: ServerConfig,
    engine: SharedEngine,
    events: EventSender,
) -> Result<HttpServer, HttpError>;
```

请求处理流程：

1. 读取 Method、URI、Headers 和 Body；
2. 检查 body 大小限制；
3. 转换为 `RequestData`；
4. 调用 `Engine::decide`；
5. 对 `Mock` 直接构建响应；
6. 对 `Forward` 使用 HTTP Client 执行请求；
7. 调用响应转换；
8. 记录耗时和结果；
9. 返回客户端。

### 4.4 `mock-runtime`

职责：

- 聚合配置、引擎和 HTTP 服务；
- 管理启动、停止和重载；
- 管理运行时状态；
- 向 CLI 发布事件；
- 保留最近请求记录；
- 为非 CLI 宿主提供统一原生 API。

建议状态：

```rust
pub enum RuntimeStatus {
    Stopped,
    Starting,
    Running,
    Reloading,
    Failed,
}
```

主要接口：

```rust
pub struct Runtime {
    // 内部字段省略
}

impl Runtime {
    pub async fn new(config: Config) -> Result<Self, RuntimeError>;
    pub async fn start(&mut self) -> Result<(), RuntimeError>;
    pub async fn reload(&mut self, config: Config) -> Result<(), RuntimeError>;
    pub async fn stop(&mut self) -> Result<(), RuntimeError>;
    pub fn subscribe(&self) -> EventReceiver;
}
```

配置热重载时建议使用“先编译、后替换”：

1. 读取新配置；
2. 解析和校验；
3. 创建新 Engine；
4. 全部成功后使用 `ArcSwap` 或 `RwLock` 原子替换；
5. 失败时继续使用旧配置。

### 4.5 `mock-cli`

职责：

- 提供命令行参数；
- 读取配置文件；
- 初始化日志；
- 启动 Runtime；
- 管理 Ratatui 终端生命周期；
- 处理按键；
- 展示请求、响应、错误和状态。

建议 CLI：

```text
mock-api run --config mock.json
mock-api validate --config mock.json
mock-api schema --output mock-api.schema.json
mock-api print-effective-config --config mock.json
mock-api version
```

首版 Ratatui 页面建议：

```text
┌─ Status ───────────────────────────────────────┐
│ Running  127.0.0.1:8080  Rules: 12            │
└────────────────────────────────────────────────┘
┌─ Requests ─────────────────┬─ Details ─────────┐
│ 200 GET  /users     12ms   │ Rule: users-list  │
│ 201 POST /users     34ms   │ Mode: Forward     │
│ 500 GET  /broken     2ms   │ Upstream: ...     │
└────────────────────────────┴───────────────────┘
┌─ Help ─────────────────────────────────────────┐
│ q Quit  r Reload  / Filter  Enter Details      │
└────────────────────────────────────────────────┘
```

建议先实现以下按键：

- `q`：安全退出；
- `r`：重新加载配置；
- `j/k` 或方向键：选择请求；
- `/`：过滤；
- `Tab`：切换面板；
- `Enter`：查看详情；
- `c`：清空请求历史；
- `?`：帮助。

### 4.6 `mock-wasm`

职责：

- 使用 `wasm-bindgen` 暴露稳定接口；
- 将 JS 字符串或对象转换为核心模型；
- 把错误转换成结构化 JSON；
- 不直接发送网络请求。

建议 API：

```rust
#[wasm_bindgen]
pub struct WasmMockEngine {
    inner: mock_core::Engine,
}

#[wasm_bindgen]
impl WasmMockEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmMockEngine, JsValue>;

    pub fn decide(&self, request_json: &str) -> Result<String, JsValue>;

    pub fn transform_response(
        &self,
        context_json: &str,
        response_json: &str,
    ) -> Result<String, JsValue>;
}

#[wasm_bindgen]
pub fn validate_config(config_json: &str) -> String;
```

首版只生成一个 WASM 包：

```text
mock-engine.wasm
```

只有当配置编辑器需要独立、轻量地加载校验能力时，再拆出：

```text
mock-config.wasm
```

---

## 5. 配置文件设计

### 5.1 顶层结构

```json
{
  "version": 1,
  "server": {
    "host": "127.0.0.1",
    "port": 8080
  },
  "defaults": {
    "upstream_timeout_ms": 10000,
    "max_body_bytes": 1048576
  },
  "routes": []
}
```

### 5.2 Mock 路由示例

```json
{
  "id": "get-user",
  "priority": 100,
  "match": {
    "method": "GET",
    "path": "/users/:id",
    "headers": {
      "x-environment": "development"
    }
  },
  "action": {
    "type": "mock",
    "response": {
      "status": 200,
      "headers": {
        "content-type": "application/json"
      },
      "json": {
        "id": "{{path.id}}",
        "name": "Mock User"
      }
    }
  }
}
```

### 5.3 转发路由示例

```json
{
  "id": "forward-create-user",
  "priority": 50,
  "match": {
    "method": "POST",
    "path": "/users"
  },
  "action": {
    "type": "forward",
    "upstream": "https://api.example.com",
    "request_transforms": [
      {
        "type": "set_header",
        "name": "x-mock-proxy",
        "value": "true"
      },
      {
        "type": "set_json_pointer",
        "pointer": "/source",
        "value": "local-cli"
      },
      {
        "type": "remove_header",
        "name": "cookie"
      }
    ],
    "response_transforms": [
      {
        "type": "set_header",
        "name": "x-proxied-by",
        "value": "mock-api"
      },
      {
        "type": "set_json_pointer",
        "pointer": "/debug/proxied",
        "value": true
      }
    ]
  }
}
```

### 5.4 首版支持的匹配器

建议首版限定为：

- HTTP Method；
- 精确 Path；
- Path 参数；
- Query 参数；
- Header 精确值；
- Header 是否存在；
- JSON Pointer 对应值；
- 规则优先级。

后续再增加：

- 正则 Path；
- Header 正则；
- JSON Schema 匹配；
- Body 部分包含；
- 概率匹配；
- 请求次数条件；
- 时间窗口条件。

### 5.5 首版支持的转换器

请求和响应均可支持：

- `set_header`；
- `remove_header`；
- `set_query`；
- `remove_query`；
- `set_json_pointer`；
- `remove_json_pointer`；
- `replace_body`；
- `set_status`，仅响应；
- 模板字符串替换。

暂不建议首版支持任意 JavaScript、Lua 或 Rhai 脚本。脚本引擎会显著增加安全模型、调试和跨 WASM 兼容复杂度。

---

## 6. 核心数据模型

### 6.1 请求模型

```rust
pub struct RequestData {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: BodyData,
}

pub enum BodyData {
    Empty,
    Text(String),
    Json(serde_json::Value),
    Binary(Vec<u8>),
}
```

不要在核心模型中直接使用 Axum、Hyper 或 Reqwest 类型。

### 6.2 响应模型

```rust
pub struct ResponseData {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: BodyData,
    pub delay_ms: Option<u64>,
}
```

### 6.3 转发计划

```rust
pub struct ForwardPlan {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: BodyData,
    pub timeout_ms: Option<u64>,
    pub context: MatchedRequestContext,
}
```

### 6.4 事件模型

```rust
pub enum RuntimeEvent {
    ServerStarted { address: String },
    ServerStopped,
    ConfigReloaded { rule_count: usize },
    ConfigReloadFailed { message: String },
    RequestStarted { request_id: String, summary: RequestSummary },
    RequestCompleted { request_id: String, result: RequestResult },
    RequestFailed { request_id: String, message: String },
}
```

Ratatui 通过异步 channel 订阅事件，不直接访问 HTTP handler 内部状态。

---

## 7. 请求匹配和规则优先级

推荐规则选择流程：

1. 按 `priority` 从高到低排序；
2. Priority 相同时保持配置文件中的声明顺序；
3. 逐条执行匹配器；
4. 第一条完整匹配的规则获胜；
5. 没有匹配时执行默认行为。

默认行为建议可配置：

```json
{
  "unmatched": {
    "type": "response",
    "status": 404,
    "json": {
      "error": "no_matching_mock_rule"
    }
  }
}
```

也可支持全局 fallback upstream：

```json
{
  "unmatched": {
    "type": "forward",
    "upstream": "https://api.example.com"
  }
}
```

每次决策应返回可解释信息，便于 CLI 展示：

- 匹配的规则 ID；
- 每个 matcher 的结果；
- 执行的 transforms；
- 最终 action；
- 耗时。

---

## 8. Header 和 Payload 处理要求

### 8.1 Header

HTTP Header 名称比较应忽略大小写，但输出时可以统一为小写。

转发时应默认过滤或重新计算以下 Headers：

- `host`；
- `content-length`；
- `transfer-encoding`；
- `connection`；
- `keep-alive`；
- `proxy-authenticate`；
- `proxy-authorization`；
- `te`；
- `trailer`；
- `upgrade`。

是否转发 `cookie`、`authorization` 和客户端 IP 相关 Header 应由配置控制。

### 8.2 JSON Payload

首版 JSON 修改建议基于 JSON Pointer，而不是自定义路径语法。

例如：

```json
{
  "type": "set_json_pointer",
  "pointer": "/user/profile/name",
  "value": "Mock Name"
}
```

对于非 JSON Body：

- JSON 转换规则应返回明确错误；或
- 配置中支持 `on_type_mismatch: skip`。

建议默认严格报错，避免静默产生错误请求。

### 8.3 Body 限制

首版可以将 Body 完整缓存在内存中，但必须支持：

- 最大请求 Body 大小；
- 最大响应 Body 大小；
- 超限时返回 `413 Payload Too Large`；
- CLI 中显示截断后的预览；
- 日志中默认不记录完整敏感 Body。

流式转发可放在后续版本实现。

---

## 9. Ratatui 实施方案

### 9.1 应用状态

```rust
pub struct App {
    pub runtime_status: RuntimeStatus,
    pub requests: VecDeque<RequestRecord>,
    pub selected_request: Option<usize>,
    pub active_panel: Panel,
    pub filter: String,
    pub config_error: Option<String>,
    pub should_quit: bool,
}
```

### 9.2 事件循环

建议使用一个主循环统一处理：

- 终端事件；
- Runtime 事件；
- Tick；
- 重绘。

```text
Tokio select!
  ├── crossterm event
  ├── runtime event receiver
  ├── periodic tick
  └── shutdown signal
```

### 9.3 页面演进

MVP 页面：

- 服务状态；
- 请求列表；
- 单条请求详情；
- 错误提示；
- 快捷键说明。

第二阶段：

- 请求和响应 Headers 对比；
- 转换前后 JSON Diff；
- 规则匹配解释；
- 配置错误列表；
- 请求过滤；
- 成功率和延迟摘要。

第三阶段：

- 规则启用和禁用；
- 录制真实响应并生成配置；
- 导出请求记录；
- 重放请求。

---

## 10. WASM 发布接口设计

### 10.1 输入输出格式

为降低 JS 绑定复杂度，首版统一接受和返回 JSON 字符串。

```javascript
const engine = new WasmMockEngine(configJson);
const decisionJson = engine.decide(requestJson);
const decision = JSON.parse(decisionJson);
```

后续可以使用 `serde-wasm-bindgen` 暴露原生 JS 对象，但应先稳定 Rust 核心模型和错误协议。

### 10.2 错误格式

```json
{
  "ok": false,
  "error": {
    "code": "CONFIG_INVALID",
    "message": "route action is missing",
    "path": "/routes/2/action"
  }
}
```

### 10.3 WASM 宿主职责

宿主负责：

1. 加载配置字符串；
2. 创建 WASM Engine；
3. 把请求转换为统一 JSON；
4. 调用 `decide`；
5. 对 `mock` 直接使用返回结果；
6. 对 `forward` 使用宿主自己的 HTTP Client；
7. 将上游响应再次传入 WASM 做响应转换。

---

## 11. 分阶段实施计划

## 阶段 0：仓库和工程基础

交付内容：

- 创建 Cargo Workspace；
- 建立六个 crate；
- 配置 rustfmt、Clippy 和基础 CI；
- 统一错误处理和日志规范；
- 添加基础 README 和示例配置。

验收标准：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

全部通过。

## 阶段 1：配置模型与校验

交付内容：

- 配置顶层模型；
- Mock 和 Forward action；
- Method、Path、Header、Query matcher；
- Header 和 JSON Pointer transforms；
- 结构化配置错误；
- JSON Schema；
- `mock-api validate` 命令。

验收标准：

- 有效配置成功解析；
- 无效配置返回准确 JSON 路径；
- 配置 Schema 可被编辑器使用；
- 配置版本字段被强制校验。

## 阶段 2：核心规则引擎

交付内容：

- Config 编译为 Engine；
- 规则优先级；
- 请求匹配；
- Mock 决策；
- ForwardPlan 生成；
- 请求和响应转换；
- 模板替换；
- 决策解释信息。

验收标准：

- 核心 crate 不依赖 Tokio、Axum、Reqwest、Ratatui 或 wasm-bindgen；
- 所有 matcher 和 transform 有单元测试；
- 同一输入产生确定性输出；
- Native 和 WASM 使用同一测试向量。

## 阶段 3：HTTP Server 与转发

交付内容：

- 本地 HTTP Server；
- Mock 响应；
- 上游转发；
- 请求 Header 和 Payload 转换；
- 响应 Header、状态码和 Payload 转换；
- 超时、大小限制和错误映射；
- 优雅关闭。

验收标准：

- Mock 路由可通过 `curl` 验证；
- Forward 路由正确修改请求；
- 上游错误产生稳定的错误响应；
- `Ctrl+C` 能安全关闭服务。

## 阶段 4：Runtime 与 CLI 命令

交付内容：

- Runtime 生命周期；
- `run`、`validate` 和 `schema` 命令；
- 文件读取；
- 文件变化监听；
- 安全热重载；
- 请求事件发布。

验收标准：

- 配置修改后自动生效；
- 新配置无效时继续使用旧配置；
- CLI 能显示重载失败原因；
- 服务关闭时所有任务正常退出。

## 阶段 5：Ratatui MVP

交付内容：

- 状态栏；
- 请求列表；
- 请求详情；
- 键盘导航；
- 过滤和清空；
- 错误弹窗；
- 正确恢复终端状态。

验收标准：

- 正常退出、异常退出均恢复终端；
- 高频请求下 UI 不阻塞 HTTP Server；
- 请求记录有容量上限；
- 敏感 Header 默认遮罩。

## 阶段 6：WASM 包

交付内容：

- `mock-wasm`；
- 配置校验 API；
- 请求决策 API；
- 响应转换 API；
- npm package；
- Node.js 和浏览器示例；
- Native/WASM 共享测试向量。

验收标准：

- Node.js 可加载并执行规则；
- 浏览器可校验配置；
- WASM 不要求文件系统或网络权限；
- 与原生 Engine 对相同输入输出一致。

## 阶段 7：高级能力

候选能力：

- 请求录制和响应快照；
- 请求重放；
- 延迟、断连和故障注入；
- JSON Schema body matcher；
- 动态变量和状态场景；
- WebSocket；
- 流式转发；
- 插件或受限脚本系统；
- Web 配置编辑器。

这些能力不应阻塞 MVP 发布。

---

## 12. 测试策略

### 12.1 单元测试

重点覆盖：

- 每种 matcher；
- 每种 transform；
- 规则优先级；
- JSON Pointer 边界；
- Header 大小写；
- 模板变量缺失；
- 非 JSON Body；
- 配置校验。

### 12.2 Snapshot 测试

适合测试：

- 配置错误报告；
- Decision JSON；
- 请求转换结果；
- 响应转换结果；
- Ratatui 渲染缓冲区。

### 12.3 集成测试

启动临时 Mock Server 和测试上游：

```text
Test Client
   ↓
Mock API Server
   ↓
Test Upstream Server
```

验证：

- 请求确实到达上游；
- Header 和 Payload 已修改；
- 响应转换生效；
- 超时和断开场景；
- 配置热重载。

### 12.4 Property Testing

可使用 `proptest` 测试：

- 任意 Header 输入不会 panic；
- 任意 JSON Pointer 不会导致越界；
- 解析和序列化保持关键字段；
- 规则引擎在任意配置下不会发生未捕获 panic。

### 12.5 Fuzzing

建议对以下入口进行 fuzz：

- JSON 配置解析；
- Header 转换；
- JSON Pointer transform；
- Path matcher；
- WASM JSON 接口。

---

## 13. 安全与隐私要求

### 13.1 敏感信息

默认遮罩：

- `authorization`；
- `proxy-authorization`；
- `cookie`；
- `set-cookie`；
- 可配置自定义敏感 Header。

CLI 和日志中只展示：

```text
Authorization: Bearer ****
```

### 13.2 监听地址

默认仅监听：

```text
127.0.0.1
```

监听 `0.0.0.0` 时输出明显警告，并建议使用显式参数允许。

### 13.3 上游限制

可选安全配置：

- 允许的上游域名列表；
- 禁止访问 link-local 或私有网段；
- 禁止自动跟随跨域重定向；
- 限制最大响应大小；
- 限制并发连接数。

对于本地开发工具，可以提供宽松默认值，但应允许团队环境启用严格模式。

### 13.4 配置脚本

在没有明确沙箱模型前，不加载任意本地动态库，不执行 Shell 命令，也不运行不受限脚本。

---

## 14. 可观测性

建议使用 `tracing`：

- 每个请求分配 request ID；
- 记录匹配规则；
- 记录 action 类型；
- 记录上游耗时；
- 记录转换耗时；
- 记录最终状态码；
- 避免默认记录完整 Body。

日志模式：

```bash
mock-api run --config mock.json --log-format pretty
mock-api run --config mock.json --log-format json
```

可选环境变量：

```bash
RUST_LOG=mock_api=debug,tower_http=info
```

---

## 15. 性能和容量约束

MVP 建议明确以下限制：

- 最大请求 Body：1 MiB；
- 最大响应 Body：5 MiB；
- 默认上游超时：10 秒；
- 请求历史：最近 1,000 条；
- UI Body 预览：最多 32 KiB；
- 默认最大并发：100；
- 配置规则建议上限：10,000 条。

具体值应允许通过配置覆盖。

性能优化顺序：

1. 避免重复解析配置；
2. 将 Path 和 Header matcher 预编译；
3. 使用 `Arc` 共享 Engine；
4. 限制 Body 复制；
5. 只在需要时解析 JSON；
6. 最后再考虑自定义索引或零拷贝。

首版不要为了假设的性能问题引入复杂生命周期和不安全代码。

---

## 16. CI 和质量门槛

CI 建议执行：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
cargo deny check
```

WASM 检查：

```bash
cargo test -p mock-core
wasm-pack test --node crates/mock-wasm
wasm-pack build crates/mock-wasm --target bundler
```

建议平台矩阵：

- Linux；
- macOS；
- Windows；
- wasm32-unknown-unknown。

---

## 17. 发布方案

### 17.1 CLI

发布产物：

- Linux x86_64；
- Linux aarch64；
- macOS x86_64；
- macOS aarch64；
- Windows x86_64。

可选发布渠道：

- GitHub Releases；
- crates.io；
- Homebrew Tap；
- Scoop；
- Cargo Binstall。

### 17.2 WASM

发布为 npm package：

```text
@your-scope/mock-api-engine
```

包含：

- `.wasm`；
- JS glue；
- TypeScript 类型；
- 浏览器示例；
- Node.js 示例；
- API 兼容性说明。

### 17.3 版本策略

建议所有 Workspace crate 首期统一版本，降低兼容管理成本。

配置文件需要单独的版本字段：

```json
{
  "version": 1
}
```

CLI 版本和配置格式版本不应视为同一个概念。

---

## 18. MVP 范围

第一版必须完成：

- JSON 配置读取和校验；
- Method、Path、Header 和 Query 匹配；
- 静态 Mock 响应；
- 上游 API 转发；
- 请求 Header 修改；
- 请求 JSON Payload 修改；
- 响应 Header 和 JSON Payload 修改；
- Ratatui 请求列表和详情；
- 配置手动重载；
- 一个可复用的 WASM 引擎；
- 基础 CI 和跨平台构建。

第一版不做：

- 任意脚本执行；
- WebSocket；
- 流式大文件代理；
- 完整 API 录制器；
- 分布式协作；
- 插件市场；
- 图形化 Web 管理后台；
- 多个独立 WASM 包。

---

## 19. 推荐开发顺序

```text
配置模型
  ↓
核心请求与响应模型
  ↓
规则匹配
  ↓
转换器
  ↓
Mock 决策
  ↓
ForwardPlan
  ↓
HTTP Server
  ↓
上游转发
  ↓
Runtime
  ↓
CLI 命令
  ↓
Ratatui
  ↓
WASM 包装
  ↓
发布和高级能力
```

不要先开发 Ratatui。先通过普通 CLI 和集成测试证明配置、规则引擎与转发链路正确，再添加 TUI。

---

## 20. 初始里程碑建议

### Milestone 1：配置可用

- Workspace 完成；
- JSON 配置可解析；
- `validate` 命令可用；
- JSON Schema 可生成。

### Milestone 2：Mock 可用

- 本地 HTTP Server；
- Path 和 Method 匹配；
- 静态 JSON 响应；
- curl 示例通过。

### Milestone 3：代理可用

- 上游转发；
- Header 修改；
- JSON Payload 修改；
- 响应转换；
- 超时处理。

### Milestone 4：TUI 可用

- 请求列表；
- 请求详情；
- 配置重载；
- 错误展示；
- 安全退出。

### Milestone 5：WASM 可用

- 配置校验；
- 请求决策；
- 响应转换；
- npm 包；
- Node 和浏览器示例。

---

## 21. 架构决策总结

1. 使用一个 GitHub repository 和一个 Cargo Workspace。
2. 项目拆成约六个 crate，而不是六个 WASM。
3. 初期只发布一个 `mock-engine.wasm`。
4. CLI 和 WASM 直接共享 `mock-core` 与 `mock-config`。
5. 文件、终端、端口监听和真实网络请求属于宿主层。
6. 核心层只处理数据、规则和决策。
7. 配置热重载使用“验证成功后原子替换”。
8. 首版限制 Body 大小并采用内存缓冲，后续再支持流式处理。
9. 首版不引入任意脚本执行。
10. 优先保证规则可解释、错误可定位、Native/WASM 行为一致。

---

## 22. 下一步执行清单

- [ ] 创建 Cargo Workspace 和六个 crate；
- [ ] 定义 `Config`、`RequestData`、`ResponseData` 和 `Decision`；
- [ ] 实现 `mock-api validate`；
- [ ] 编写基础配置 Schema；
- [ ] 实现 Method 和 Path matcher；
- [ ] 实现 Mock response；
- [ ] 启动 Axum Server；
- [ ] 实现 Reqwest 上游转发；
- [ ] 实现 Header transforms；
- [ ] 实现 JSON Pointer transforms；
- [ ] 增加集成测试上游；
- [ ] 实现 Runtime 事件 channel；
- [ ] 实现 Ratatui 请求列表；
- [ ] 实现配置重载；
- [ ] 添加 `mock-wasm`；
- [ ] 添加 Node.js 和浏览器示例；
- [ ] 配置 CI、Release 和跨平台构建。
