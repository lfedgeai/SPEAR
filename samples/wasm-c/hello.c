#include <spear.h>

int main() {
    printf("hello spear wasm\n");
    int64_t t = sp_time_now_ms();
    printf("time_now_ms: %lld\n", (long long)t);
    return 0;
}
