#include <spear.h>

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
    rc = sp_cchat_set_param_string(fd, "model", model);
    if (rc != 0) {
        printf("set model failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    rc = sp_cchat_set_param_u32(fd, "timeout_ms", 30000);
    if (rc != 0) {
        printf("set timeout_ms failed: %d\n", rc);
        sp_cchat_close(fd);
        return 1;
    }

    int32_t resp_fd = sp_cchat_send(fd, 0);
    if (resp_fd < 0) {
        printf("cchat_send failed: %d\n", resp_fd);
        sp_cchat_close(fd);
        return 1;
    }

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
