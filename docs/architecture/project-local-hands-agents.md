# Project-Local Hands & Agents: Architecture Plan

## Developer Experience Vision

### The Story

A team builds an internal deploy pipeline. They want their OpenFang agents
and hands version-controlled alongside their code, so any teammate who clones
the repo gets the same agent configurations. But runtime state (conversations,
memory, secrets) must never leak into git.

### What the Developer Does

```bash
# 1. Clone the project
git clone https://github.com/acme/backend-api.git
cd backend-api

# 2. The repo already has .openfang/ checked in:
#
#    .openfang/
#      .gitignore            ← ignores runtime state, secrets
#      hands/
#        deploy/
#          HAND.toml          ← hand definition (version controlled)
#          SKILL.md           ← reference knowledge (version controlled)
#        monitor/
#          HAND.toml
#          SKILL.md
#      agents/
#        backend-coder/
#          agent.toml         ← agent template manifest (version controlled)
#          SOUL.md            ← identity seed (version controlled)
#          TOOLS.md           ← environment docs (version controlled)
#          skills/            ← agent-specific skills (version controlled)
#            deploy-check.md
#        qa-engineer/
#          agent.toml
#          SOUL.md

# 3. Start the daemon, pointing at this project
openfang start --project .

# 4. Project hands appear in the marketplace, tagged [project]
#    They are NOT auto-activated -- the developer chooses which to activate.
#    This is critical for security: the developer must consent.
curl -s http://127.0.0.1:4200/api/hands | python3 -c "
import sys, json
for h in json.load(sys.stdin)['hands']:
    src = '[project]' if h.get('source') == 'project' else '[bundled]'
    print(f\"  {src} {h['id']}: {h['name']}\")
"
#   [project] project:backend-api:deploy: Deploy Hand
#   [project] project:backend-api:monitor: Monitor Hand
#   [bundled] browser: Browser Hand
#   [bundled] clip: Clipboard Hand
#   ... 5 more bundled hands

# 5. Activate a project hand (same UX as bundled hands)
curl -s -X POST http://127.0.0.1:4200/api/hands/project:backend-api:deploy/activate \
  -H "Content-Type: application/json" \
  -d '{"config": {"target_env": "staging"}}'

# 6. Project agent templates appear in the template picker
openfang agent new backend-coder
# Shows: "backend-coder [project] - Expert backend engineer for this repo"

# 7. The spawned agent gets a runtime workspace (outside the repo):
#    ~/.openfang/workspaces/backend-coder-a1b2c3d4/
#      SOUL.md     ← COPIED from .openfang/agents/backend-coder/SOUL.md (seed)
#      TOOLS.md    ← COPIED from .openfang/agents/backend-coder/TOOLS.md (seed)
#      USER.md     ← generated (agent learns about THIS user)
#      MEMORY.md   ← generated (agent's personal long-term notes)
#      AGENT.json  ← runtime metadata
#      skills/     ← COPIED from .openfang/agents/backend-coder/skills/
#      data/       ← runtime data
#      output/     ← runtime output
#      sessions/   ← JSONL conversation mirrors
#      logs/       ← runtime logs
#      memory/     ← runtime memory files

# 8. The developer chats with their project agent
openfang chat backend-coder
# Agent has the SOUL.md personality, TOOLS.md context, and deploy-check.md skill
# from the project repo -- the team agreed on these.

# 9. Another developer clones the same repo:
#    - They get the same HAND.toml, agent.toml, SOUL.md, TOOLS.md, skills/
#    - They activate the same hands with their own settings
#    - Their runtime workspace is their own (separate conversations, memory)
#    - No secrets cross between developers
```

### What Gets Version Controlled vs What Doesn't

```
.openfang/                         ← CHECKED INTO GIT
  .gitignore                       ← auto-generated, protects runtime state
  hands/
    deploy/
      HAND.toml                    ← hand definition
      SKILL.md                     ← reference knowledge
  agents/
    backend-coder/
      agent.toml                   ← agent template manifest
      SOUL.md                      ← identity seed (team-agreed personality)
      TOOLS.md                     ← environment docs (team-agreed tooling notes)
      skills/
        deploy-check.md            ← agent-specific prompt skills

~/.openfang/workspaces/            ← NEVER IN GIT (user-local runtime)
  backend-coder-a1b2c3d4/
    SOUL.md                        ← copied from seed, agent may evolve it
    USER.md                        ← what agent learned about THIS user
    MEMORY.md                      ← agent's personal long-term notes
    data/, output/, sessions/, ... ← runtime artifacts
```

