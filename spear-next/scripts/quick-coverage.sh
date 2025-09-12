#!/bin/bash

# Quick Code Coverage Test Script / 快速代码覆盖率测试脚本
# This script runs a quick code coverage analysis
# 此脚本运行快速代码覆盖率分析

set -e

# Colors for output / 输出颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Quick Code Coverage Analysis / 快速代码覆盖率分析${NC}"
echo "=============================================="

# Check if cargo-tarpaulin is installed / 检查是否安装了cargo-tarpaulin
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}⚠️  cargo-tarpaulin not found. Installing... / 未找到cargo-tarpaulin，正在安装...${NC}"
    cargo install cargo-tarpaulin
fi

# Clean previous coverage data / 清理之前的覆盖率数据
echo -e "${BLUE}🧹 Cleaning previous coverage data... / 清理之前的覆盖率数据...${NC}"
rm -rf target/coverage
mkdir -p target/coverage

# Run quick coverage analysis / 运行快速覆盖率分析
echo -e "${BLUE}📊 Running quick coverage analysis... / 运行快速覆盖率分析...${NC}"

cargo tarpaulin \
    --config tarpaulin.toml \
    --output-dir target/coverage \
    --out Html \
    --out Stdout \
    --timeout 60 \
    --jobs 2 \
    || {
        echo -e "${RED}❌ Coverage analysis failed / 覆盖率分析失败${NC}"
        exit 1
    }

echo ""
echo -e "${GREEN}🎉 Quick coverage analysis completed! / 快速覆盖率分析完成！${NC}"
echo -e "${BLUE}📁 HTML report: / HTML报告: ${NC}target/coverage/tarpaulin-report.html"

# Try to open the report / 尝试打开报告
if command -v open &> /dev/null; then
    echo -e "${YELLOW}💡 Opening coverage report... / 打开覆盖率报告...${NC}"
    open target/coverage/tarpaulin-report.html
elif command -v xdg-open &> /dev/null; then
    echo -e "${YELLOW}💡 Opening coverage report... / 打开覆盖率报告...${NC}"
    xdg-open target/coverage/tarpaulin-report.html
else
    echo -e "${YELLOW}💡 Please open target/coverage/tarpaulin-report.html in your browser / 请在浏览器中打开target/coverage/tarpaulin-report.html${NC}"
fi

echo -e "${GREEN}✨ Quick coverage script completed! / 快速覆盖率脚本完成！${NC}"