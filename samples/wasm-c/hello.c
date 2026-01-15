#include <spear.h>

// Minimal WASM-C sample.
// 最小 WASM-C 示例。

int main() {
    printf("hello spear wasm\n");
    int64_t t = sp_time_now_ms();
    printf("time_now_ms: %lld\n", (long long)t);
    return 0;
}