### The .gitignore Inside .openfang/

Auto-generated when a developer runs `openfang init --project` or manually creates
`.openfang/`. The system also checks for its existence when loading project-local
definitions and warns if missing.

```gitignore
# OpenFang project-local runtime state -- DO NOT COMMIT
# This file is auto-generated. Edit with care.

# Runtime workspaces (per-user agent state)
workspaces/

# Database and state files
*.db
*.db-wal
*.db-shm
state/

# Secrets
.env
.env.*
*.key
*.pem
vault.enc

# User-specific identity files (generated per-user, not team-shared)
# Note: SOUL.md and TOOLS.md in agents/ directories ARE meant to be shared.
# Only top-level USER.md should be ignored.
USER.md
```

---

## Architecture Design

### 1. Project Discovery: How the Daemon Finds `.openfang/`

**Primary mechanism**: Explicit config field + CLI flag, with CWD as convenience fallback.

```toml
# ~/.openfang/config.toml
project_dirs = ["~/acme/backend-api", "~/acme/frontend"]
```

```bash
# CLI flag (highest priority, overrides config)
openfang start --project ~/acme/backend-api

# CWD fallback (lowest priority, convenience for `cd project && openfang start`)
cd ~/acme/backend-api && openfang start
# Detects .openfang/ in CWD if project_dirs is empty and no --project flag
```

**Resolution order**:
1. `--project` CLI flag(s) → absolute path(s)
2. `project_dirs` from `config.toml` → expanded paths
3. CWD check → only if neither of the above provided anything

**Implementation**:
- Add `project_dirs: Vec<PathBuf>` to `KernelConfig` with `#[serde(default)]`
- Add `--project` flag to the `start` CLI command (in openfang-cli -- **note**: this is a CLI arg, not modifying CLI internals, which is allowed per CLAUDE.md)
- The kernel's `boot_with_config()` receives the resolved list of project directories
- Each directory is validated: must exist, must contain `.openfang/`, must be readable

**Why not CWD-only**: The daemon is a long-running background process. Its CWD is
unpredictable when started via systemd, launchd, or background spawn. Explicit config
is the only reliable mechanism.

### 2. Hand Loading: `load_from_directory()`

**New method on `HandRegistry`** in `openfang-hands/src/registry.rs`:

```rust
/// Load hand definitions from a project directory's .openfang/hands/ folder.
/// Returns the number of hands successfully loaded.
/// Project-local hands are namespaced with "project:{dir}:" prefix to avoid
/// collisions with bundled hands and between multiple projects.
pub fn load_from_directory(&mut self, project_dir: &Path) -> usize {
    let hands_dir = project_dir.join(".openfang").join("hands");
    // For each subdirectory in hands_dir:
    //   1. Read HAND.toml
    //   2. Read SKILL.md (optional)
    //   3. Parse HandDefinition via toml::from_str()
    //   4. Manually attach skill_content (because it's #[serde(skip)])
    //   5. Validate: strip "shell_exec" from tools if present (security)
    //   6. Force mcp_servers = [], api_key_env = None (security)
    //   7. Validate hand ID does not contain ":" (reserved separator)
    //   8. Namespace the id: "project:{project_dir_name}:{original_id}"
    //   9. Insert into definitions HashMap
    //  10. If a bundled hand has the same base id, log a warning
}
```

**Key design decisions**:

- **Namespacing**: Project hands get `id = "project:{project-dir-name}:{original_id}"`.
  This prevents silent overwriting of bundled hands AND collisions between multiple
  projects. The original `id` field in HAND.toml is the base name (e.g., `id = "deploy"`),
  and the project dir name is the last component of the project path (e.g., `backend-api`),
  resulting in `project:backend-api:deploy` in the registry. The `:` character is
  reserved as a namespace separator — hand IDs in HAND.toml must not contain `:`.

- **`skill_content` handling**: Mirrors `parse_bundled()` -- read SKILL.md separately,
  attach to the parsed struct after deserialization. If SKILL.md is missing, that's
  fine (skill_content stays None).

