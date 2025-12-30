# SPEARlet Default KV Backend Configuration Change

## Overview

This document records the change made to SPEARlet's default KV backend configuration from RocksDB to Memory storage.

## Change Summary

**Date**: 2024-01-15  
**Component**: SPEARlet Configuration  
**Change Type**: Default Configuration Update  

### Before
- **Default KV Backend**: `rocksdb`
- **Configuration Location**: `src/spearlet/config.rs` - `StorageConfig::default()`

### After
- **Default KV Backend**: `memory`
- **Configuration Location**: `src/spearlet/config.rs` - `StorageConfig::default()`

## Rationale

The change was made to align the default configuration with the actual usage patterns observed in the codebase:

1. **Test Environment Consistency**: All test cases were already using memory backend explicitly
2. **Development Convenience**: Memory backend provides faster startup and easier debugging
3. **Simplified Setup**: No need for persistent storage setup during development
4. **Resource Efficiency**: Lower resource requirements for development and testing

## Impact Analysis

### SPEARlet Service
- ✅ **Default behavior changed**: New SPEARlet instances will use memory storage by default
- ✅ **Backward compatibility**: Existing configurations specifying `rocksdb` will continue to work
- ✅ **CLI override**: Users can still specify `--storage-backend rocksdb` for persistent storage

### SMS Service
- ✅ **No impact**: SMS continues to use `sled` as default database backend
- ✅ **Independent configuration**: SMS database configuration remains unchanged

## Files Modified

1. **`src/spearlet/config.rs`**
   - Changed `StorageConfig::default()` backend from `"rocksdb"` to `"memory"`
   - Added explanatory comment

2. **`src/spearlet/config_test.rs`**
   - Updated test expectations to match new default
   - Modified `test_spearlet_config_default()` and `test_storage_config_default()`

## Testing

All tests pass successfully:
- ✅ SPEARlet tests: 71 passed, 0 failed
- ✅ SMS tests: 101 passed, 0 failed
- ✅ Configuration tests updated and passing

## Migration Guide

### For Existing Users
If you need persistent storage (RocksDB), you have several options:

1. **Command Line**: `spearlet --storage-backend rocksdb`
2. **Configuration File**: Set `storage.backend = "rocksdb"` in your TOML config
3. **Environment Variable**: Set `STORAGE_BACKEND=rocksdb`

### For New Users
- Default memory storage works out of the box
- No additional setup required for development
- Data will not persist between restarts (by design)

## Configuration Options

SPEARlet supports multiple KV backends:
- `memory`: In-memory storage (new default)
- `rocksdb`: Persistent RocksDB storage
- `sled`: Persistent Sled storage (if enabled)

## Best Practices

1. **Development**: Use default memory backend for fast iteration
2. **Testing**: Memory backend is ideal for unit and integration tests
3. **Production**: Consider using `rocksdb` for persistent storage needs
4. **Configuration**: Always specify backend explicitly in production configs

## Related Documentation

- [SPEARlet KV Backend Analysis](./spearlet-kv-backend-analysis-en.md)
- [Configuration Guide](./README-en.md)