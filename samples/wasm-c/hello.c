#include <stdio.h>
#include <stdint.h>

__attribute__((import_module("spear"), import_name("time_now_ms")))
extern int64_t time_now_ms(void);

int main() {
    printf("hello spear wasm\n");
    int64_t t = time_now_ms();
    printf("time_now_ms: %lld\n", (long long)t);
    return 0;
}