- **Security restrictions for v1**:
  - Project-local hands CANNOT grant themselves `shell_exec` (stripped with warning log)
  - Project-local hands CANNOT specify `api_key_env` (force-nullified after parsing, uses kernel default only)
  - Project-local hands are NEVER auto-activated (appear in marketplace only; explicit activation = user consent)
  - MCP servers are **blocked entirely** for project-local hands in v1 (force `mcp_servers = []` after parsing, log warning)
  - `web_fetch` is **allowed** — since hands require explicit manual activation, the user's
    trust in activating a hand is the security boundary. Blocking `web_fetch` would cripple
    legitimate use cases (research hands, API hands, monitor hands). The existing SSRF
    protection (see `docs/security.md` §7) applies to all `web_fetch` calls regardless of source.

- **Filesystem**: `openfang-hands` currently uses zero `std::fs`. This adds it, but
  only in the new `load_from_directory()` method. The rest of the crate remains
  unchanged. Add `std::fs` and `std::path` imports, no new dependencies needed.

- **Error handling**: Individual hand parse failures are logged and skipped (same
  pattern as `load_bundled()`). A bad HAND.toml in one project directory doesn't
  block others.

### 3. Agent Template Loading: Project Templates

**Existing mechanism**: Templates are already discovered from 3 locations in
`openfang-cli/src/templates.rs`:
1. Repo `agents/` directory (walking up from binary)
2. `~/.openfang/agents/`
3. `OPENFANG_AGENTS_DIR` env var

**New addition**: Add project directories as a 4th discovery source.

The template system already returns `AgentManifest` structs. Project templates
are loaded as templates (not live agents), tagged with `source: "project"` metadata.

**Seed files**: When `spawn_agent()` creates a workspace, it currently generates
identity files (SOUL.md, USER.md, TOOLS.md, MEMORY.md) from scratch. The change:

```
If the agent was spawned from a project template at .openfang/agents/{name}/:
  1. Check if .openfang/agents/{name}/SOUL.md exists → copy to workspace
  2. Check if .openfang/agents/{name}/TOOLS.md exists → copy to workspace
  3. Check if .openfang/agents/{name}/skills/*.md exist → copy to workspace/skills/
  4. Still generate USER.md, MEMORY.md, AGENT.json fresh (user-specific)
```

This means the team controls the agent's personality and tooling docs, while
each developer gets their own conversation history and personal notes.

**No auto-spawn**: Project agent templates are NEVER auto-spawned. They appear
in the template picker (`openfang agent new`) with a `[project]` tag. The developer
explicitly chooses to spawn them. This avoids:
- Duplicate agents on restart (the FATAL issue from the critic review)
- Zombie agents that respawn after being killed
- State drift between TOML and SQLite
- Unwanted LLM API cost from auto-spawned agents

### 4. Agent Workspace Version Control

The workspace lives at `~/.openfang/workspaces/` (outside the project repo),
so it's inherently isolated from git. But teams want to share agent configurations
while keeping runtime state private.

**The split**:

| What | Where | In Git? | Who Controls |
|------|-------|---------|--------------|
| Agent manifest (model, tools, capabilities) | `.openfang/agents/{name}/agent.toml` | Yes | Team |
| Agent personality seed | `.openfang/agents/{name}/SOUL.md` | Yes | Team |
| Environment/tooling docs | `.openfang/agents/{name}/TOOLS.md` | Yes | Team |
| Agent-specific skills | `.openfang/agents/{name}/skills/` | Yes | Team |
| User profile (what agent learns about you) | `~/.openfang/workspaces/{name}-{id}/USER.md` | No | Agent |
| Long-term memory notes | `~/.openfang/workspaces/{name}-{id}/MEMORY.md` | No | Agent |
| Conversation history | `~/.openfang/workspaces/{name}-{id}/sessions/` | No | Agent |
| Runtime data/output | `~/.openfang/workspaces/{name}-{id}/data/,output/` | No | Agent |
| SQLite memory/KV state | `~/.openfang/data/openfang.db` | No | Kernel |

**Secrets never in git**: API keys stay in `~/.openfang/.env` (user-global) or
system environment variables. Project HAND.toml files reference keys by env var
name (`api_key_env = "DEPLOY_TOKEN"`), never by value.

### 5. Source Tagging (Deduplication Foundation)

