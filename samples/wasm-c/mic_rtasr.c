#include <spear.h>

// Microphone -> Realtime ASR sample (WASM-C).
// 麦克风 -> Realtime ASR 示例（WASM-C）。
//
// This sample shows a single-threaded epoll loop that:
// - reads pcm frames from mic_fd
// - writes audio bytes into rtasr_fd
// - reads JSON transcription events from rtasr_fd
//
// 本示例展示一个单线程 epoll 循环：
// - 从 mic_fd 读取 pcm 帧
// - 写入 rtasr_fd
// - 从 rtasr_fd 读取转写 JSON 事件

#ifndef SP_RTASR_BACKEND
#define SP_RTASR_BACKEND "openai-realtime-asr"
#endif

#ifndef SP_RTASR_MODEL
#define SP_RTASR_MODEL "gpt-4o-mini-transcribe"
#endif

static int print_json_string_field(const char *json, const char *key) {
    char pat[128];
    int n = snprintf(pat, sizeof(pat), "\"%s\":\"", key);
    if (n <= 0 || (size_t)n >= sizeof(pat)) {
        return 0;
    }
    const char *p = strstr(json, pat);
    if (!p) {
        return 0;
    }
    p += n;
    const char *end = strchr(p, '\"');
    if (!end) {
        return 0;
    }
    fwrite(p, 1, (size_t)(end - p), stdout);
    fputc('\n', stdout);
    return 1;
}

static int get_json_string_field_buf(const char *json, const char *key, char *out,
                                     size_t out_cap) {
    if (!out || out_cap == 0) {
        return 0;
    }
    out[0] = 0;

    char pat[128];
    int n = snprintf(pat, sizeof(pat), "\"%s\":\"", key);
    if (n <= 0 || (size_t)n >= sizeof(pat)) {
        return 0;
    }
    const char *p = strstr(json, pat);
    if (!p) {
        return 0;
    }
    p += n;
    const char *end = strchr(p, '\"');
    if (!end) {
        return 0;
    }
    size_t len = (size_t)(end - p);
    if (len >= out_cap) {
        len = out_cap - 1;
    }
    memcpy(out, p, len);
    out[len] = 0;
    return 1;
}

