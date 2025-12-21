# Code Coverage Testing Guide

## Overview

This document describes how to perform code coverage testing in the SPEAR Next project. Code coverage is an important metric for measuring test quality and helps us understand how well our code is tested.

## Tool Selection

We use `cargo-tarpaulin` as the primary code coverage tool, which is the most popular and powerful coverage tool in the Rust ecosystem.

### Installing cargo-tarpaulin

```bash
cargo install cargo-tarpaulin
```

## Configuration Files

### tarpaulin.toml

The `tarpaulin.toml` file in the project root contains the coverage testing configuration:

```toml
[report]
# Output formats: HTML report, LCOV format, JSON format
out = ["Html", "Lcov", "Json"]

# Output directory
output-dir = "target/coverage"

# Excluded files and directories
exclude = [
    "proto/*",           # Generated protobuf files
    "tests/*",           # Test files themselves
    "src/main.rs",       # Main entry point
    "src/bin/*",         # Binary entry points
    "examples/*",        # Example code
    "benches/*",         # Benchmark tests
]

# Included files and directories
include = [
    "src/config/*",
    "src/services/*",
    "src/sms/*",
    "src/spearlet/*",
    "src/storage/*",
    "src/utils/*",
    "src/lib.rs",
]

# Minimum coverage threshold
fail-under = 70

# Timeout setting (seconds)
timeout = 300

# Number of parallel test threads
jobs = 4

# Verbose output
verbose = true

# Skip cleanup
no-clean = false
```

## Usage

### 1. Quick Coverage Testing

Use the provided quick script:

```bash
# Run quick coverage test
./scripts/quick-coverage.sh

# Or use Makefile
make coverage-quick
```

### 2. Full Coverage Testing

Run complete coverage analysis (including all features):

```bash
# Run full coverage test
./scripts/coverage.sh

# Or use Makefile
make coverage
```

### 3. Manual Execution

Use cargo-tarpaulin directly:

```bash
# Basic coverage test
cargo tarpaulin --config tarpaulin.toml

# Specify features
cargo tarpaulin --features sled --config tarpaulin.toml

# All features
cargo tarpaulin --all-features --config tarpaulin.toml
```

## Output Formats

### HTML Report

- Location: `target/coverage/tarpaulin-report.html`
- Provides detailed visual coverage reports
- View line-level coverage for each file

### LCOV Format

- Location: `target/coverage/lcov.info`
- Suitable for CI/CD integration and third-party tools

### JSON Format

- Location: `target/coverage/tarpaulin-report.json`
- Machine-readable format for automated processing

## Makefile Targets

The project provides the following Makefile targets:

```bash
# Quick coverage test (default features)
make coverage-quick

# Full coverage test (all features)
make coverage

# Clean coverage data
make clean-coverage
```

## Script Descriptions

### scripts/quick-coverage.sh

Quick coverage testing script:
- Check cargo-tarpaulin installation
- Clean old coverage data
- Run coverage test with default features
- Generate HTML and console output
- Automatically open browser to view report

### scripts/coverage.sh

Full coverage testing script:
- Support multiple feature combination testing
- Generate detailed coverage reports
- Include error handling and logging
- Generate summary reports

## Coverage Goals

### Current Settings

- **Minimum coverage threshold**: 70%
- **Target coverage**: 80%+
- **Critical module coverage**: 90%+

### Coverage Categories

1. **Core Business Logic**: Requires 90%+ coverage
   - SMS services
   - Configuration management
   - Storage layer

2. **Utilities and Helper Modules**: Requires 80%+ coverage
   - Utility functions
   - Middleware

3. **Integration and Interface Layer**: Requires 70%+ coverage
   - HTTP routes
   - gRPC services

## Best Practices

### 1. Regular Execution

- Run quick coverage tests before each commit
- Run full coverage analysis weekly
- Integrate coverage checks in CI/CD pipelines

### 2. Focus on Quality over Quantity

- Focus on coverage of critical business logic
- Ensure testing of boundary conditions and error handling
- Avoid writing meaningless tests just for coverage

### 3. Analyze Reports

- Regularly review HTML reports to identify uncovered code
- Monitor coverage trend changes
- Develop improvement plans for low-coverage modules

## Troubleshooting

### Common Issues

1. **Compilation Errors**
   ```bash
   # Ensure project compiles normally
   cargo check
   cargo test
   ```

2. **Permission Issues**
   ```bash
   # Ensure scripts have execute permissions
   chmod +x scripts/*.sh
   ```

3. **Dependency Issues**
   ```bash
   # Reinstall cargo-tarpaulin
   cargo install --force cargo-tarpaulin
   ```

### Performance Optimization

- Use `--jobs` parameter to adjust parallelism
- Exclude unnecessary files and directories
- Use `--skip-clean` to skip cleanup (when debugging)

## CI/CD Integration

Refer to the `.github/workflows/coverage.yml` file to understand how to integrate code coverage testing in GitHub Actions.

## Related Resources

- [cargo-tarpaulin Official Documentation](https://github.com/xd009642/tarpaulin)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Code Coverage Best Practices](https://martinfowler.com/bliki/TestCoverage.html)