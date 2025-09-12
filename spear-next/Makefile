# SPEAR Next Makefile / SPEAR Next构建文件
# This Makefile provides targets for building, testing, and analyzing the SPEAR Next project
# 此Makefile为SPEAR Next项目提供构建、测试和分析目标

# Project configuration / 项目配置
PROJECT_NAME := spear-next
VERSION := $(shell git describe --tags --match "*" --always --dirty 2>/dev/null || echo "dev")
REPO_ROOT := $(shell pwd)
TARGET_DIR := $(REPO_ROOT)/target
COVERAGE_DIR := $(TARGET_DIR)/coverage

# Rust configuration / Rust配置
CARGO := cargo
RUSTC_VERSION := $(shell rustc --version 2>/dev/null || echo "unknown")

# Colors for output / 输出颜色
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[1;33m
BLUE := \033[0;34m
NC := \033[0m # No Color

.PHONY: all build test clean coverage coverage-quick install-deps format lint check doc help

# Default target / 默认目标
all: check test build

# Display help information / 显示帮助信息
help:
	@echo -e "$(BLUE)SPEAR Next Build System / SPEAR Next构建系统$(NC)"
	@echo "=================================================="
	@echo ""
	@echo -e "$(GREEN)Available targets / 可用目标:$(NC)"
	@echo "  build           - Build the project / 构建项目"
	@echo "  test            - Run all tests / 运行所有测试"
	@echo "  coverage        - Run comprehensive code coverage analysis / 运行全面代码覆盖率分析"
	@echo "  coverage-quick  - Run quick code coverage analysis / 运行快速代码覆盖率分析"
	@echo "  clean           - Clean build artifacts / 清理构建产物"
	@echo "  format          - Format code / 格式化代码"
	@echo "  lint            - Run linter / 运行代码检查"
	@echo "  check           - Run cargo check / 运行cargo检查"
	@echo "  doc             - Generate documentation / 生成文档"
	@echo "  install-deps    - Install development dependencies / 安装开发依赖"
	@echo "  help            - Show this help message / 显示此帮助信息"
	@echo ""
	@echo -e "$(YELLOW)Examples / 示例:$(NC)"
	@echo "  make build                    # Build with default features / 使用默认特性构建"
	@echo "  make test                     # Run all tests / 运行所有测试"
	@echo "  make coverage-quick           # Quick coverage analysis / 快速覆盖率分析"
	@echo "  make FEATURES=sled build      # Build with sled feature / 使用sled特性构建"
	@echo ""

# Install development dependencies / 安装开发依赖
install-deps:
	@echo -e "$(BLUE)📦 Installing development dependencies... / 安装开发依赖...$(NC)"
	@if ! command -v cargo-tarpaulin >/dev/null 2>&1; then \
		echo -e "$(YELLOW)Installing cargo-tarpaulin... / 安装cargo-tarpaulin...$(NC)"; \
		$(CARGO) install cargo-tarpaulin; \
	fi
	@if ! command -v cargo-audit >/dev/null 2>&1; then \
		echo -e "$(YELLOW)Installing cargo-audit... / 安装cargo-audit...$(NC)"; \
		$(CARGO) install cargo-audit; \
	fi
	@if ! command -v cargo-outdated >/dev/null 2>&1; then \
		echo -e "$(YELLOW)Installing cargo-outdated... / 安装cargo-outdated...$(NC)"; \
		$(CARGO) install cargo-outdated; \
	fi
	@echo -e "$(GREEN)✅ Development dependencies installed / 开发依赖安装完成$(NC)"

# Build the project / 构建项目
build:
	@echo -e "$(BLUE)🔨 Building $(PROJECT_NAME)... / 构建$(PROJECT_NAME)...$(NC)"
	@if [ -n "$(FEATURES)" ]; then \
		echo -e "$(YELLOW)Building with features: $(FEATURES) / 使用特性构建: $(FEATURES)$(NC)"; \
		$(CARGO) build --features $(FEATURES); \
	else \
		$(CARGO) build; \
	fi
	@echo -e "$(GREEN)✅ Build completed / 构建完成$(NC)"

# Build release version / 构建发布版本
build-release:
	@echo -e "$(BLUE)🚀 Building release version... / 构建发布版本...$(NC)"
	@if [ -n "$(FEATURES)" ]; then \
		$(CARGO) build --release --features $(FEATURES); \
	else \
		$(CARGO) build --release; \
	fi
	@echo -e "$(GREEN)✅ Release build completed / 发布版本构建完成$(NC)"

# Run tests / 运行测试
test:
	@echo -e "$(BLUE)🧪 Running tests... / 运行测试...$(NC)"
	@if [ -n "$(FEATURES)" ]; then \
		$(CARGO) test --features $(FEATURES); \
	else \
		$(CARGO) test; \
	fi
	@echo -e "$(GREEN)✅ Tests completed / 测试完成$(NC)"

# Run tests with specific feature / 运行特定特性的测试
test-sled:
	@echo -e "$(BLUE)🧪 Running tests with sled feature... / 运行sled特性测试...$(NC)"
	$(CARGO) test --features sled

test-rocksdb:
	@echo -e "$(BLUE)🧪 Running tests with rocksdb feature... / 运行rocksdb特性测试...$(NC)"
	$(CARGO) test --features rocksdb

test-all-features:
	@echo -e "$(BLUE)🧪 Running tests with all features... / 运行所有特性测试...$(NC)"
	$(CARGO) test --all-features

# Run comprehensive code coverage analysis / 运行全面代码覆盖率分析
coverage: install-deps
	@echo -e "$(BLUE)📊 Running comprehensive code coverage analysis... / 运行全面代码覆盖率分析...$(NC)"
	@./scripts/coverage.sh
	@echo -e "$(GREEN)✅ Coverage analysis completed / 覆盖率分析完成$(NC)"

# Run quick code coverage analysis / 运行快速代码覆盖率分析
coverage-quick:
	@echo -e "$(BLUE)📊 Running quick coverage analysis... / 运行快速覆盖率分析...$(NC)"
	@./scripts/quick-coverage.sh
	@echo -e "$(GREEN)✅ Quick coverage analysis completed / 快速覆盖率分析完成$(NC)"

# Open coverage report / 打开覆盖率报告
coverage-open:
	@if [ -f "$(COVERAGE_DIR)/index.html" ]; then \
		echo -e "$(BLUE)🌐 Opening comprehensive coverage report... / 打开全面覆盖率报告...$(NC)"; \
		open "$(COVERAGE_DIR)/index.html" 2>/dev/null || xdg-open "$(COVERAGE_DIR)/index.html" 2>/dev/null || echo -e "$(YELLOW)Please open $(COVERAGE_DIR)/index.html manually / 请手动打开$(COVERAGE_DIR)/index.html$(NC)"; \
	elif [ -f "$(COVERAGE_DIR)/tarpaulin-report.html" ]; then \
		echo -e "$(BLUE)🌐 Opening quick coverage report... / 打开快速覆盖率报告...$(NC)"; \
		open "$(COVERAGE_DIR)/tarpaulin-report.html" 2>/dev/null || xdg-open "$(COVERAGE_DIR)/tarpaulin-report.html" 2>/dev/null || echo -e "$(YELLOW)Please open $(COVERAGE_DIR)/tarpaulin-report.html manually / 请手动打开$(COVERAGE_DIR)/tarpaulin-report.html$(NC)"; \
	else \
		echo -e "$(RED)❌ No coverage report found. Run 'make coverage' or 'make coverage-quick' first / 未找到覆盖率报告。请先运行'make coverage'或'make coverage-quick'$(NC)"; \
	fi

# Clean build artifacts / 清理构建产物
clean:
	@echo -e "$(BLUE)🧹 Cleaning build artifacts... / 清理构建产物...$(NC)"
	$(CARGO) clean
	rm -rf $(COVERAGE_DIR)
	@echo -e "$(GREEN)✅ Clean completed / 清理完成$(NC)"

# Format code / 格式化代码
format:
	@echo -e "$(BLUE)🎨 Formatting code... / 格式化代码...$(NC)"
	$(CARGO) fmt
	@echo -e "$(GREEN)✅ Code formatted / 代码格式化完成$(NC)"

# Check code formatting / 检查代码格式
format-check:
	@echo -e "$(BLUE)🔍 Checking code formatting... / 检查代码格式...$(NC)"
	$(CARGO) fmt --check

# Run linter / 运行代码检查
lint:
	@echo -e "$(BLUE)🔍 Running linter... / 运行代码检查...$(NC)"
	$(CARGO) clippy -- -D warnings
	@echo -e "$(GREEN)✅ Linting completed / 代码检查完成$(NC)"

# Run cargo check / 运行cargo检查
check:
	@echo -e "$(BLUE)✅ Running cargo check... / 运行cargo检查...$(NC)"
	$(CARGO) check
	@if [ -n "$(FEATURES)" ]; then \
		$(CARGO) check --features $(FEATURES); \
	fi
	@echo -e "$(GREEN)✅ Check completed / 检查完成$(NC)"

# Generate documentation / 生成文档
doc:
	@echo -e "$(BLUE)📚 Generating documentation... / 生成文档...$(NC)"
	$(CARGO) doc --no-deps --open
	@echo -e "$(GREEN)✅ Documentation generated / 文档生成完成$(NC)"

# Run benchmarks / 运行基准测试
bench:
	@echo -e "$(BLUE)⚡ Running benchmarks... / 运行基准测试...$(NC)"
	$(CARGO) bench
	@echo -e "$(GREEN)✅ Benchmarks completed / 基准测试完成$(NC)"

# Security audit / 安全审计
audit: install-deps
	@echo -e "$(BLUE)🔒 Running security audit... / 运行安全审计...$(NC)"
	$(CARGO) audit
	@echo -e "$(GREEN)✅ Security audit completed / 安全审计完成$(NC)"

# Check for outdated dependencies / 检查过时的依赖
outdated: install-deps
	@echo -e "$(BLUE)📦 Checking for outdated dependencies... / 检查过时的依赖...$(NC)"
	$(CARGO) outdated
	@echo -e "$(GREEN)✅ Dependency check completed / 依赖检查完成$(NC)"

# Full CI pipeline / 完整CI流水线
ci: format-check lint check test coverage-quick
	@echo -e "$(GREEN)🎉 CI pipeline completed successfully! / CI流水线成功完成！$(NC)"

# Development workflow / 开发工作流
dev: format lint test
	@echo -e "$(GREEN)🚀 Development workflow completed! / 开发工作流完成！$(NC)"

# Show project information / 显示项目信息
info:
	@echo -e "$(BLUE)📋 Project Information / 项目信息$(NC)"
	@echo "=================================================="
	@echo "Project Name / 项目名称: $(PROJECT_NAME)"
	@echo "Version / 版本: $(VERSION)"
	@echo "Rust Version / Rust版本: $(RUSTC_VERSION)"
	@echo "Repository Root / 仓库根目录: $(REPO_ROOT)"
	@echo "Target Directory / 目标目录: $(TARGET_DIR)"
	@echo "Coverage Directory / 覆盖率目录: $(COVERAGE_DIR)"
	@echo ""