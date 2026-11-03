#include <stdio.h>

int main(void) {
    long long total = 0;
    long long i = 0;
    while (i < 10000000) {
        total = total + i;
        i = i + 1;
    }
    printf("%lld\n", total);
    return 0;
}
