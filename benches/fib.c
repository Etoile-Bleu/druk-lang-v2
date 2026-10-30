#include <stdio.h>

static long long fib(long long n) {
    if (n <= 1) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

int main(void) {
    printf("%lld\n", fib(33));
    return 0;
}
