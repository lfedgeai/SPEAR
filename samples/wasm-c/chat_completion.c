#include <spear.h>

#include <stdlib.h>
#include <string.h>

// Chat completion sample (WASM-C).
// Chat completion 示例（WASM-C）。
//
// This sample uses cchat_* hostcalls to submit a single chat completion request.
// 本示例使用 cchat_* hostcalls 提交一次 chat completion 请求。

#ifndef SP_MODEL
#define SP_MODEL "gpt-4o-mini"
#endif

#ifndef SP_OLLAMA_GEMMA3_MODEL
#define SP_OLLAMA_GEMMA3_MODEL "gemma3:1b"
#endif

#define CHAT_CONTENT "Hi, what is your name?"

static int extract_spear_backend(const char *json, char *out, size_t out_len) {
    const char *spear = strstr(json, "\"_spear\"");
    if (!spear) return 0;
    const char *backend = strstr(spear, "\"backend\":\"");
    if (!backend) return 0;
    backend += strlen("\"backend\":\"");
    const char *end = strchr(backend, '"');
    if (!end) return 0;
    size_t n = (size_t)(end - backend);
    if (n + 1 > out_len) n = out_len - 1;
    memcpy(out, backend, n);
    out[n] = '\0';
    return 1;
}

//#define SP_ROUTE_OLLAMA_GEMMA3

int main() {
    int32_t fd = sp_cchat_create();
    if (fd < 0) {
        printf("cchat_create failed: %d\n", fd);
        return 1;
    }

    const char *role = "user";
    const char *content = CHAT_CONTENT;
    int32_t rc = sp_cchat_write_msg_str(fd, role, content);
    if (rc != 0) {
        printf("cchat_write_msg failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    const char *model = SP_MODEL;

#if defined(SP_ROUTE_OLLAMA_GEMMA3)
    model = SP_OLLAMA_GEMMA3_MODEL;
#endif

    rc = sp_cchat_set_param_string(fd, "model", model);
    if (rc != 0) {
        printf("set model failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    printf("debug_model=%s\n", model);

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

    char backend_buf[128] = {0};
    if (extract_spear_backend((char *)resp, backend_buf, sizeof(backend_buf))) {
        printf("debug_backend=%s\n", backend_buf);
    } else {
        printf("debug_backend=unknown\n");
    }

    free(resp);
    sp_cchat_close(resp_fd);
    sp_cchat_close(fd);
    return 0;
}
