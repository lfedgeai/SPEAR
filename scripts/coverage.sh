#!/bin/bash

# Code Coverage Test Script / 代码覆盖率测试脚本
# This script runs code coverage analysis using cargo-tarpaulin
# 此脚本使用cargo-tarpaulin运行代码覆盖率分析

set -e

# Colors for output / 输出颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory / 脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo -e "${BLUE}🔍 SPEAR Next Code Coverage Analysis / SPEAR Next代码覆盖率分析${NC}"
echo "=================================================="

# Check if cargo-tarpaulin is installed / 检查是否安装了cargo-tarpaulin
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}⚠️  cargo-tarpaulin not found. Installing... / 未找到cargo-tarpaulin，正在安装...${NC}"
    cargo install cargo-tarpaulin
fi

# Change to project directory / 切换到项目目录
cd "$PROJECT_DIR"

# Clean previous coverage data / 清理之前的覆盖率数据
echo -e "${BLUE}🧹 Cleaning previous coverage data... / 清理之前的覆盖率数据...${NC}"
rm -rf target/coverage
mkdir -p target/coverage

# Function to run coverage with specific features / 运行特定特性的覆盖率函数
run_coverage() {
    local feature_name="$1"
    local feature_flag="$2"
    local output_suffix="$3"
    
    echo -e "${BLUE}📊 Running coverage for $feature_name... / 运行$feature_name覆盖率...${NC}"
    
    # Set output directory / 设置输出目录
    local output_dir="target/coverage/$output_suffix"
    mkdir -p "$output_dir"
    
    # Run tarpaulin / 运行tarpaulin
    cargo tarpaulin \
        $feature_flag \
        --config tarpaulin.toml \
        --output-dir "$output_dir" \
        --out Html --out Lcov --out Json \
        --timeout 120 \
        --verbose \
        || {
            echo -e "${RED}❌ Coverage failed for $feature_name / $feature_name覆盖率失败${NC}"
            return 1
        }
    
    echo -e "${GREEN}✅ Coverage completed for $feature_name / $feature_name覆盖率完成${NC}"
}

# Run coverage for different configurations / 运行不同配置的覆盖率

echo -e "${BLUE}📊 Running coverage with default configuration... / 运行默认配置覆盖率...${NC}"

# Run basic coverage analysis / 运行基本覆盖率分析
cargo tarpaulin \
    --config tarpaulin.toml \
    --output-dir target/coverage \
    --out Html --out Lcov --out Json \
    --timeout 120 \
    --verbose \
    || {
        echo -e "${RED}❌ Coverage analysis failed / 覆盖率分析失败${NC}"
        exit 1
    }

# Display coverage results / 显示覆盖率结果
echo -e "${GREEN}✅ Coverage analysis completed successfully! / 覆盖率分析成功完成！${NC}"

# Display results / 显示结果
echo ""
echo -e "${GREEN}🎉 Code coverage analysis completed! / 代码覆盖率分析完成！${NC}"
echo -e "${BLUE}📁 Reports are available in: / 报告位于: ${NC}target/coverage/"
echo -e "${BLUE}🌐 HTML report: / HTML报告: ${NC}target/coverage/tarpaulin-report.html"
echo ""

# Check if we can open the report / 检查是否可以打开报告
if command -v open &> /dev/null; then
    echo -e "${YELLOW}💡 Opening coverage report in browser... / 在浏览器中打开覆盖率报告...${NC}"
    open target/coverage/tarpaulin-report.html
elif command -v xdg-open &> /dev/null; then
    echo -e "${YELLOW}💡 Opening coverage report in browser... / 在浏览器中打开覆盖率报告...${NC}"
    xdg-open target/coverage/tarpaulin-report.html
else
    echo -e "${YELLOW}💡 Please open target/coverage/tarpaulin-report.html in your browser / 请在浏览器中打开target/coverage/tarpaulin-report.html${NC}"
fi

echo -e "${GREEN}✨ Coverage analysis script completed successfully! / 覆盖率分析脚本成功完成！${NC}"