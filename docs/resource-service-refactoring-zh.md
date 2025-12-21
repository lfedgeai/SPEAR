# 资源服务代码重构指南

## 概述
本文档描述了对 `ResourceService` 的重构，以消除平均值计算方法中的代码重复。

## 发现的问题
`get_average_cpu_usage` 和 `get_average_memory_usage` 函数包含重复的代码模式：
- 两个函数都获取资源列表
- 两个函数都检查空资源并返回 0.0
- 两个函数都通过对特定字段求和来计算总计
- 两个函数都执行相同的除法：`total / resources.len() as f64`

## 实施的解决方案
创建了一个通用的辅助函数 `calculate_average_field`，它：
- 接受一个闭包来从 `NodeResourceInfo` 中提取所需字段
- 处理空资源列表的通用逻辑
- 使用提取的字段值执行平均值计算
- 返回计算出的平均值

## 代码变更

### 重构前
```rust
pub async fn get_average_cpu_usage(&self) -> Result<f64, SmsError> {
    let resources = self.list_resources().await?;
    if resources.is_empty() {
        return Ok(0.0);
    }
    
    let total: f64 = resources.iter().map(|r| r.cpu_usage_percent).sum();
    Ok(total / resources.len() as f64)
}

pub async fn get_average_memory_usage(&self) -> Result<f64, SmsError> {
    let resources = self.list_resources().await?;
    if resources.is_empty() {
        return Ok(0.0);
    }
    
    let total: f64 = resources.iter().map(|r| r.memory_usage_percent).sum();
    Ok(total / resources.len() as f64)
}
```

### 重构后
```rust
async fn calculate_average_field<F>(&self, field_extractor: F) -> Result<f64, SmsError>
where
    F: Fn(&NodeResourceInfo) -> f64,
{
    let resources = self.list_resources().await?;
    if resources.is_empty() {
        return Ok(0.0);
    }
    
    let total: f64 = resources.iter().map(field_extractor).sum();
    Ok(total / resources.len() as f64)
}

pub async fn get_average_cpu_usage(&self) -> Result<f64, SmsError> {
    self.calculate_average_field(|r| r.cpu_usage_percent).await
}

pub async fn get_average_memory_usage(&self) -> Result<f64, SmsError> {
    self.calculate_average_field(|r| r.memory_usage_percent).await
}
```

## 优势
1. **减少代码重复**：消除了多个函数中的重复逻辑
2. **提高可维护性**：平均值计算逻辑的更改只需要在一个地方进行
3. **增强可扩展性**：容易为其他字段添加新的平均值计算方法
4. **更好的可测试性**：通用逻辑集中化，更容易测试

## 测试
所有现有测试继续通过，确认重构在改善代码质量的同时保持了相同的功能。

## 修改的文件
- `src/services/resource.rs`：重构了平均值计算方法

## 未来增强
`calculate_average_field` 辅助函数可以用来轻松添加其他资源指标的平均值计算，例如：
- 平均磁盘使用率
- 平均网络吞吐量
- 平均负载

## 相关文件
- `src/services/resource.rs`：主要实现
- 同一文件中的测试验证功能