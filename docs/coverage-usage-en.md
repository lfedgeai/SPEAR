# Code Coverage Analysis Usage Guide

## Overview / 概述

This document describes how to use code coverage analysis tools in the spear-next project. The project uses `cargo-tarpaulin` for code coverage analysis, supporting multiple output formats and configuration options.

## Available Commands / 可用命令

### 1. Standard Coverage Analysis
```bash
make coverage
```
- Runs complete code coverage analysis
- Includes all test configurations (default, sled, rocksdb, all features)
- Checks coverage threshold (currently set to 35%)
- Generates HTML, LCOV, and JSON format reports

### 2. Quick Coverage Analysis
```bash
make coverage-quick
```
- Runs quick coverage analysis
- Uses default configuration only
- Suitable for quick coverage checks
- Automatically opens HTML report

### 3. No-Fail Coverage Analysis
```bash
make coverage-no-fail
```
- Runs coverage analysis without checking failure threshold
- Won't fail even if coverage is below threshold
- Suitable for information gathering in CI/CD environments

### 4. Open Coverage Report
```bash
make coverage-open
```
- Opens the latest HTML coverage report in browser

## Configuration / 配置文件

### tarpaulin.toml
The project's coverage configuration file is located at `tarpaulin.toml`, containing the following main configurations:

- **Output Formats**: HTML, LCOV, JSON
- **Output Directory**: `target/coverage`
- **Coverage Threshold**: 35% (adjustable)
- **Excluded Files**: proto files, build scripts, test files, etc.
- **Included Files**: main source code directories

### Key Configuration Items
```toml
# Coverage threshold
fail-under = 35

# Output formats
out = ["Html", "Lcov", "Json"]

# Output directory
output-dir = "target/coverage"

# Excluded files
exclude = [
    "src/proto/*",
    "build.rs",
    "tests/*",
    "benches/*",
    "src/bin/*",
]
```

## Report Files / 报告文件

After coverage analysis completes, the following files will be generated in the `target/coverage/` directory:

- `tarpaulin-report.html` - HTML format report (recommended for viewing)
- `lcov.info` - LCOV format report
- `tarpaulin-report.json` - JSON format report

## Troubleshooting / 故障排除

### Common Issues

1. **cargo-audit Installation Failure**
   - Cause: Rust version incompatible with cargo-audit
   - Solution: Error handling added to Makefile, doesn't affect coverage analysis

2. **Coverage Below Threshold**
   - Use `make coverage-no-fail` to skip threshold check
   - Or adjust `fail-under` value in `tarpaulin.toml`

3. **Test Timeout**
   - Adjust `timeout` value in `tarpaulin.toml`
   - Currently set to 120 seconds

### Requirements

- Rust toolchain
- cargo-tarpaulin (automatically installed)
- Project dependencies properly installed

## Best Practices / 最佳实践

1. **Regular Coverage Analysis**
   - Run `make coverage-quick` before committing code
   - Use `make coverage-no-fail` in CI/CD

2. **Monitor Coverage Trends**
   - Track coverage changes
   - Write tests for new features

3. **Set Reasonable Thresholds**
   - Adjust thresholds based on project reality
   - Gradually increase coverage requirements

## Related Files / 相关文件

- `Makefile` - Contains all coverage-related commands
- `tarpaulin.toml` - Coverage configuration file
- `scripts/coverage.sh` - Coverage analysis script
- `scripts/quick-coverage.sh` - Quick coverage analysis script

## Update History / 更新历史

- 2024-01-XX: Fixed cargo-audit compatibility issues
- 2024-01-XX: Added coverage-no-fail target
- 2024-01-XX: Adjusted coverage threshold to 35%