# Plugin System

The plugin system provides extensibility through well-defined trait interfaces, allowing the community to extend Canaveral's functionality without modifying core code.

## Plugin Architecture

Canaveral supports external subprocess plugins that communicate via JSON over stdin/stdout. Plugins are standalone executables that implement a simple request/response protocol.

## Plugin Types

Three plugin types are supported:

| Type | Purpose |
|------|---------|
| `adapter` | Custom package manager adapters |
| `strategy` | Custom version calculation strategies |
| `formatter` | Custom changelog formatters |

## Subprocess Protocol

Plugins communicate with Canaveral via JSON messages over stdin/stdout:

**Request (sent to plugin via stdin):**
```json
{
  "action": "detect",
  "input": {"path": "/path/to/project"},
  "config": {"key": "value"}
}
```

**Response (received from plugin via stdout):**
```json
{
  "output": true,
  "error": null
}
```

Standard actions depend on plugin type:
- **Adapter plugins:** `info`, `detect`, `get_version`, `set_version`, `publish`
- **Strategy plugins:** `info`, `parse`, `format`, `bump`
- **Formatter plugins:** `info`, `format`

## Plugin Traits

### Adapter Plugin Interface

```rust
pub trait AdapterPlugin {
    /// Get adapter name
    fn name(&self) -> &str;

    /// Detect if this adapter applies to a path
    fn detect(&self, path: &Path) -> bool;

    /// Get package version
    fn get_version(&self, path: &Path) -> Result<String>;

    /// Set package version
    fn set_version(&self, path: &Path, version: &str) -> Result<()>;

    /// Publish package
    fn publish(&self, path: &Path, options: &serde_json::Value) -> Result<()>;
}
```

### Strategy Plugin Interface

```rust
pub trait StrategyPlugin {
    /// Get strategy name
    fn name(&self) -> &str;

    /// Parse a version string
    fn parse(&self, version: &str) -> Result<serde_json::Value>;

    /// Format version components
    fn format(&self, components: &serde_json::Value) -> Result<String>;

    /// Calculate next version
    fn bump(&self, current: &str, bump_type: &str) -> Result<String>;
}
```

## Plugin Metadata

Each plugin provides metadata via the `info` action:

```rust
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub plugin_type: PluginType,  // adapter, strategy, or formatter
    pub description: Option<String>,
    pub author: Option<String>,
    pub capabilities: Vec<String>,
}
```

## Plugin Configuration

Plugins are configured in `canaveral.yaml`:

```yaml
plugins:
  - name: my-custom-adapter
    plugin_type: adapter
    command: /usr/local/bin/my-adapter-plugin
    enabled: true
    config:
      registry_url: "https://custom-registry.example.com"
      timeout: 30

  - name: my-strategy
    plugin_type: strategy
    path: ./plugins/my-strategy
    config:
      format: "YYYY.MM.BUILD"
```

### Plugin Configuration Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Plugin name |
| `plugin_type` | string | Yes | `adapter`, `strategy`, or `formatter` |
| `path` | string | No | Path to plugin executable |
| `command` | string | No | Command to execute (alternative to path) |
| `config` | map | No | Plugin-specific configuration (passed in JSON request) |
| `enabled` | bool | `true` | Whether the plugin is active |

Either `path` or `command` must be specified.

## Creating a Plugin

### Example: Custom Adapter (Python)

```python
#!/usr/bin/env python3
import json
import sys

def handle_request(request):
    action = request["action"]
    input_data = request.get("input", {})
    config = request.get("config", {})

    if action == "info":
        return {
            "output": {
                "name": "my-adapter",
                "version": "1.0.0",
                "plugin_type": "adapter",
                "description": "Custom adapter for my registry",
                "capabilities": ["detect", "get_version", "set_version", "publish"]
            }
        }
    elif action == "detect":
        path = input_data.get("path", "")
        # Check if my-manifest.json exists
        import os
        exists = os.path.exists(os.path.join(path, "my-manifest.json"))
        return {"output": exists}
    elif action == "get_version":
        path = input_data.get("path", "")
        import os
        manifest = json.load(open(os.path.join(path, "my-manifest.json")))
        return {"output": manifest.get("version", "0.0.0")}
    else:
        return {"error": f"Unknown action: {action}"}

# Read request from stdin
request = json.load(sys.stdin)
response = handle_request(request)
print(json.dumps(response))
```

### Example: Custom Strategy (Rust)

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read};

#[derive(Deserialize)]
struct PluginRequest {
    action: String,
    input: serde_json::Value,
    config: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct PluginResponse {
    output: Option<serde_json::Value>,
    error: Option<String>,
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let request: PluginRequest = serde_json::from_str(&input).unwrap();
    let response = handle_request(&request);

    println!("{}", serde_json::to_string(&response).unwrap());
}

fn handle_request(request: &PluginRequest) -> PluginResponse {
    match request.action.as_str() {
        "info" => PluginResponse {
            output: Some(serde_json::json!({
                "name": "my-strategy",
                "version": "1.0.0",
                "plugin_type": "strategy",
                "capabilities": ["parse", "format", "bump"]
            })),
            error: None,
        },
        "bump" => {
            let current = request.input.get("current")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0");
            let bump_type = request.input.get("bump_type")
                .and_then(|v| v.as_str())
                .unwrap_or("patch");
            // Custom bump logic here
            PluginResponse {
                output: Some(serde_json::json!(format!("{}.1", current))),
                error: None,
            }
        }
        _ => PluginResponse {
            output: None,
            error: Some(format!("Unknown action: {}", request.action)),
        },
    }
}
```

## Plugin Discovery

Plugins are discovered through multiple mechanisms:

### 1. Configuration
Explicitly defined in `canaveral.yaml` (see above).

### 2. Search Paths
The plugin registry can search directories for executable plugins:
- Plugins matching `canaveral-plugin-*` in search paths
- Executable files with `.so`, `.dylib`, `.dll`, or `.exe` extensions

### 3. Built-in Plugins
All built-in adapters, strategies, and formatters are compiled into the binary and always available.

## Plugin Lifecycle

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  Load    │────>│  Query   │────>│ Register │
│  Config  │     │  Info    │     │ in       │
│          │     │ (stdin)  │     │ Registry │
└──────────┘     └──────────┘     └──────────┘
                                       │
                                       v
                                 ┌──────────┐
                                 │ Execute  │
                                 │ Actions  │
                                 │(stdin/out)│
                                 └──────────┘
```

1. Plugin configs are loaded from `canaveral.yaml`
2. Each plugin is queried for its info via the `info` action
3. Plugins are registered in the `PluginRegistry` by type and name
4. During execution, plugins are invoked via their `execute` method

## Security Considerations

- Plugins run as separate processes with the same permissions as the calling user
- Plugin configuration can include arbitrary key-value pairs passed via JSON
- Only install plugins from trusted sources
- Review plugin code before use in CI/CD environments
- Use lock files to pin plugin versions where possible
