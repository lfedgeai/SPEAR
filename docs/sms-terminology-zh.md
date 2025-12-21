# SMS术语说明 / SMS Terminology

## 什么是SMS？ / What is SMS?

**SMS**是**SPEAR Metadata Server**（SPEAR元数据服务器）的缩写。

**SMS** stands for **SPEAR Metadata Server**.

## 概述 / Overview

SPEAR元数据服务器（SMS）是SPEAR系统的核心组件，提供：

The SPEAR Metadata Server (SMS) is a core component of the SPEAR system that provides:

### 主要功能 / Key Features

- **节点管理 / Node Management**: 计算节点的注册、更新和生命周期管理
- **资源跟踪 / Resource Tracking**: 节点资源（CPU、内存等）的监控和管理
- **元数据存储 / Metadata Storage**: 使用KV抽象层的系统元数据集中存储
- **gRPC接口 / gRPC API**: 用于节点操作的高性能API
- **HTTP网关 / HTTP Gateway**: 带有Swagger文档的RESTful API
- **心跳监控 / Heartbeat Monitoring**: 节点健康监控和自动清理

### 架构组件 / Architecture Components

1. **SMS服务 / SmsService**: 主要的gRPC服务实现
2. **节点注册表 / NodeRegistry**: 节点信息管理
3. **节点资源注册表 / NodeResourceRegistry**: 资源信息管理
4. **KV存储层 / KV Storage Layer**: 可插拔的存储后端（内存、Sled、RocksDB）
5. **HTTP网关 / HTTP Gateway**: 带有OpenAPI文档的REST API网关

### 代码中的使用 / Usage in Code

在代码库中，您会看到以下引用：

Throughout the codebase, you'll see references to:

- `SmsService` - 主要服务接口 / The main service interface
- `SmsServiceImpl` - 服务实现 / The service implementation
- `SmsError` - SMS操作的错误类型 / Error types for SMS operations
- `SmsConfig` - SMS服务配置 / Configuration for SMS service

这些都指向**SPEAR元数据服务器**功能。

These all refer to the **SPEAR Metadata Server** functionality.

## 相关文档 / Related Documentation

- [统一KV架构](./unified-kv-architecture-zh.md) - SMS使用的存储架构
- [KV抽象层](./kv-abstraction-zh.md) - 存储接口文档
- [KV工厂模式](./kv-factory-pattern-zh.md) - 存储后端工厂模式

## 配置 / Configuration

SMS可以通过以下方式配置：

SMS can be configured through:

- 配置文件（TOML格式）/ Configuration files (TOML format)
- 环境变量 / Environment variables
- 命令行参数 / Command-line arguments

参见`example-config.toml`获取配置示例。

See `example-config.toml` for configuration examples.