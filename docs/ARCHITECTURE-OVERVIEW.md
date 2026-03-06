# OpenFang Architecture Overview

A comprehensive architectural guide to OpenFang, the open-source Agent Operating System built in Rust.

**Version**: 0.1.0 | **Crates**: 14 | **Lines of Code**: ~137K | **Tests**: 1,767+

---

## Table of Contents

- [1. Crate Structure](#1-crate-structure)
- [2. Agent System](#2-agent-system)
- [3. Hands System](#3-hands-system)
- [4. Kernel](#4-kernel)
- [5. API & Server Layer](#5-api--server-layer)
- [6. Configuration System](#6-configuration-system)
- [7. Skills System](#7-skills-system)
- [8. Memory Substrate](#8-memory-substrate)
- [9. Channel Adapters](#9-channel-adapters)
- [10. Wire Protocol (OFP)](#10-wire-protocol-ofp)
- [11. Security Model](#11-security-model)
- [12. Workspace & Project Context](#12-workspace--project-context)
- [13. Existing Documentation Map](#13-existing-documentation-map)

---

## 1. Crate Structure

OpenFang is organized as a Cargo workspace with 14 crates (13 library/binary crates + 1 build automation crate). Dependencies flow downward — lower crates have no upward dependencies.

### Dependency Graph

```
openfang-cli              CLI binary, daemon management, TUI dashboard, MCP server mode
    |
openfang-desktop          Tauri 2.0 native desktop app (WebView + system tray)
    |
openfang-api              REST/WS/SSE API server (Axum 0.8), 140+ endpoints, dashboard SPA
    |
openfang-kernel           Central coordinator: assembles all subsystems, workflows, RBAC, metering
    |
    +-- openfang-runtime      Agent loop, 3 LLM drivers, 53 tools, WASM sandbox, MCP, A2A
    +-- openfang-channels     40 channel adapters, bridge, formatter, rate limiter
    +-- openfang-wire         OFP P2P networking with HMAC-SHA256 mutual auth
    +-- openfang-migrate      Migration engine (OpenClaw, LangChain, AutoGPT)
    +-- openfang-skills       60 bundled skills, FangHub marketplace, ClawHub client
    +-- openfang-hands        7 autonomous Hands, HAND.toml parser, lifecycle management
    |
openfang-memory           SQLite persistence, vector embeddings, canonical sessions, compaction
    |
openfang-types            Core types: Agent, Capability, Config, Event, Message, Tool, Taint, etc.

xtask                     Build automation (cargo-xtask pattern)
```

### All 14 Crates — Descriptions & Responsibilities

| # | Crate | Cargo.toml Description | Key Responsibilities |
|---|-------|----------------------|---------------------|
| 1 | **openfang-types** | "Core types and traits for the OpenFang Agent OS" | `AgentManifest`, `AgentId`, `AgentEntry`, `Capability`, `Event`, `ToolDefinition`, `KernelConfig`, `OpenFangError`, taint tracking (`TaintLabel`, `TaintSet`), Ed25519 manifest signing, model catalog types, tool compatibility mappings, MCP/A2A config types, web config types. All config structs use `#[serde(default)]` for forward-compatible TOML parsing. |
| 2 | **openfang-memory** | "Memory substrate for the OpenFang Agent OS" | SQLite-backed memory (schema v5). `Arc<Mutex<Connection>>` with `spawn_blocking` for async bridge. Structured KV store, semantic search with vector embeddings, knowledge graph (entities/relations), session management, task board, usage event persistence (`usage_events` table), canonical sessions for cross-channel memory. Five schema versions: V1 core, V2 collab, V3 embeddings, V4 usage, V5 canonical_sessions. |
| 3 | **openfang-runtime** | "Agent runtime and execution environment for OpenFang" | Agent loop (`run_agent_loop`, `run_agent_loop_streaming`), 3 native LLM drivers (Anthropic, Gemini, OpenAI-compatible covering 20+ providers), 53 built-in tools, WASM sandbox (Wasmtime with dual fuel+epoch metering), MCP client/server (JSON-RPC 2.0 over stdio/SSE), A2A protocol, web search engine (4 providers), web fetch with SSRF protection, loop guard, session repair, LLM session compactor, Merkle hash chain audit trail, embedding driver. Defines `KernelHandle` trait. |
| 4 | **openfang-kernel** | "Core kernel for the OpenFang Agent OS" | Central coordinator `OpenFangKernel`. Assembles: `AgentRegistry`, `AgentScheduler`, `CapabilityManager`, `EventBus`, `Supervisor`, `WorkflowEngine`, `TriggerEngine`, `BackgroundExecutor`, `WasmSandbox`, `ModelCatalog`, `MeteringEngine`, `ModelRouter`, `AuthManager` (RBAC), `HeartbeatMonitor`, `SetupWizard`, `SkillRegistry`, MCP connections, `WebToolsContext`. Implements `KernelHandle` for inter-agent operations. |
| 5 | **openfang-api** | "HTTP/WebSocket API server for the OpenFang Agent OS daemon" | Axum 0.8 HTTP server with 140+ endpoints. REST routes for agents, workflows, triggers, memory, channels, templates, models, providers, skills, ClawHub, MCP, health, status, version, shutdown. WebSocket for real-time agent chat. SSE for streaming. OpenAI-compatible endpoints (`/v1/chat/completions`, `/v1/models`). A2A endpoints. GCRA rate limiter, Bearer token auth, security headers, dashboard SPA (Alpine.js). |
| 6 | **openfang-channels** | "Channel Bridge Layer — pluggable messaging integrations for OpenFang" | 40 channel adapters each implementing `ChannelAdapter` trait. `AgentRouter` for message routing, `BridgeManager` for lifecycle, `ChannelRateLimiter` (per-user DashMap), formatter (Markdown -> TelegramHTML/SlackMrkdwn/PlainText), `ChannelOverrides` (model/system_prompt/dm_policy/group_policy/rate_limit/threading/output_format). |
| 7 | **openfang-hands** | "Hands system for OpenFang — curated autonomous capability packages" | 7 bundled Hands with HAND.toml parser, `HandRegistry` managing definitions and active instances, lifecycle management (activate/pause/resume/deactivate), requirement checking (binary/env/API key), settings resolution, project-local hand loading with security sandboxing. |
| 8 | **openfang-skills** | "Skill system for OpenFang" | 60 bundled skills compiled via `include_str!()`. `SkillManifest` with metadata, runtime config, tools, requirements. `SkillRegistry` for installed/bundled skills. `FangHubClient` for FangHub marketplace. `ClawHubClient` for clawhub.ai cross-ecosystem discovery. SKILL.md parser for OpenClaw compat. `SkillVerifier` with SHA256. Prompt injection scanner. |
| 9 | **openfang-extensions** | "Extension & integration system for OpenFang — one-click MCP server setup, credential vault, OAuth2 PKCE" | 25 MCP templates, AES-256-GCM credential vault, OAuth2 PKCE flow for third-party integrations. |
| 10 | **openfang-wire** | "OpenFang Protocol (OFP) — agent-to-agent networking" | JSON-framed TCP with HMAC-SHA256 mutual auth (nonce + constant-time verify via `subtle`). `PeerNode` for connections, `PeerRegistry` for tracking remote peers and their agents. |
| 11 | **openfang-cli** | "CLI tool for the OpenFang Agent OS" | Clap-based CLI binary (`openfang`). Commands: `init`, `start`, `status`, `doctor`, `agent spawn/list/chat/kill`, `workflow`, `trigger`, `migrate`, `skill install/list/remove/search/create`, `channel`, `config`, `chat`, `mcp`, `hand activate/pause/list/status`. Daemon auto-detect via `~/.openfang/daemon.json` + health pings. Ratatui TUI dashboard. |
| 12 | **openfang-desktop** | "Native desktop application for the OpenFang Agent OS (Tauri 2.0)" | Boots kernel in-process, runs Axum on background thread, WebView at `http://127.0.0.1:{port}`. System tray, single-instance, desktop notifications, hide-to-tray. IPC commands: `get_port`, `get_status`. Mobile-ready with `#[cfg(desktop)]` guards. |
| 13 | **openfang-migrate** | "Migration engine for importing from other agent frameworks into OpenFang" | Supports OpenClaw (`~/.openclaw/`), LangChain, AutoGPT. Converts YAML->TOML, maps tool/provider names, imports agent manifests, copies memory files, converts channel configs. Produces `MigrationReport`. |
| 14 | **xtask** | "Build automation for the OpenFang workspace" | cargo-xtask pattern for build automation tasks. |

---

## 2. Agent System

### 2.1 Agent Identity & Manifest

Every agent is defined by an `AgentManifest` (file: `crates/openfang-types/src/agent.rs`), which is the complete declarative specification of an agent. Agents are defined in `agent.toml` files within the `agents/` directory.

**Key types:**

- **`AgentId`** — UUID v4 wrapper for unique agent identification
- **`SessionId`** — UUID v4 wrapper for session tracking
- **`AgentEntry`** — Runtime registry entry (ID + name + manifest + state + mode + timestamps + parent/children + session + identity)
- **`AgentManifest`** — Full agent definition (see below)
- **`AgentIdentity`** — Visual identity (emoji, avatar URL, color, archetype, vibe, greeting style)

### 2.2 AgentManifest Structure

```toml
# Example: agents/hello-world/agent.toml
name = "hello-world"
version = "0.1.0"
description = "A friendly greeting agent"
author = "openfang"
module = "builtin:chat"    # or "wasm:path/to.wasm" or "python:path/to.py"

[model]
provider = "groq"
model = "llama-3.3-70b-versatile"
max_tokens = 4096
temperature = 0.6
system_prompt = """You are Hello World..."""

[resources]
max_llm_tokens_per_hour = 100000
# Also: max_memory_bytes, max_cpu_time_ms, max_tool_calls_per_minute,
#        max_network_bytes_per_hour, max_cost_per_hour_usd,
#        max_cost_per_day_usd, max_cost_per_month_usd

[capabilities]
tools = ["file_read", "file_list", "web_fetch", "web_search", "memory_store", "memory_recall"]
network = ["*"]
memory_read = ["*"]
memory_write = ["self.*"]
agent_spawn = false
# Also: shell, agent_message, ofp_discover, ofp_connect
```

**Full manifest fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Human-readable name |
| `version` | String | Semantic version |
| `description` | String | What the agent does |
| `author` | String | Author identifier |
| `module` | String | Execution module: `builtin:chat`, `wasm:path`, `python:path` |
| `schedule` | ScheduleMode | `reactive` (default), `periodic {cron}`, `proactive {conditions}`, `continuous {check_interval_secs}` |
| `model` | ModelConfig | Provider, model ID, max_tokens, temperature, system_prompt, api_key_env, base_url |
| `fallback_models` | Vec | Chain of fallback models tried in order |
| `resources` | ResourceQuota | Rate limits and cost caps |
| `priority` | Priority | Low / Normal / High / Critical |
| `capabilities` | ManifestCapabilities | Security grants (see Capability section) |
| `profile` | Option<ToolProfile> | Named preset: Minimal, Coding, Research, Messaging, Automation, Full, Custom |
| `tools` | HashMap | Per-tool configuration parameters |
| `skills` | Vec | Installed skill references (empty = all) |
| `mcp_servers` | Vec | MCP server allowlist (empty = all) |
| `routing` | Option<ModelRoutingConfig> | Auto-select models by complexity (simple/medium/complex) |
| `autonomous` | Option<AutonomousConfig> | Guardrails for 24/7 agents (quiet hours, max iterations/restarts, heartbeat) |
| `workspace` | Option<PathBuf> | Agent workspace directory |
| `exec_policy` | Option<ExecPolicy> | Per-agent execution policy override |
| `generate_identity_files` | bool | Whether to create SOUL.md, USER.md, etc. on spawn (default: true) |

### 2.3 Agent States

```
    spawn                    message/tick              kill
      |                         |                       |
      v                         v                       v
  [Created] --> [Running] <-> [Running] ---------> [Terminated]
                    |                                    ^
                    |          shutdown                   |
                    +-------> [Suspended] ---------------+
                    |              |     reboot/restore
                    +-------> [Crashed]
```

- **Created**: Agent initialized but not yet started
- **Running**: Active, processing events
- **Suspended**: Paused (daemon shutdown); persisted to SQLite for restore
- **Terminated**: Killed; removed from registry and storage
- **Crashed**: Awaiting recovery

### 2.4 Agent Modes

| Mode | Behavior |
|------|----------|
| **Full** (default) | Unrestricted access to all granted tools |
| **Assist** | Read-only tools only: `file_read`, `file_list`, `memory_recall`, `web_fetch`, `web_search`, `agent_list` |
| **Observe** | No tool access at all — agent can only observe |

### 2.5 Agent Lifecycle

**Spawn Flow:**
1. Generate `AgentId` (UUID v4) and `SessionId`
2. Create session in memory substrate
3. Parse manifest, extract capabilities
4. Validate capability inheritance (prevents privilege escalation)
5. Grant capabilities via `CapabilityManager`
6. Register with `AgentScheduler` (quota tracking)
7. Create `AgentEntry`, register in `AgentRegistry` (DashMap)
8. Persist to SQLite via `memory.save_agent()`
9. If parent exists, update parent's children list
10. Register proactive triggers if schedule is `Proactive`
11. Publish `Lifecycle::Spawned` event

**Message Flow:**
1. RBAC check (channel identity resolution + role permissions)
2. Channel policy check (DM/group policy enforcement)
3. Quota check (token-per-hour limit)
4. Module dispatch based on `manifest.module`:
   - `builtin:chat`: LLM agent loop
   - `wasm:path`: WASM sandbox execution
   - `python:path`: Python subprocess (env_clear + selective vars)
5. LLM agent loop (for builtin:chat):
   - Load/create session from memory
   - Load canonical context (cross-channel memory)
   - Append stability guidelines to system prompt
   - Resolve LLM driver (per-agent override or kernel default)
   - Gather tools (filtered by capabilities + skill tools + MCP tools)
   - Initialize loop guard + session repair
   - Iterative loop: LLM call -> tool execution -> accumulate
   - Auto-compact session if threshold exceeded
   - Save session + canonical session
6. Cost estimation via `MeteringEngine`
7. Record usage event
8. Return `AgentLoopResult` (response, tokens, iterations, cost_usd)

**Kill Flow:**
1. Check `AgentKill(target_name)` capability
2. Remove from `AgentRegistry`
3. Stop background loops
4. Unregister from scheduler, revoke capabilities
5. Unsubscribe from `EventBus`, remove triggers
6. Remove from SQLite

### 2.6 Pre-built Agent Templates

30 agent templates ship in the `agents/` directory, each with an `agent.toml`:

| Template | Description |
|----------|-------------|
| analyst | Data analysis |
| architect | System architecture |
| assistant | General purpose |
| code-reviewer | Code review |
| coder | Software development |
| customer-support | Support automation |
| data-scientist | Data science |
| debugger | Bug investigation |
| devops-lead | DevOps management |
| doc-writer | Documentation |
| email-assistant | Email management |
| health-tracker | Health monitoring |
| hello-world | Beginner-friendly demo |
| home-automation | IoT/home control |
| legal-assistant | Legal research |
| meeting-assistant | Meeting management |
| ops | Operations |
| orchestrator | Multi-agent orchestration |
| personal-finance | Finance tracking |
| planner | Project planning |
| recruiter | Hiring pipeline |
| researcher | Research |
| sales-assistant | Sales support |
| security-auditor | Security analysis |
| social-media | Social media management |
| test-engineer | Testing |
| translator | Translation |
| travel-planner | Travel planning |
| tutor | Education |
| writer | Content writing |

### 2.7 Agent Storage

Agents are persisted to SQLite (via `openfang-memory`). The kernel restores all agents from the database on boot, re-registering them in the in-memory registry, capabilities manager, and scheduler. Session history, structured KV data, and usage events are all stored in the same SQLite database.

---

## 3. Hands System

### 3.1 What Are Hands?

**Hands are OpenFang's core differentiator** — pre-built autonomous capability packages that run independently, on schedules, without user prompting. Unlike regular agents (you chat with them), Hands work *for* you (you check in on them).

Think of a Hand as a domain-complete agent configuration that bundles:
- **HAND.toml** — Manifest declaring tools, settings, requirements, and dashboard metrics
- **System Prompt** — Multi-phase operational playbook (500+ words, not a one-liner)
- **SKILL.md** — Domain expertise reference injected into context at runtime
- **Guardrails** — Approval gates for sensitive actions

### 3.2 Hand Definition Structure (HAND.toml)

```toml
id = "clip"
name = "Clip Hand"
description = "Turns long-form video into viral short clips"
category = "content"       # content | security | productivity | development | communication | data
icon = "🎬"
tools = ["shell_exec", "file_read", "file_write", "file_list", "web_fetch", "memory_store", "memory_recall"]
skills = []                # Skill allowlist (empty = all)
mcp_servers = []           # MCP server allowlist (empty = all)

# Requirements that must be satisfied before activation
[[requires]]
key = "ffmpeg"
label = "FFmpeg must be installed"
requirement_type = "binary"      # binary | env_var | api_key
check_value = "ffmpeg"
description = "FFmpeg is the core video processing engine."
[requires.install]
macos = "brew install ffmpeg"
windows = "winget install Gyan.FFmpeg"
linux_apt = "sudo apt install ffmpeg"
estimated_time = "2-5 min"

# Configurable settings (shown in activation modal)
[[settings]]
key = "stt_provider"
label = "Speech-to-Text Provider"
description = "How audio is transcribed"
setting_type = "select"     # select | text | toggle
default = "auto"

[[settings.options]]
value = "groq_whisper"
label = "Groq Whisper API (fast, free tier)"
provider_env = "GROQ_API_KEY"

# Agent configuration
[agent]
name = "clip-hand"
description = "AI video editor"
module = "builtin:chat"
provider = "default"
model = "default"
max_tokens = 8192
temperature = 0.4
max_iterations = 40
system_prompt = """You are Clip Hand — an AI-powered shorts factory..."""

# Dashboard metrics
[dashboard]
[[dashboard.metrics]]
label = "Jobs Completed"
memory_key = "clip_hand_jobs_completed"
format = "number"
```

### 3.3 The 7 Bundled Hands

All shipped as compiled-in TOML + SKILL.md in `crates/openfang-hands/bundled/`:

| Hand | Category | What It Does |
|------|----------|-------------|
| **Clip** | Content | 8-phase video pipeline: downloads videos, transcribes (5 STT backends), identifies viral moments, cuts vertical shorts with burned captions and thumbnails, optional TTS voice-over, publishes to Telegram/WhatsApp |
| **Lead** | Data | Daily lead generation: discovers prospects matching ICP, enriches with web research, scores 0-100, deduplicates, delivers qualified leads in CSV/JSON/Markdown |
| **Collector** | Data | OSINT intelligence: continuous monitoring of targets (company/person/topic), change detection, sentiment tracking, knowledge graph construction, critical alerts |
| **Predictor** | Data | Superforecasting: collects signals, builds calibrated reasoning chains, predictions with confidence intervals, tracks accuracy via Brier scores, contrarian mode |
| **Researcher** | Data | Deep research: cross-references sources, CRAAP credibility evaluation, cited reports with APA formatting, multi-language support |
| **Twitter** | Communication | Autonomous Twitter/X manager: 7 rotating content formats, scheduled posts, mention responses, performance tracking, approval queue |
| **Browser** | Productivity | Web automation via Playwright bridge: navigation, form filling, multi-step workflows, session persistence, mandatory purchase approval gate |

### 3.4 Hand Lifecycle

```
[Inactive] --activate--> [Active] --pause--> [Paused] --resume--> [Active]
                |                                                      |
                |         --deactivate/error-->  [Inactive/Error]  <---+
```

**Activation flow:**
1. User calls `openfang hand activate <hand_id>` (or API `POST /api/hands/{id}/activate`)
2. `HandRegistry.activate()` creates a `HandInstance` with status `Active`
3. Kernel spawns an agent from the hand's `[agent]` config
4. `HandRegistry.set_agent()` links the instance to the spawned `AgentId`
5. Hand is now running autonomously

**HandInstance** tracks: `instance_id` (UUID), `hand_id`, `status`, `agent_id`, `agent_name`, `config` (user overrides), `activated_at`, `updated_at`.

### 3.5 Hand Registry

`HandRegistry` (file: `crates/openfang-hands/src/registry.rs`) manages:
- **Definitions**: `HashMap<String, HandDefinition>` — all known hands (bundled + project-local)
- **Instances**: `DashMap<Uuid, HandInstance>` — active hand instances

Key operations:
- `load_bundled()` — loads 7 compiled-in hands
- `load_from_directory(project_dir)` — loads project-local hands from `.openfang/hands/`
- `activate(hand_id, config)` — creates instance (kernel spawns agent separately)
- `deactivate(instance_id)` — removes instance
- `pause(instance_id)` / `resume(instance_id)` — lifecycle control
- `check_requirements(hand_id)` — verifies binaries/env vars/API keys
- `check_settings_availability(hand_id)` — checks which setting options are available

### 3.6 Project-Local Hands

Hands can be defined in a project's `.openfang/hands/` directory. Security restrictions apply:
- **Namespaced** as `project:{dir_name}:{hand_id}`
- **`shell_exec` stripped** from tools (security)
- **`api_key_env` nullified** on agent config
- **MCP servers blocked** entirely
- **Symlinks rejected** to prevent escape
- **Colon in ID rejected** (reserved namespace separator)

### 3.7 Settings Resolution

When activating a hand, user config values are resolved against the hand's settings schema via `resolve_settings()`. This produces:
- A **prompt block** appended to the system prompt (e.g., "## User Configuration\n- STT: Groq Whisper")
- A list of **env vars** the agent's subprocess needs access to

---

## 4. Kernel

### 4.1 Overview

`OpenFangKernel` (file: `crates/openfang-kernel/src/kernel.rs`, ~198K lines — the largest file) is the central coordinator. It assembles all subsystems and implements the `KernelHandle` trait that enables inter-agent operations without circular crate dependencies.

### 4.2 Subsystems

| Subsystem | Purpose |
|-----------|---------|
| `AgentRegistry` | DashMap-based concurrent agent store |
| `AgentScheduler` | Quota tracking per agent, hourly window reset |
| `CapabilityManager` | DashMap-based capability grants and enforcement |
| `EventBus` | Async broadcast channel for system events |
| `Supervisor` | Health monitoring, panic/restart counters |
| `WorkflowEngine` | Workflow registration and execution (run eviction cap 200) |
| `TriggerEngine` | Event pattern matching |
| `BackgroundExecutor` | Continuous/periodic agent loops |
| `WasmSandbox` | Wasmtime engine with dual fuel+epoch metering |
| `ModelCatalog` | 51+ builtin models, 20+ aliases, 20 providers |
| `MeteringEngine` | Cost catalog (20+ model families), usage tracking |
| `ModelRouter` | TaskComplexity scoring, automatic model selection |
| `AuthManager` | RBAC with UserRole hierarchy |
| `HeartbeatMonitor` | Background agent health checks |
| `SetupWizard` | Interactive first-run configuration |
| `SkillRegistry` | Bundled + installed skill management |
| `HandRegistry` | Hand definitions and instances |
| `WebToolsContext` | Web search (4-provider cascade) + web fetch (SSRF-protected) |

### 4.3 Boot Sequence

1. Load `~/.openfang/config.toml` (with serde defaults)
2. Create `~/.openfang/data/` directory
3. Initialize SQLite memory substrate (schema migrations to v5)
4. Initialize LLM driver from config
5. Build model catalog (51 models, 20+ aliases, 20 providers)
6. Initialize metering engine (20+ model families)
7. Initialize model router with TaskComplexity scoring
8. Initialize core subsystems (registry, scheduler, capabilities, events, supervisor, workflows, triggers, background executor, WASM sandbox)
9. Initialize RBAC auth manager
10. Load skill registry (60 bundled + user-installed)
11. Initialize web tools (search engine + fetch engine)
12. Restore persisted agents from SQLite
13. Publish `KernelStarted` event
14. (After Arc wrap) Connect MCP servers, start heartbeat, start background loops

### 4.4 KernelHandle Trait

Defined in `openfang-runtime`, implemented by `OpenFangKernel`. Enables tools like `agent_send` and `agent_spawn` to interact with the kernel without circular crate dependencies:

```rust
#[async_trait]
pub trait KernelHandle: Send + Sync {
    async fn send_message_to_agent(&self, agent_id: AgentId, message: String) -> Result<String>;
    async fn spawn_agent(&self, manifest: AgentManifest) -> Result<AgentId>;
    async fn list_agents(&self) -> Vec<AgentEntry>;
    // ... more operations
}
```

---

## 5. API & Server Layer

### 5.1 Server Architecture

Built on **Axum 0.8** (file: `crates/openfang-api/src/server.rs`). `AppState` bridges the kernel to API routes:

```rust
struct AppState {
    kernel: Arc<OpenFangKernel>,
    peer_registry: Option<Arc<PeerRegistry>>,
    // ... other shared state
}
```

### 5.2 Endpoint Categories

140+ endpoints organized as:

| Category | Endpoints | Description |
|----------|-----------|-------------|
| **Agents** | `GET/POST /api/agents`, `GET/PUT/DELETE /api/agents/{id}`, `POST /api/agents/{id}/message` | CRUD + messaging |
| **Hands** | `GET /api/hands`, `POST /api/hands/{id}/activate`, etc. | Hand lifecycle |
| **Workflows** | `GET/POST /api/workflows`, `POST /api/workflows/{id}/run` | Workflow engine |
| **Triggers** | `GET/POST/DELETE /api/triggers` | Event triggers |
| **Memory** | `GET/POST /api/memory`, search, knowledge graph | Memory operations |
| **Budget** | `GET/PUT /api/budget`, `GET /api/budget/agents` | Cost tracking |
| **Channels** | `GET /api/channels`, enable/disable/config | Channel management |
| **Skills** | `GET /api/skills`, install/remove/search | Skill management |
| **Models** | `GET /api/models`, `GET /api/providers` | Model catalog |
| **MCP** | `GET /api/mcp/servers`, connect/disconnect | MCP management |
| **A2A** | `GET /api/a2a/agents`, discover, send | Agent-to-agent |
| **Network** | `GET /api/network/status`, `GET /api/peers` | OFP networking |
| **OpenAI** | `POST /v1/chat/completions`, `GET /v1/models` | Drop-in compatibility |
| **Dashboard** | `GET /` | Alpine.js SPA served from `static/index_body.html` |
| **System** | `/api/health`, `/api/status`, `/api/version`, `/api/shutdown` | System operations |

### 5.3 Middleware Stack

- **Bearer token auth** — Optional API key validation
- **Request ID injection** — UUID per request
- **Structured request logging** — Tracing integration
- **GCRA rate limiter** — Cost-aware token bucket with per-IP tracking
- **Security headers** — CSP, X-Frame-Options, HSTS, X-Content-Type-Options
- **Health endpoint redaction** — Minimal info publicly, full diagnostics authenticated

### 5.4 Real-Time Communication

- **WebSocket** — `/ws/agents/{id}/chat` for real-time streaming agent chat
- **SSE** — Server-Sent Events for streaming LLM responses
- **Web Chat** — Embeddable web chat widget

---

## 6. Configuration System

### 6.1 Config File Location

`~/.openfang/config.toml` (or custom path via CLI flag)

### 6.2 Config Structure

Defined in `crates/openfang-types/src/config.rs`. Key sections:

```toml
# API server
api_key = ""                          # Bearer auth token
api_listen = "127.0.0.1:50051"        # HTTP bind address

# Default LLM model
[default_model]
provider = "anthropic"                # anthropic | gemini | openai | groq | ollama | ...
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"

# Memory
[memory]
decay_rate = 0.05
# sqlite_path = "~/.openfang/data/openfang.db"

# Network / OFP
[network]
listen_addr = "127.0.0.1:4200"
# shared_secret = ""

# Session compaction
[compaction]
threshold = 80
keep_recent = 20
max_summary_tokens = 1024

# Usage display
usage_footer = "Full"                 # Off | Tokens | Cost | Full

# Web tools
[web]
search_provider = "auto"              # auto | brave | tavily | perplexity | duckduckgo
cache_ttl_minutes = 15
[web.brave]
api_key_env = "BRAVE_API_KEY"
max_results = 5
[web.tavily]
api_key_env = "TAVILY_API_KEY"
[web.fetch]
max_size_bytes = 1048576

# Channel adapters
[telegram]
bot_token_env = "TELEGRAM_BOT_TOKEN"
allowed_users = []
[discord]
bot_token_env = "DISCORD_BOT_TOKEN"
# ... 40 adapters total

# Per-channel overrides
[telegram.overrides]
model = "claude-haiku-4-5-20251001"
dm_policy = "respond"                 # respond | allowed_only | ignore
group_policy = "mention_only"         # all | mention_only | commands_only | ignore
rate_limit_per_user = 10
output_format = "telegram_html"

# MCP servers
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]

# Multi-user RBAC
[users.alice]
name = "Alice"
role = "owner"
channel_bindings = { telegram = "123456" }
```

### 6.3 Config Conventions

- All config structs use `#[serde(default)]` — missing fields get sensible defaults
- New config fields **must** be added to both the struct AND the `Default` impl
- Config is hot-reloadable via `config_reload.rs` (watches file for changes)
- `KernelMode` controls behavior: `Stable` (conservative), `Default` (balanced), `Dev` (experimental)

---

## 7. Skills System

### 7.1 Overview

Skills are pluggable tool bundles that extend agent capabilities. They can be:
- **PromptOnly** (default) — Markdown injected into LLM system prompt
- **Python** — Script executed in subprocess
- **WASM** — Module executed in sandbox
- **Node.js** — For OpenClaw compatibility
- **Builtin** — Compiled into the binary

### 7.2 Skill Manifest (skill.toml)

```toml
[skill]
name = "web-research"
version = "0.1.0"
description = "Advanced web research capabilities"
author = "openfang"
tags = ["research", "web"]

[runtime]
runtime = "prompt_only"

[tools]
# Tools provided by this skill

[requirements]
tools = ["web_search", "web_fetch"]
```

### 7.3 Provenance

Skills track their origin via `SkillSource`:
- **Native** — Built into OpenFang or manually installed
- **Bundled** — Ships with the binary (60 skills)
- **OpenClaw** — Converted from OpenClaw format
- **ClawHub** — Downloaded from clawhub.ai marketplace

### 7.4 Security

- `SkillVerifier` — SHA256 hash verification
- `scan_prompt_content()` — Detects prompt injection: override attempts, data exfiltration patterns, shell reference injection

---

## 8. Memory Substrate

### 8.1 Six Storage Layers

| Layer | Description |
|-------|-------------|
| **Structured KV Store** | Per-agent key-value (JSON values) backed by SQLite. Shared namespace for cross-agent data. |
| **Semantic Search** | Vector embeddings for similarity-based memory retrieval (cosine similarity) |
| **Knowledge Graph** | Entities and relations stored in SQLite |
| **Session Management** | Conversation history with block-aware compaction |
| **Task Board** | Per-agent task tracking |
| **Usage Events** | Token counts, cost, provider, model per interaction |

### 8.2 Schema Versions

| Version | What It Added |
|---------|--------------|
| V1 | Core tables: agents, sessions, messages, memory KV |
| V2 | Collaboration tables |
| V3 | Embedding/vector tables |
| V4 | Usage events table |
| V5 | Canonical sessions (cross-channel memory) |

### 8.3 Canonical Sessions

Cross-channel memory: if a user talks to the same agent via Telegram and then Discord, the canonical session summary carries context across channels.

---

## 9. Channel Adapters

40 adapters implementing the `ChannelAdapter` trait (file: `crates/openfang-channels/src/lib.rs`):

**Core:** Telegram, Discord, Slack, WhatsApp, Signal, Matrix, Email (IMAP/SMTP)
**Enterprise:** Teams, Mattermost, Google Chat, Webex, Feishu/Lark, Zulip
**Social:** LINE, Viber, Messenger, Mastodon, Bluesky, Reddit, LinkedIn, Twitch
**Community:** IRC, XMPP, Guilded, Revolt, Keybase, Discourse, Gitter
**Privacy:** Threema, Nostr, Mumble, Nextcloud Talk, Rocket.Chat, Ntfy, Gotify
**Workplace:** Pumble, Flock, Twist, DingTalk, Zalo, Webhooks

Each adapter supports:
- Per-channel **model overrides**
- **DM/group policies** (respond/allowed_only/ignore for DMs; all/mention_only/commands_only/ignore for groups)
- **Rate limiting** (per-user DashMap tracking)
- **Output formatting** (Markdown -> TelegramHTML / SlackMrkdwn / PlainText)
- **Threading** support

---

## 10. Wire Protocol (OFP)

OpenFang Protocol for peer-to-peer agent communication:
- JSON-framed messages over TCP
- HMAC-SHA256 mutual authentication (nonce + constant-time verification via `subtle`)
- `PeerNode` — Listens for connections, manages peers
- `PeerRegistry` — Tracks known remote peers and their agents

---

## 11. Security Model

16 discrete security systems (defense in depth):

| # | System | Implementation |
|---|--------|---------------|
| 1 | WASM Dual-Metered Sandbox | Wasmtime with fuel + epoch interruption + watchdog thread |
| 2 | Merkle Hash-Chain Audit | Cryptographic linking of every action |
| 3 | Information Flow Taint | `TaintLabel` / `TaintSet` propagation from source to sink |
| 4 | Ed25519 Signed Manifests | Agent identity + capability cryptographic signing |
| 5 | SSRF Protection | Blocks private IPs, cloud metadata, DNS rebinding |
| 6 | Secret Zeroization | `Zeroizing<String>` auto-wipes API keys from memory |
| 7 | OFP Mutual Auth | HMAC-SHA256 nonce-based, constant-time verification |
| 8 | Capability Gates | Role-based access control with glob pattern matching |
| 9 | Security Headers | CSP, X-Frame-Options, HSTS, X-Content-Type-Options |
| 10 | Health Redaction | Minimal public health, full diagnostics require auth |
| 11 | Subprocess Sandbox | `env_clear()` + selective variable passthrough |
| 12 | Prompt Injection Scanner | Detects overrides, exfiltration, shell references |
| 13 | Loop Guard | SHA256-based tool call loop detection + circuit breaker |
| 14 | Session Repair | 7-phase message history validation and recovery |
| 15 | Path Traversal Prevention | Canonicalization + symlink escape prevention |
| 16 | GCRA Rate Limiter | Cost-aware token bucket with per-IP tracking |

### Capability-Based Security

Capabilities (file: `crates/openfang-types/src/capability.rs`) are fine-grained permissions:

| Category | Capabilities |
|----------|-------------|
| File System | `FileRead(glob)`, `FileWrite(glob)` |
| Network | `NetConnect(pattern)`, `NetListen(port)` |
| Tools | `ToolInvoke(id)`, `ToolAll` |
| LLM | `LlmQuery(pattern)`, `LlmMaxTokens(n)` |
| Agent | `AgentSpawn`, `AgentMessage(pattern)`, `AgentKill(pattern)` |
| Memory | `MemoryRead(scope)`, `MemoryWrite(scope)` |
| Shell | `ShellExec(pattern)`, `EnvRead(pattern)` |
| OFP | `OfpDiscover`, `OfpConnect(pattern)`, `OfpAdvertise` |
| Economic | `EconSpend(usd)`, `EconEarn`, `EconTransfer(pattern)` |

Pattern matching supports `*` wildcards, glob patterns (`*.openai.com:443`), and exact matches. `validate_capability_inheritance()` prevents privilege escalation: a restricted parent cannot create an unrestricted child.

---

## 12. Workspace & Project Context

### 12.1 Workspace Detection

`WorkspaceContext` (file: `crates/openfang-runtime/src/workspace_context.rs`) auto-detects:
- **Project type**: Rust, Node.js, Python, Go, Java, .NET, Unknown (via marker files)
- **Git repository**: Checks for `.git/` directory
- **OpenFang directory**: Checks for `.openfang/`
- **Context files**: `AGENTS.md`, `SOUL.md`, `TOOLS.md`, `IDENTITY.md`, `HEARTBEAT.md`

Context files are mtime-cached (32KB max) and injected into agent system prompts.

### 12.2 Workspace State

`.openfang/workspace-state.json` tracks:
- Format version
- Bootstrap seed timestamp
- Onboarding completion timestamp

### 12.3 Git Integration

Minimal built-in git support:
- Workspace context detects if working directory is a git repo
- No built-in version control of agents/hands (agents are defined as static TOML files)
- The `shell_exec` tool can run git commands when agents have shell capabilities

---

## 13. Existing Documentation Map

All documentation lives in the `docs/` directory:

| File | Size | Content |
|------|------|---------|
| `README.md` | 3.5K | Docs hub / index |
| `getting-started.md` | 9.5K | Installation and first-run guide |
| `architecture.md` | 44.5K | Deep internal architecture (crate structure, boot sequence, agent lifecycle, memory, LLM drivers, security, channels, skills, MCP/A2A, OFP, desktop) |
| `configuration.md` | 53.9K | Complete config.toml reference |
| `api-reference.md` | 46K | All API endpoints documentation |
| `cli-reference.md` | 29.1K | CLI commands reference |
| `agent-templates.md` | 41.7K | All 30 agent templates documented |
| `channel-adapters.md` | 24.5K | Channel adapter setup guides |
| `providers.md` | 33K | LLM provider configuration |
| `security.md` | 47.5K | Security model deep-dive |
| `skill-development.md` | 16.4K | How to create custom skills |
| `workflows.md` | 27.5K | Workflow engine documentation |
| `mcp-a2a.md` | 24.5K | MCP and A2A protocol docs |
| `desktop.md` | 15.6K | Desktop app (Tauri) docs |
| `production-checklist.md` | 8.7K | Production deployment guide |
| `troubleshooting.md` | 13.8K | Common issues and solutions |
| `launch-roadmap.md` | 20.7K | Roadmap and planned features |

Additionally:
- `CLAUDE.md` — Agent instructions for AI development assistants
- `README.md` (root) — Project overview, benchmarks, feature comparison
- `openfang.toml.example` — Example configuration file

---

## Summary

OpenFang is a comprehensive Agent OS with a clean layered architecture:

1. **Types layer** (`openfang-types`) defines all shared data structures
2. **Storage layer** (`openfang-memory`) provides SQLite-backed persistence
3. **Execution layer** (`openfang-runtime`) runs agent loops with LLM drivers and tools
4. **Coordination layer** (`openfang-kernel`) orchestrates all subsystems
5. **Interface layers** (`openfang-api`, `openfang-cli`, `openfang-desktop`) expose the system
6. **Extension layers** (`openfang-hands`, `openfang-skills`, `openfang-channels`, `openfang-extensions`) add capabilities
7. **Networking layer** (`openfang-wire`) enables P2P agent communication
8. **Migration layer** (`openfang-migrate`) handles framework transitions

The Hands system is the key innovation — pre-built autonomous packages that transform agents from chatbots into autonomous workers that operate on schedules, build knowledge graphs, and deliver results without constant human prompting.
