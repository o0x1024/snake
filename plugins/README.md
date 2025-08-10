# Aurora Plugin System

## Overview

The Aurora Plugin System provides a framework for extending the security assessment platform with custom functionality. The system is designed with security and isolation in mind, supporting hot-reloading and capability-based access control.

## Architecture

### Core Components

1. **PluginRuntime** - Manages plugin lifecycle and execution
2. **PluginLoader** - Handles loading plugins from directories or bytes
3. **PluginApi** - High-level API for plugin operations
4. **HostFunctions** - Functions that plugins can call

### Plugin Framework Features

- ✅ Plugin loading and unloading
- ✅ Hot reload mechanism with file watching
- ✅ Capability-based security model
- ✅ Plugin execution statistics
- ✅ Host function interface
- 🔄 WASM runtime (framework ready, implementation pending)

## Plugin Structure

Each plugin should be organized in its own directory:

```
plugins/
├── vulnerability_scanner/
│   ├── manifest.json
│   └── plugin.wasm
├── password_cracker/
│   ├── manifest.json
│   └── plugin.wasm
└── plugin_template/
    ├── Cargo.toml
    ├── src/lib.rs
    └── build.sh
```

### Manifest Format

```json
{
  "name": "plugin_name",
  "version": "1.0.0",
  "description": "Plugin description",
  "author": "Author Name",
  "entry_point": "plugin.wasm",
  "permissions": [
    "network.http",
    "filesystem.read",
    "crypto.encrypt"
  ],
  "dependencies": [],
  "capabilities": {
    "network_access": true,
    "filesystem_access": true,
    "crypto_access": true,
    "system_access": false,
    "memory_limit_mb": 128,
    "execution_timeout_ms": 60000
  },
  "hot_reload": true
}
```

## Available Permissions

- `network.http` - HTTP/HTTPS network access
- `filesystem.read` - Read filesystem access
- `filesystem.write` - Write filesystem access
- `crypto.encrypt` - Encryption operations
- `crypto.decrypt` - Decryption operations
- `system.execute` - System command execution

## Plugin Development

### Template Plugin

A template plugin is provided in `plugins/plugin_template/` that demonstrates:

- Basic plugin structure
- Host function usage
- Memory management
- Function exports

### Host Functions

Plugins can call these host functions:

- `log(ptr, len)` - Log messages to Aurora
- `get_timestamp()` - Get current Unix timestamp
- `alloc(size)` - Allocate memory
- `dealloc(ptr, size)` - Deallocate memory

### Building Plugins

1. Use the template as a starting point
2. Implement your plugin logic in Rust
3. Build with `cargo build --target wasm32-unknown-unknown --release`
4. Copy the WASM file to your plugin directory

## API Usage

### Loading a Plugin

```rust
let api = PluginApi::new("./plugins".to_string())?;
api.load_plugin_from_directory("vulnerability_scanner").await?;
```

### Executing Plugin Functions

```rust
let request = PluginRequest {
    plugin_name: "vulnerability_scanner".to_string(),
    function_name: "scan_target".to_string(),
    parameters: HashMap::new(),
};

let response = api.execute_plugin(request).await?;
```

### Hot Reload

```rust
api.enable_hot_reload("vulnerability_scanner").await?;
```

## Security Model

### Capabilities

Each plugin declares its required capabilities in the manifest. The runtime enforces these at execution time:

- **Network Access**: Controls HTTP/HTTPS requests
- **Filesystem Access**: Controls file read/write operations
- **Crypto Access**: Controls cryptographic operations
- **System Access**: Controls system command execution
- **Memory Limits**: Enforces maximum memory usage
- **Execution Timeouts**: Prevents runaway plugins

### Isolation

- Plugins run in isolated environments
- Memory access is controlled
- System calls are mediated through host functions
- Network access is capability-gated

## Current Status

The plugin framework is implemented and ready for use. The WASM runtime integration is prepared but uses a simplified execution model for now. Full WASM support can be added by:

1. Implementing proper WASM memory management
2. Adding WASI support for filesystem operations
3. Implementing secure inter-plugin communication
4. Adding plugin signing and verification

## Example Plugins

### Vulnerability Scanner

Scans targets for known vulnerabilities using CVE databases.

### Password Cracker

High-performance password cracking with wordlist support.

### Network Scanner

Port scanning and service detection capabilities.

## Future Enhancements

- Full WASM runtime with wasmer
- Plugin marketplace and distribution
- Advanced inter-plugin communication
- Plugin signing and verification
- Performance monitoring and profiling
- Plugin dependency management