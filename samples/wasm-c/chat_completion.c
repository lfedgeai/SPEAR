#include <spear.h>

// Chat completion sample (WASM-C).
// Chat completion 示例（WASM-C）。
//
// This sample uses cchat_* hostcalls to submit a single chat completion request.
// 本示例使用 cchat_* hostcalls 提交一次 chat completion 请求。

#ifndef SP_OPENAI_MODEL
#define SP_OPENAI_MODEL "gpt-4o-mini"
#endif

int main() {
    int32_t fd = sp_cchat_create();
    if (fd < 0) {
        printf("cchat_create failed: %d\n", fd);
        return 1;
    }

    const char *role = "user";
    const char *content = "Reply with exactly: pong";
    int32_t rc = sp_cchat_write_msg_str(fd, role, content);
    if (rc != 0) {
        printf("cchat_write_msg failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    const char *model = SP_OPENAI_MODEL;
    // Set model explicitly for non-stub backends.
    // 对非 stub backend 显式设置模型。
    rc = sp_cchat_set_param_string(fd, "model", model);
    if (rc != 0) {
        printf("set model failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    // Timeout in milliseconds.
    // 超时（毫秒）。
    rc = sp_cchat_set_param_u32(fd, "timeout_ms", 30000);
    if (rc != 0) {
        printf("set timeout_ms failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    // Send request and get a response fd.
    // 发送请求并获得 response fd。
    int32_t resp_fd = sp_cchat_send(fd, 0);
    if (resp_fd < 0) {
        printf("cchat_send failed: %d\n", resp_fd);
        sp_cchat_close(fd);
        return 1;
    }

    // Receive response JSON into a heap buffer.
    // 把响应 JSON 读到堆内存 buffer。
    uint32_t resp_len = 0;
    uint8_t *resp = sp_cchat_recv_alloc(resp_fd, &resp_len);
    if (!resp) {
        printf("cchat_recv failed\n");
        sp_cchat_close(resp_fd);
        sp_cchat_close(fd);
        return 1;
    }

    printf("response_bytes=%u\n", resp_len);
    printf("response_json=%s\n", (char *)resp);

    free(resp);
    sp_cchat_close(resp_fd);
    sp_cchat_close(fd);
    return 0;
}