To support future reconciliation (if we ever add auto-spawn or hand persistence),
add a `source` field to both `HandDefinition` and `AgentEntry`:

```rust
// In HandDefinition (openfang-hands/src/lib.rs)
#[serde(skip)]
pub source: HandSource,

#[derive(Debug, Clone, Default)]
pub enum HandSource {
    #[default]
    Bundled,
    Project { dir: PathBuf },
}

// In AgentEntry (openfang-types/src/agent.rs) -- add to tags for now
// Tags already exist: tags: Vec<String>
// Project agents get: tags = ["source:project", "project_dir:/path/to/repo"]
```

For v1, this is informational only (used for display in marketplace and template
picker). It lays the groundwork for future reconciliation without adding complexity now.

### 6. Security Model (v1)

**Threat**: A malicious repo includes `.openfang/hands/evil/HAND.toml` that:
- Grants itself `shell_exec` → could run arbitrary commands
- Sets a prompt to exfiltrate env vars → could steal API keys
- References attacker MCP servers → could phone home

**v1 mitigations** (pragmatic, no trust prompt yet):

1. **Never auto-activated (primary security boundary)**: Project hands appear in the
   marketplace but require explicit user action to activate. The developer must review
   the hand definition and consciously choose to activate it. This explicit consent is
   the primary security mechanism — the same trust model as installing any npm package
   or pip dependency from a repo.

2. **`shell_exec` stripped**: Project-local hands cannot grant themselves shell access.
   If HAND.toml lists `shell_exec` in tools, it's silently removed and a warning is logged.
   ```
   WARN: Project hand "project:backend-api:deploy" requested shell_exec -- stripped for security.
         To grant shell access, activate via API with explicit override.
   ```

3. **`api_key_env` force-nullified**: After parsing a project HAND.toml, `api_key_env`
   is explicitly set to `None` (not just "ignored" — actively cleared). This prevents a
   hand from tricking the system into reading arbitrary env vars.

4. **MCP servers blocked entirely**: In v1, project-local hands CANNOT reference MCP
   servers. After parsing, `mcp_servers` is forced to `[]`. MCP servers can execute
   arbitrary code on the host — they bypass the capability model entirely.
   ```
   WARN: Project hand "project:backend-api:deploy" references MCP servers: ["custom-server"].
         MCP servers are blocked for project-local hands in v1. Stripped.
   ```

5. **`web_fetch` allowed**: Since activation requires explicit user consent, `web_fetch`
   is permitted for project-local hands. The existing SSRF protection system (private IP
   blocking, DNS rebinding guard — see `docs/security.md` §7) applies to all `web_fetch`
   calls regardless of hand source. Blocking `web_fetch` would cripple legitimate use
   cases like research hands, API integration hands, and monitoring hands.

6. **Path traversal prevention**: Hand/agent directory scanning rejects symlinks that
   escape the `.openfang/` directory and paths containing `..`.

7. **CWD auto-detection warning**: If `.openfang/` exists in the current working
   directory but is not in the loaded project set, log a prominent notice:
   ```
   NOTICE: Found .openfang/ in CWD (/home/user/backend-api) but it is not registered.
           Use `openfang start --project .` to load project-local definitions.
   ```
   This prevents the silent failure mode where a developer forgets `--project`.

**Audit logging**: When a project-local hand uses `web_fetch`, log the URL at INFO
level for auditability:
```
INFO: Project hand "project:backend-api:deploy" used web_fetch → https://api.example.com/deploy
```
This gives operators visibility without blocking legitimate use.

**Future enhancement** (not in v1): A workspace trust prompt similar to VS Code:
"This project wants to register 2 hands and 1 agent template. Allow? [y/N]"
with a hash-based allowlist in `~/.openfang/trusted_projects.toml`.

---

## Implementation Plan

### Phase 1: Config & Discovery (openfang-types, openfang-kernel)

1. Add `project_dirs: Vec<PathBuf>` to `KernelConfig` with `#[serde(default)]`
2. Add `project_dirs` to `KernelConfig::default()` (empty vec)
3. Add `--project` flag to `StartArgs` in openfang-cli (just the arg definition)
4. In kernel `boot_with_config()`, resolve project directories (CLI > config > CWD)
5. Validate each: exists, contains `.openfang/`, is readable
6. If CWD has `.openfang/` but is not in the resolved set, log a prominent NOTICE