int main() {
    // Create epoll instance.
    // 创建 epoll 实例。
    int32_t epfd = sp_ep_create();
    if (epfd < 0) {
        printf("ep_create failed: %d\n", epfd);
        return 1;
    }

    // Create mic and rtasr fds.
    // 创建 mic 与 rtasr fd。
    int32_t mic_fd = sp_mic_create();
    if (mic_fd < 0) {
        printf("mic_create failed: %d\n", mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    int32_t asr_fd = sp_rtasr_create();
    if (asr_fd < 0) {
        printf("rtasr_create failed: %d\n", asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    // Watch both fds.
    // 同时监听两个 fd。
    int32_t rc = sp_ep_ctl(epfd, SPEAR_EP_CTL_ADD, mic_fd,
                           SPEAR_EPOLLIN | SPEAR_EPOLLERR | SPEAR_EPOLLHUP);
    if (rc != 0) {
        printf("ep_ctl add mic_fd failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    rc = sp_ep_ctl(epfd, SPEAR_EP_CTL_ADD, asr_fd,
                   SPEAR_EPOLLIN | SPEAR_EPOLLERR | SPEAR_EPOLLHUP);
    if (rc != 0) {
        printf("ep_ctl add asr_fd failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    // Configure mic source.
    // 配置 mic 输入源。
    const char *mic_cfg =
        "{\"sample_rate_hz\":24000,\"channels\":1,\"format\":\"pcm16\",\"frame_ms\":20,\"source\":\"device\",\"fallback\":{\"to_stub\":false}}";
    rc = sp_mic_set_param_json(mic_fd, mic_cfg, (uint32_t)strlen(mic_cfg));
    if (rc != 0) {
        printf("mic_ctl failed: %d\n", rc);
        uint32_t st_len = 0;
        uint8_t *st = sp_mic_get_status_alloc(mic_fd, &st_len);
        if (st) {
            printf("mic_status: %s\n", (char *)st);
            free(st);
        }
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    // Configure rtasr backend.
    // 配置 rtasr backend。
    rc = sp_rtasr_set_param_string(asr_fd, "transport", "websocket");
    if (rc != 0) {
        printf("rtasr set transport failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    rc = sp_rtasr_set_param_string(asr_fd, "backend", SP_RTASR_BACKEND);
    if (rc != 0) {
        printf("rtasr set backend failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    rc = sp_rtasr_set_param_string(asr_fd, "model", SP_RTASR_MODEL);
    if (rc != 0) {
        printf("rtasr set model failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    // Set server-vad based segmentation / autoflush.
    // 设置 server-vad 分段策略 / autoflush。
    const char *autoflush =
        "{\"strategy\":\"server_vad\",\"vad\":{\"silence_ms\":600},\"flush_on_close\":true}";
    rc = sp_rtasr_set_autoflush_json(asr_fd, autoflush, (uint32_t)strlen(autoflush));
    if (rc != 0) {
        printf("rtasr set autoflush failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    // Connect to backend.
    // 连接 backend。
    rc = sp_rtasr_connect(asr_fd);
    if (rc != 0) {
        printf("rtasr connect failed: %d\n", rc);
        sp_rtasr_close(asr_fd);
        sp_mic_close(mic_fd);
        sp_ep_close(epfd);
        return 1;
    }

    printf("mic_rtasr started\n");

    uint8_t ready_buf[8 * 64];
    while (1) {
        // Wait for readiness events.
        // 等待就绪事件。
        uint32_t ready_len = sizeof(ready_buf);
        int32_t nready = sp_ep_wait(epfd, (int32_t)(uintptr_t)ready_buf,
                                    (int32_t)(uintptr_t)&ready_len, 2000);
        if (nready < 0) {
            printf("ep_wait failed: %d\n", nready);
            break;
        }
        if (nready == 0) {
            continue;
        }

        for (int i = 0; i < nready; i++) {
            int32_t fd = 0;
            int32_t ev = 0;
            memcpy(&fd, ready_buf + (i * 8), 4);
            memcpy(&ev, ready_buf + (i * 8) + 4, 4);

            if (ev & SPEAR_EPOLLHUP) {
                if (fd == mic_fd) {
                    printf("mic hup\n");
                } else if (fd == asr_fd) {
                    printf("rtasr hup\n");
                }
                goto out;
            }
            if (ev & SPEAR_EPOLLERR) {
                if (fd == mic_fd) {
                    printf("mic err\n");
                } else if (fd == asr_fd) {
                    printf("rtasr err\n");
                }
                goto out;
            }

            if (fd == mic_fd && (ev & SPEAR_EPOLLIN)) {
                // Read one PCM frame and feed it into rtasr.
                // 读一帧 PCM 并写入 rtasr。
                uint32_t pcm_len = 0;
                uint8_t *pcm = sp_mic_read_alloc(mic_fd, &pcm_len);
                if (!pcm) {
                    continue;
                }
                int32_t wr = sp_rtasr_write(asr_fd, (int32_t)(uintptr_t)pcm, (int32_t)pcm_len);
                free(pcm);
                if (wr < 0 && wr != -EAGAIN) {
                    printf("rtasr_write failed: %d\n", wr);
                }
            }

            if (fd == asr_fd && (ev & SPEAR_EPOLLIN)) {
                // Read one JSON event.
                // 读一条 JSON 事件。
                uint32_t msg_len = 0;
                uint8_t *msg = sp_rtasr_read_alloc(asr_fd, &msg_len);
                if (!msg) {
                    continue;
                }
                const char *s = (const char *)msg;

                if (msg_len == 0 || s[0] != '{') {
                    printf("event_bytes=%u\n", msg_len);
                    free(msg);
                    continue;
                }

                char ty[128];
                if (!get_json_string_field_buf(s, "type", ty, sizeof(ty))) {
                    printf("event_bytes=%u\n", msg_len);
                    free(msg);
                    continue;
                }

                static int warned_stub = 0;
                if (!warned_stub && strncmp(ty, "transcription.", 13) == 0) {
                    warned_stub = 1;
                    printf("warning: rtasr appears to be using stub transport (text is fake 'a's)\n");
                    printf("hint: ensure Spearlet has a websocket speech_to_text backend and OPENAI_REALTIME_API_KEY is set\n");
                }

                if (strstr(ty, "transcription") == NULL) {
                    printf("event_type=%s\n", ty);
                    free(msg);
                    continue;
                }

                if (!print_json_string_field(s, "delta")) {
                    if (!print_json_string_field(s, "transcript")) {
                        if (!print_json_string_field(s, "text")) {
                            printf("event_type=%s\n", ty);
                        }
                    }
                }
                free(msg);
            }
        }
    }

out:
    sp_rtasr_close(asr_fd);
    sp_mic_close(mic_fd);
    sp_ep_close(epfd);
    return 0;
}
