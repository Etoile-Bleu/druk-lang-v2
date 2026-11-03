#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long long n = 100000;
    long long *xs = malloc((size_t)n * sizeof(long long));
    if (!xs) {
        return 1;
    }

    for (long long i = 0; i < n; i++) {
        xs[i] = i;
    }

    long long total = 0;
    for (long long i = 0; i < n; i++) {
        total = total + xs[i];
    }

    printf("%lld\n", total);
    free(xs);
    return 0;
}
