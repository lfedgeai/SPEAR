#include <spear.h>

int32_t tool_call(int32_t args_ptr, int32_t args_len, int32_t out_ptr, int32_t out_len_ptr) {
    printf(
        "tool_call invoked: args_ptr=%d args_len=%d out_ptr=%d out_len_ptr=%d\n",
        args_ptr,
        args_len,
        out_ptr,
        out_len_ptr
    );
    (void)args_ptr;
    (void)args_len;
    (void)out_ptr;
    (void)out_len_ptr;
    return 0;
}

int main() {
    int32_t fd = sp_cchat_create();
    if (fd < 0) {
        printf("cchat_create failed: %d\n", fd);
        return 1;
    }

    const char *fn_json = "{\"name\":\"tool_call\",\"parameters\":{}}";
    int32_t fn_offset = (int32_t)(uintptr_t)tool_call;
    printf("tool_fn_offset=%d\n", fn_offset);

    int32_t rc = sp_cchat_write_fn_str(fd, fn_offset, fn_json);
    printf("cchat_write_fn_rc=%d\n", rc);

    sp_cchat_close(fd);
    return rc == 0 ? 0 : 1;
}
