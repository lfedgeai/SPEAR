# TASK_ID 环境变量注入测试

“实例环境中会注入 `TASK_ID`”这一行为，应该在注入逻辑的源头进行单元测试。

## 实现位置

`Task::create_instance_config()` 会注入：

- `TASK_ID = task.id`

见：[task.rs](../src/spearlet/execution/task.rs)

## 测试位置

对应单测放在 `task.rs`：

- `test_instance_config_injects_task_id_env`

这样可以避免依赖 runtime 的 `create_instance` 失败/成功等实现细节。