### Phase 2: Hand Loading (openfang-hands)

7. Add `HandSource` enum to `lib.rs`
8. Add `source: HandSource` field to `HandDefinition` with `#[serde(skip)]`
9. Implement `HandRegistry::load_from_directory(project_dir: &Path) -> usize`
   - Walk `.openfang/hands/*/HAND.toml`
   - Parse TOML, attach SKILL.md (mirror `parse_bundled()` pattern)
   - Namespace id with `project:{dir}:` prefix
   - Strip `shell_exec`, force-nullify `api_key_env`, block MCP servers
   - Validate hand ID has no `:`
   - Insert into definitions HashMap
10. Call `load_from_directory()` in kernel boot, after `load_bundled()`

### Phase 3: Agent Templates (openfang-kernel)

11. In template discovery, add project `.openfang/agents/` directories as a source
12. Tag project templates with `source:project` metadata
13. In `spawn_agent()`, if template has a project source directory:
    - Copy SOUL.md from project template to workspace (if exists)
    - Copy TOOLS.md from project template to workspace (if exists)
    - Copy skills/ from project template to workspace (if exists)
    - Generate USER.md, MEMORY.md fresh (user-specific)

### Phase 4: .gitignore Generation

14. When loading from a project directory, check for `.openfang/.gitignore`
15. If missing, log a warning:
    ```
    WARN: Project .openfang/ has no .gitignore. Runtime state may leak into git.
          Run `openfang init --project` to generate one, or create it manually.
    ```
16. Add `openfang init --project` subcommand that creates `.openfang/.gitignore`
    with the standard patterns, plus scaffolds `hands/` and `agents/` dirs with
    example files

### Phase 5: API & Display

17. Add `source` field to hand list API response (for UI "[project]" tags)
18. Add `source` field to template list (for CLI `[project]` display)

### Phase 6: Tests

19. Unit tests: TOML parsing for project-local hands (string input, no filesystem)
20. Unit tests: namespace collision handling, security stripping (shell_exec, MCP, api_key_env)
21. Unit tests: hand ID validation (reject `:` in HAND.toml id field)
22. Integration tests: tempdir with `.openfang/hands/` structure → boot kernel → verify hands appear
23. Integration tests: tempdir with `.openfang/agents/` → spawn agent → verify seed files copied
24. Integration tests: multi-project namespace isolation (two projects with same hand id)

---

## What This Plan Does NOT Include (Explicitly Deferred)

| Feature | Why Deferred |
|---------|-------------|
| Auto-spawn of project agents | Creates duplicate/zombie/drift problems (FATAL from review) |
| Hand persistence across restarts | Hands are ephemeral by design; persistence needs a reconciliation model |
| Hot reload of project files | Nice-to-have but not blocking; config_reload.rs exists as foundation |
| Workspace trust prompt | Security enhancement for v2; v1 mitigations are sufficient |
| Agent state export back to project | Useful but complex; needs a `openfang agent export` command design |
| Auto-activation of project hands | Security risk; explicit activation is the consent mechanism |

---

## Files Modified

| File | Change |
|------|--------|
| `crates/openfang-types/src/config.rs` | Add `project_dirs: Vec<PathBuf>` field + Default |
| `crates/openfang-hands/src/lib.rs` | Add `HandSource` enum, `source` field on `HandDefinition` |
| `crates/openfang-hands/src/registry.rs` | Add `load_from_directory()` method |
| `crates/openfang-kernel/src/kernel.rs` | Call `load_from_directory()` in boot, seed files in `spawn_agent()` |
| `crates/openfang-cli/src/main.rs` | Add `--project` to `StartArgs` (arg only, not CLI internals) |
| `crates/openfang-cli/src/templates.rs` | Add project dirs to template discovery |
| `crates/openfang-api/src/routes.rs` | Add `source` to hand list response |

No new crates. No new dependencies (std::fs is sufficient for directory walking).

## Documentation Updates Required

| Document | Update Needed |
|----------|--------------|
| `docs/agent-templates.md` | Document SOUL.md, TOOLS.md, and per-agent `skills/` directory convention for project-local agent templates |
| `docs/configuration.md` | Add `project_dirs` field to the config reference |
| `docs/architecture.md` | Add a section on project-local discovery in the system architecture overview |
