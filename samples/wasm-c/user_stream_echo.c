#include <spear.h>

// User stream echo sample (WASM-C).
// 用户流回显示例（WASM-C）。
//
// Behavior:
// - Wait for a user stream connection via user_stream_ctl_open + epoll.
// - Open the stream as bidirectional.
// - For each inbound SSF frame, echo the same frame back to the outbound direction.
//
// 行为：
// - 通过 user_stream_ctl_open + epoll 等待 user stream 连接事件。
// - 以双向模式打开 stream。
// - 对每个收到的 inbound SSF 帧，原样写回 outbound，实现回显。

typedef struct {
    uint32_t stream_id;
    int32_t fd;
    uint8_t *pending;
    uint32_t pending_len;
} sp_stream_slot_t;

static int slot_find(sp_stream_slot_t *slots, int cap, uint32_t stream_id) {
    for (int i = 0; i < cap; i++) {
        if (slots[i].fd > 0 && slots[i].stream_id == stream_id) {
            return i;
        }
    }
    return -1;
}

static int slot_alloc(sp_stream_slot_t *slots, int cap) {
    for (int i = 0; i < cap; i++) {
        if (slots[i].fd <= 0) {
            slots[i].stream_id = 0;
            slots[i].fd = 0;
            slots[i].pending = NULL;
            slots[i].pending_len = 0;
            return i;
        }
    }
    return -1;
}

static void slot_close(sp_stream_slot_t *s) {
    if (s->pending) {
        free(s->pending);
        s->pending = NULL;
        s->pending_len = 0;
    }
    if (s->fd > 0) {
        (void)sp_user_stream_close(s->fd);
        s->fd = 0;
    }
    s->stream_id = 0;
}

static void slot_try_flush_pending(sp_stream_slot_t *s) {
    if (!s->pending || s->pending_len == 0 || s->fd <= 0) {
        return;
    }
    int32_t wr = sp_user_stream_write(s->fd, (int32_t)(uintptr_t)s->pending, (int32_t)s->pending_len);
    if (wr == 0) {
        free(s->pending);
        s->pending = NULL;
        s->pending_len = 0;
        return;
    }
    if (wr == -SPEAR_EAGAIN) {
        return;
    }
    printf("user_stream_write failed: %d\n", wr);
    free(s->pending);
    s->pending = NULL;
    s->pending_len = 0;
}

