# 前端 UI 测试指南（Playwright）

## 位置与启动

- 目录：`spear/ui-tests`
- 启动：`npm test`
- 测试会自动编译并运行内置 SMS（启用 WebAdmin），地址 `127.0.0.1:8081`

## 全局初始化

- `global-setup.ts` 在每次测试前清理 `data/objects`，保证对象列表、上传、删除用例的幂等性

## 关键用例

- `task_modal.spec.ts`：选择 `object`，在弹窗中点击 `Use` 填充对象 key
- `task_modal_scheme_prefill.spec.ts`：切换 URI 来源时自动预填 `Executable URI`
- `executable_select_unit.spec.ts`：使用隐藏原生 `select` 验证类型选择（稳定，不受弹层干扰）
- `objects.spec.ts`：上传对象并复制对象 URI
- `objects_delete.spec.ts`：删除对象并校验列表变化
- `objects_modified_tz.spec.ts`：按设置的时区显示人类可读时间

## 运行环境

- Playwright 配置：`playwright.config.ts`
- 浏览器：默认使用 Chromium（可在配置中调整）

## 常见问题

- 下拉选择不稳定：通过隐藏原生 `select` 作为测试入口，避免 Antd 弹层选择的定位波动
- 可执行类型提供原生 `select` 镜像，兼顾可访问性与自动化稳定性
- 删除校验：优先通过提示与行匹配校验，减少依赖全局行计数

## 提示

- 可在 CI 中运行：结合 `ui-tests/package.json` 脚本
- 日志与截图：Playwright 默认在失败时生成上下文文件，可在 `test-results` 查看
