# TASK_ID environment injection test

The behavior “`TASK_ID` is injected into an instance environment” is tested at the source of the injection.

## Where it is implemented

`Task::create_instance_config()` injects:

- `TASK_ID = task.id`

See: [task.rs](../src/spearlet/execution/task.rs)

## Where it is tested

The unit test lives in `task.rs`:

- `test_instance_config_injects_task_id_env`

This keeps the test independent from runtime-specific `create_instance` behavior.