int main() {
    int32_t epfd = sp_ep_create();
    if (epfd < 0) {
        printf("ep_create failed: %d\n", epfd);
        return 1;
    }

    int32_t ctl_fd = sp_user_stream_ctl_open();
    if (ctl_fd < 0) {
        printf("user_stream_ctl_open failed: %d\n", ctl_fd);
        sp_ep_close(epfd);
        return 1;
    }

    int32_t rc = sp_ep_ctl(epfd, SPEAR_EP_CTL_ADD, ctl_fd,
                           SPEAR_EPOLLIN | SPEAR_EPOLLERR | SPEAR_EPOLLHUP);
    if (rc != 0) {
        printf("ep_ctl add ctl_fd failed: %d\n", rc);
        sp_user_stream_close(ctl_fd);
        sp_ep_close(epfd);
        return 1;
    }

    printf("user_stream_echo started (waiting for user streams)\n");

    sp_stream_slot_t slots[16];
    memset(slots, 0, sizeof(slots));

    uint8_t ready_buf[8 * 64];
    while (1) {
        uint32_t ready_len = sizeof(ready_buf);
        int32_t nready = sp_ep_wait(epfd, (int32_t)(uintptr_t)ready_buf,
                                    (int32_t)(uintptr_t)&ready_len, 2000);
        if (nready < 0) {
            printf("ep_wait failed: %d\n", nready);
            break;
        }
        if (nready == 0) {
            for (int i = 0; i < (int)(sizeof(slots) / sizeof(slots[0])); i++) {
                slot_try_flush_pending(&slots[i]);
            }
            continue;
        }

        for (int i = 0; i < nready; i++) {
            int32_t fd = 0;
            int32_t ev = 0;
            memcpy(&fd, ready_buf + (i * 8), 4);
            memcpy(&ev, ready_buf + (i * 8) + 4, 4);

            if (ev & SPEAR_EPOLLHUP) {
                if (fd == ctl_fd) {
                    printf("ctl hup\n");
                    goto out;
                }
                for (int j = 0; j < (int)(sizeof(slots) / sizeof(slots[0])); j++) {
                    if (slots[j].fd == fd) {
                        printf("stream hup: stream_id=%u fd=%d\n", slots[j].stream_id, fd);
                        slot_close(&slots[j]);
                    }
                }
                continue;
            }
            if (ev & SPEAR_EPOLLERR) {
                if (fd == ctl_fd) {
                    printf("ctl err\n");
                    goto out;
                }
                for (int j = 0; j < (int)(sizeof(slots) / sizeof(slots[0])); j++) {
                    if (slots[j].fd == fd) {
                        printf("stream err: stream_id=%u fd=%d\n", slots[j].stream_id, fd);
                        slot_close(&slots[j]);
                    }
                }
                continue;
            }

            if (fd == ctl_fd && (ev & SPEAR_EPOLLIN)) {
                while (1) {
                    sp_user_stream_ctl_event_t evt;
                    int32_t rr = sp_user_stream_ctl_read_event(ctl_fd, &evt);
                    if (rr == -SPEAR_EAGAIN) {
                        break;
                    }
                    if (rr < 0) {
                        printf("user_stream_ctl_read failed: %d\n", rr);
                        goto out;
                    }

                    if (evt.kind == SPEAR_USER_STREAM_CTL_EVENT_SESSION_CLOSED) {
                        printf("session closed\n");
                        goto out;
                    }

                    if (evt.kind != SPEAR_USER_STREAM_CTL_EVENT_STREAM_CONNECTED) {
                        printf("unknown ctl event: kind=%u stream_id=%u\n", evt.kind, evt.stream_id);
                        continue;
                    }

                    if (slot_find(slots, (int)(sizeof(slots) / sizeof(slots[0])), evt.stream_id) >= 0) {
                        continue;
                    }

                    int slot = slot_alloc(slots, (int)(sizeof(slots) / sizeof(slots[0])));
                    if (slot < 0) {
                        printf("too many streams, drop stream_id=%u\n", evt.stream_id);
                        continue;
                    }

                    int32_t sfd = sp_user_stream_open((int32_t)evt.stream_id, SPEAR_USER_STREAM_DIR_BIDIRECTIONAL);
                    if (sfd < 0) {
                        printf("user_stream_open failed: %d (stream_id=%u)\n", sfd, evt.stream_id);
                        continue;
                    }

                    slots[slot].stream_id = evt.stream_id;
                    slots[slot].fd = sfd;

                    int32_t add_rc = sp_ep_ctl(epfd, SPEAR_EP_CTL_ADD, sfd,
                                              SPEAR_EPOLLIN | SPEAR_EPOLLOUT | SPEAR_EPOLLERR | SPEAR_EPOLLHUP);
                    if (add_rc != 0) {
                        printf("ep_ctl add stream fd failed: %d\n", add_rc);
                        slot_close(&slots[slot]);
                        continue;
                    }

                    printf("stream connected: stream_id=%u fd=%d\n", evt.stream_id, sfd);
                }
                continue;
            }

            for (int j = 0; j < (int)(sizeof(slots) / sizeof(slots[0])); j++) {
                sp_stream_slot_t *s = &slots[j];
                if (s->fd != fd || s->fd <= 0) {
                    continue;
                }

                if ((ev & SPEAR_EPOLLOUT) != 0) {
                    slot_try_flush_pending(s);
                }

                if ((ev & SPEAR_EPOLLIN) == 0) {
                    continue;
                }

                uint32_t frame_len = 0;
                uint8_t *frame = sp_user_stream_read_alloc(s->fd, &frame_len);
                if (!frame) {
                    continue;
                }

                int32_t wr = sp_user_stream_write(s->fd, (int32_t)(uintptr_t)frame, (int32_t)frame_len);
                if (wr == 0) {
                    free(frame);
                    frame = NULL;
                    continue;
                }

                if (wr == -SPEAR_EAGAIN) {
                    if (!s->pending) {
                        s->pending = frame;
                        s->pending_len = frame_len;
                        frame = NULL;
                        continue;
                    }
                    free(frame);
                    frame = NULL;
                    continue;
                }

                printf("user_stream_write failed: %d\n", wr);
                free(frame);
                frame = NULL;
            }
        }
    }

out:
    for (int i = 0; i < (int)(sizeof(slots) / sizeof(slots[0])); i++) {
        slot_close(&slots[i]);
    }
    (void)sp_user_stream_close(ctl_fd);
    (void)sp_ep_close(epfd);
    return 0;
}
