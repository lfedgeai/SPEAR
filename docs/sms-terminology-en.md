# SMS Terminology / SMS术语说明

## What is SMS? / 什么是SMS？

**SMS** stands for **SPEAR Metadata Server** (SPEAR元数据服务器).

SMS是**SPEAR Metadata Server**（SPEAR元数据服务器）的缩写。

## Overview / 概述

The SPEAR Metadata Server (SMS) is a core component of the SPEAR system that provides:

SPEAR元数据服务器（SMS）是SPEAR系统的核心组件，提供：

### Key Features / 主要功能

- **Node Management / 节点管理**: Registration, updates, and lifecycle management of compute nodes
- **Resource Tracking / 资源跟踪**: Monitoring and management of node resources (CPU, memory, etc.)
- **Metadata Storage / 元数据存储**: Centralized storage for system metadata using KV abstraction layer
- **gRPC API / gRPC接口**: High-performance API for node operations
- **HTTP Gateway / HTTP网关**: RESTful API with Swagger documentation
- **Heartbeat Monitoring / 心跳监控**: Node health monitoring and automatic cleanup

### Architecture Components / 架构组件

1. **SmsService / SMS服务**: Main gRPC service implementation
2. **NodeRegistry / 节点注册表**: Node information management
3. **NodeResourceRegistry / 节点资源注册表**: Resource information management
4. **KV Storage Layer / KV存储层**: Pluggable storage backend (Memory, Sled, RocksDB)
5. **HTTP Gateway / HTTP网关**: REST API gateway with OpenAPI documentation

### Usage in Code / 代码中的使用

Throughout the codebase, you'll see references to:

在代码库中，您会看到以下引用：

- `SmsService` - The main service interface
- `SmsServiceImpl` - The service implementation
- `SmsError` - Error types for SMS operations
- `SmsConfig` - Configuration for SMS service

These all refer to the **SPEAR Metadata Server** functionality.

这些都指向**SPEAR元数据服务器**功能。

## Related Documentation / 相关文档

- [Unified KV Architecture](./unified-kv-architecture-en.md) - Storage architecture used by SMS
- [KV Abstraction Layer](./kv-abstraction-en.md) - Storage interface documentation
- [KV Factory Pattern](./kv-factory-pattern-en.md) - Storage backend factory pattern

## Configuration / 配置

SMS can be configured through:

SMS可以通过以下方式配置：

- Configuration files (TOML format)
- Environment variables
- Command-line arguments

See `example-config.toml` for configuration examples.

参见`example-config.toml`获取配置示例。