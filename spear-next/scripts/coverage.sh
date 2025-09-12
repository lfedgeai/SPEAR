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

echo -e "${BLUE}📊 Running coverage with default features... / 运行默认特性覆盖率...${NC}"
run_coverage "Default Features" "" "default"

echo -e "${BLUE}📊 Running coverage with sled feature... / 运行sled特性覆盖率...${NC}"
run_coverage "Sled Feature" "--features sled" "sled"

echo -e "${BLUE}📊 Running coverage with rocksdb feature... / 运行rocksdb特性覆盖率...${NC}"
run_coverage "RocksDB Feature" "--features rocksdb" "rocksdb"

echo -e "${BLUE}📊 Running coverage with all features... / 运行所有特性覆盖率...${NC}"
run_coverage "All Features" "--all-features" "all-features"

# Generate combined report / 生成合并报告
echo -e "${BLUE}📋 Generating combined coverage report... / 生成合并覆盖率报告...${NC}"

# Create a summary HTML file / 创建摘要HTML文件
cat > target/coverage/index.html << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SPEAR Next Code Coverage Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background-color: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        h1 { color: #333; text-align: center; margin-bottom: 30px; }
        .report-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; margin-top: 30px; }
        .report-card { background: #f8f9fa; padding: 20px; border-radius: 6px; border-left: 4px solid #007bff; }
        .report-card h3 { margin-top: 0; color: #007bff; }
        .report-card a { color: #007bff; text-decoration: none; font-weight: bold; }
        .report-card a:hover { text-decoration: underline; }
        .description { color: #666; margin-bottom: 20px; text-align: center; }
        .timestamp { text-align: center; color: #888; font-size: 0.9em; margin-top: 30px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🔍 SPEAR Next Code Coverage Report</h1>
        <p class="description">
            This report provides code coverage analysis for different feature configurations of the SPEAR Next project.
            <br>
            此报告提供SPEAR Next项目不同特性配置的代码覆盖率分析。
        </p>
        
        <div class="report-grid">
            <div class="report-card">
                <h3>📊 Default Features</h3>
                <p>Coverage report with default project features / 默认项目特性的覆盖率报告</p>
                <a href="default/tarpaulin-report.html">View Report / 查看报告</a>
            </div>
            
            <div class="report-card">
                <h3>🗄️ Sled Feature</h3>
                <p>Coverage report with Sled database backend / Sled数据库后端的覆盖率报告</p>
                <a href="sled/tarpaulin-report.html">View Report / 查看报告</a>
            </div>
            
            <div class="report-card">
                <h3>🪨 RocksDB Feature</h3>
                <p>Coverage report with RocksDB database backend / RocksDB数据库后端的覆盖率报告</p>
                <a href="rocksdb/tarpaulin-report.html">View Report / 查看报告</a>
            </div>
            
            <div class="report-card">
                <h3>🚀 All Features</h3>
                <p>Coverage report with all project features enabled / 启用所有项目特性的覆盖率报告</p>
                <a href="all-features/tarpaulin-report.html">View Report / 查看报告</a>
            </div>
        </div>
        
        <div class="timestamp">
            Generated on: $(date) / 生成时间: $(date)
        </div>
    </div>
</body>
</html>
EOF

# Display results / 显示结果
echo ""
echo -e "${GREEN}🎉 Code coverage analysis completed! / 代码覆盖率分析完成！${NC}"
echo -e "${BLUE}📁 Reports are available in: / 报告位于: ${NC}target/coverage/"
echo -e "${BLUE}🌐 Open the main report: / 打开主报告: ${NC}target/coverage/index.html"
echo ""

# Check if we can open the report / 检查是否可以打开报告
if command -v open &> /dev/null; then
    echo -e "${YELLOW}💡 Opening coverage report in browser... / 在浏览器中打开覆盖率报告...${NC}"
    open target/coverage/index.html
elif command -v xdg-open &> /dev/null; then
    echo -e "${YELLOW}💡 Opening coverage report in browser... / 在浏览器中打开覆盖率报告...${NC}"
    xdg-open target/coverage/index.html
else
    echo -e "${YELLOW}💡 Please open target/coverage/index.html in your browser / 请在浏览器中打开target/coverage/index.html${NC}"
fi

echo -e "${GREEN}✨ Coverage analysis script completed successfully! / 覆盖率分析脚本成功完成！${NC}"