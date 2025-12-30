#ifndef SPEAR_WASM_SPEAR_H
#define SPEAR_WASM_SPEAR_H

#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SPEAR_IMPORT(name) __attribute__((import_module("spear"), import_name(name)))

enum {
    SPEAR_CCHAT_OK = 0,
    SPEAR_CCHAT_ERR_INVALID_FD = -1,
    SPEAR_CCHAT_ERR_INVALID_PTR = -2,
    SPEAR_CCHAT_ERR_BUFFER_TOO_SMALL = -3,
    SPEAR_CCHAT_ERR_INVALID_CMD = -4,
    SPEAR_CCHAT_ERR_INTERNAL = -5,
};

enum {
    SPEAR_CCHAT_CTL_SET_PARAM = 1,
    SPEAR_CCHAT_CTL_GET_METRICS = 2,
};

SPEAR_IMPORT("time_now_ms")
int64_t sp_time_now_ms(void);

SPEAR_IMPORT("cchat_create")
int32_t sp_cchat_create(void);

SPEAR_IMPORT("cchat_write_msg")
int32_t sp_cchat_write_msg(int32_t fd, int32_t role_ptr, int32_t role_len, int32_t content_ptr,
                           int32_t content_len);

SPEAR_IMPORT("cchat_ctl")
int32_t sp_cchat_ctl(int32_t fd, int32_t cmd, int32_t arg_ptr, int32_t arg_len_ptr);

SPEAR_IMPORT("cchat_send")
int32_t sp_cchat_send(int32_t fd, int32_t flags);

SPEAR_IMPORT("cchat_recv")
int32_t sp_cchat_recv(int32_t response_fd, int32_t out_ptr, int32_t out_len_ptr);

SPEAR_IMPORT("cchat_close")
int32_t sp_cchat_close(int32_t fd);

static inline int32_t sp_cchat_write_msg_str(int32_t fd, const char *role, const char *content) {
    return sp_cchat_write_msg(fd, (int32_t)(uintptr_t)role, (int32_t)strlen(role),
                              (int32_t)(uintptr_t)content, (int32_t)strlen(content));
}

static inline int32_t sp_cchat_set_param_json(int32_t fd, const char *json, uint32_t json_len) {
    uint32_t len = json_len;
    return sp_cchat_ctl(fd, SPEAR_CCHAT_CTL_SET_PARAM, (int32_t)(uintptr_t)json,
                        (int32_t)(uintptr_t)&len);
}

static inline int32_t sp_cchat_set_param_string(int32_t fd, const char *key, const char *value) {
    char buf[512];
    int n = snprintf(buf, sizeof(buf), "{\"key\":\"%s\",\"value\":\"%s\"}", key, value);
    if (n <= 0 || (size_t)n >= sizeof(buf)) {
        return SPEAR_CCHAT_ERR_INTERNAL;
    }
    return sp_cchat_set_param_json(fd, buf, (uint32_t)n);
}

static inline int32_t sp_cchat_set_param_u32(int32_t fd, const char *key, uint32_t value) {
    char buf[256];
    int n = snprintf(buf, sizeof(buf), "{\"key\":\"%s\",\"value\":%u}", key, value);
    if (n <= 0 || (size_t)n >= sizeof(buf)) {
        return SPEAR_CCHAT_ERR_INTERNAL;
    }
    return sp_cchat_set_param_json(fd, buf, (uint32_t)n);
}

static inline uint8_t *sp_cchat_recv_alloc(int32_t resp_fd, uint32_t *out_len) {
    uint32_t cap = 64 * 1024;
    uint8_t *buf = (uint8_t *)malloc(cap + 1);
    if (!buf) {
        return NULL;
    }
    for (int attempt = 0; attempt < 3; attempt++) {
        uint32_t len = cap;
        int32_t rc = sp_cchat_recv(resp_fd, (int32_t)(uintptr_t)buf, (int32_t)(uintptr_t)&len);
        if (rc >= 0) {
            buf[len] = 0;
            *out_len = len;
            return buf;
        }
        if (rc != SPEAR_CCHAT_ERR_BUFFER_TOO_SMALL) {
            free(buf);
            return NULL;
        }
        cap = len;
        uint8_t *b2 = (uint8_t *)realloc(buf, cap + 1);
        if (!b2) {
            free(buf);
            return NULL;
        }
        buf = b2;
    }
    free(buf);
    return NULL;
}

#endif
