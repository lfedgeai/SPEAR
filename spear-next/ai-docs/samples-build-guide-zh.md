# WASM 示例构建指南（samples）

## 目录结构
- 源码：`samples/wasm-c/hello.c`
- 产物：`samples/build/hello.wasm`

## 构建命令
- 运行：`make samples`
- 编译器优先级：
  - 优先使用 `zig`：`zig cc -target wasm32-wasi`
  - 备选 `clang`：需要设置 `WASI_SYSROOT` 指向 WASI SDK 的 sysroot

## clang 使用说明
- 环境变量：`WASI_SYSROOT=/opt/wasi-sdk/share/wasi-sysroot`（按实际路径）
- 命令会使用：`clang --target=wasm32-wasi --sysroot=$(WASI_SYSROOT)`
- 如未设置或未安装 SDK，会报错并提示安装 `zig` 或设置 `WASI_SYSROOT`

## 重要变更
- Makefile 仅保留 `samples` 目标
- 已移除 `sample-upload` 与 `sample-register` 目标（不再在构建脚本中执行上传/注册）

## 与运行时集成
- 构建生成的 `hello.wasm` 可通过 SMS 文件服务上传后在任务注册中以 `executable.uri` 引用
- Spearlet WASM 运行时在实例创建阶段将校验模块字节格式，非法内容会报错

## 示例源码
```c
int main() { return 0; }
```
