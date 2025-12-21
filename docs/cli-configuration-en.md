# CLI Configuration Guide

## Overview

The SPEAR Node Service supports flexible configuration through command-line arguments, configuration files, and environment variables. This guide covers all available configuration options and their usage.

## Command Line Arguments

### Basic Usage

```bash
cargo run --bin sms -- [OPTIONS]
```

### Available Options

| Option | Type | Description | Default |
|--------|------|-------------|---------|
| `--config <FILE>` | String | Path to configuration file | None |
| `--grpc-addr <ADDR>` | String | gRPC server address | 0.0.0.0:50051 |
| `--http-addr <ADDR>` | String | HTTP gateway address | 0.0.0.0:8080 |
| `--heartbeat-timeout <SECONDS>` | u64 | Node heartbeat timeout | 120 |
| `--cleanup-interval <SECONDS>` | u64 | Node cleanup interval | 300 |
| `--enable-swagger` | Flag | Enable Swagger UI | false |
| `--log-level <LEVEL>` | String | Log level (trace, debug, info, warn, error) | info |
| `--storage-backend <BACKEND>` | String | KV storage backend (memory, sled, rocksdb) | memory |
| `--storage-path <PATH>` | String | Storage path for file-based backends | None |

### Storage Backend Configuration

#### Memory Backend (Default)
```bash
cargo run --bin sms -- --storage-backend memory
```

#### Sled Backend
```bash
# Enable sled feature and specify backend
cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./data/sled.db
```

#### RocksDB Backend
```bash
# Enable rocksdb feature and specify backend
cargo run --features rocksdb --bin sms -- --storage-backend rocksdb --storage-path ./data/rocksdb
```

## Configuration Files

### File Format

Configuration files use TOML format. You can specify a configuration file using the `--config` option.

### Example Configuration Files

#### Basic Configuration (config.toml)
```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "memory"
```

#### Sled Backend Configuration (config-sled.toml)
```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "sled"

[sms.kv_store.params]
path = "./data/node-service.db"
cache_capacity = "10000"
compression = "true"
flush_every_ms = "5000"
```

#### RocksDB Backend Configuration (config-rocksdb.toml)
```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "rocksdb"

[sms.kv_store.params]
path = "./data/rocksdb-node-service"
cache_size = "67108864"  # 64MB
write_buffer_size = "16777216"  # 16MB
max_write_buffer_number = "3"
compression = "snappy"
```

### Loading Configuration Files
```bash
cargo run --bin sms -- --config config.toml
cargo run --features sled --bin sms -- --config config-sled.toml
cargo run --features rocksdb --bin sms -- --config config-rocksdb.toml
```

## Environment Variables

### KV Store Configuration via Environment

```bash
# Set backend type
export KV_STORE_BACKEND=sled

# Set storage path
export KV_STORE_PATH=./data/env-sled.db

# Set additional parameters
export KV_STORE_CACHE_SIZE=5000
export KV_STORE_COMPRESSION=true

# Run the application
cargo run --features sled --bin sms
```

### Supported Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `KV_STORE_BACKEND` | Storage backend type | `memory`, `sled`, `rocksdb` |
| `KV_STORE_PATH` | Storage path | `./data/storage.db` |
| `KV_STORE_*` | Additional parameters | `KV_STORE_CACHE_SIZE=1000` |

## Configuration Priority

Configuration values are applied in the following order (highest to lowest priority):

1. **Command line arguments** - Override all other sources
2. **Configuration file** - Specified via `--config` option
3. **Environment variables** - System environment settings
4. **Default values** - Built-in defaults

## Examples

### Development Setup
```bash
# Quick start with memory backend
cargo run --bin sms

# Development with file persistence
cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./dev-data.db
```

### Production Setup
```bash
# Using configuration file
cargo run --features sled --bin sms -- --config production-config.toml

# With custom addresses
cargo run --features sled --bin sms -- \
  --config production-config.toml \
  --grpc-addr 0.0.0.0:9090 \
  --http-addr 0.0.0.0:8090
```

### Testing Different Backends
```bash
# Test memory backend
cargo run --bin sms -- --storage-backend memory

# Test sled backend
cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./test-sled.db

# Test rocksdb backend (if dependencies are available)
cargo run --features rocksdb --bin sms -- --storage-backend rocksdb --storage-path ./test-rocksdb
```

## Troubleshooting

### Common Issues

1. **Feature not enabled**: Ensure you enable the required feature for your storage backend
   ```bash
   # Wrong: cargo run --bin sms -- --storage-backend sled
   # Correct: cargo run --features sled --bin sms -- --storage-backend sled
   ```

2. **Missing storage path**: File-based backends require a storage path
   ```bash
   cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./data.db
   ```

3. **Configuration file not found**: Ensure the configuration file exists
   ```bash
   # Check if file exists
   ls -la config.toml
   ```

### Validation

The application validates configuration at startup and will report errors for:
- Unsupported storage backends
- Invalid file paths
- Malformed configuration files
- Missing required parameters

## Best Practices

1. **Use configuration files for production** - More maintainable than command line arguments
2. **Enable appropriate features** - Only compile with needed storage backends
3. **Set appropriate timeouts** - Adjust heartbeat and cleanup intervals based on your environment
4. **Use environment variables for secrets** - Don't put sensitive data in configuration files
5. **Test configuration changes** - Validate configuration before deploying to production