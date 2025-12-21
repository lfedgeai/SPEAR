# Backend Adapter 设计文档索引

本目录包含 Spear 的 Backend Adapter Layer（多操作/多模态）设计文档，按层级拆分，便于逐步落地与演进。

## 总览

- `overview-zh.md`：范围、目标、术语与整体架构概览
- `architecture-zh.md`：分层设计、模块边界与 Hostcall family 演进

## 规范

- `ir-zh.md`：Canonical IR（Envelope/Response/Error/MediaRef）
- `operations-zh.md`：多操作（Chat/Image/ASR/TTS/Embeddings/Realtime）字段骨架与能力建议

## 路由与后端

- `routing-zh.md`：Capabilities 建模、候选过滤、策略选择（LB/Fallback/Hedge/Mirror）
- `backends-zh.md`：Cargo feature 编译裁剪、Registry/Discovery、配置模型
- `streaming-zh.md`：Realtime/Streaming 子系统（transport、生命周期、事件）

## 工程化

- `reliability-security-observability-zh.md`：错误模型、安全边界、可观测性
- `migration-mvp-zh.md`：与 legacy Go 的映射与分阶段落地（MVP→扩展）

