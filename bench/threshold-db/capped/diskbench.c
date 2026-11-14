// Read latency and throughput of a file with O_DIRECT (page cache bypassed).
// usage: diskbench FILE BLOCK_BYTES rand|seq THREADS SECONDS
// Each thread issues one pread at a time, so THREADS is the queue depth.
#define _GNU_SOURCE
#include <fcntl.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static const char *path; static size_t bs; static int rnd; static double secs; static off_t fsize; static int nthr;
typedef struct { int id; double *lat; size_t n, cap; } T;

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }

static void *run(void *arg) {
    T *t = arg;
    int fd = open(path, O_RDONLY | O_DIRECT);
    if (fd < 0) { perror("open"); exit(1); }
    void *buf; posix_memalign(&buf, 4096, bs);
    uint64_t s = 0x9E3779B97F4A7C15ull * (t->id + 1);
    off_t nblk = fsize / bs, pos = (nblk / nthr) * t->id;
    double end = now() + secs;
    while (1) {
        double a = now();
        if (a > end) break;
        off_t o;
        if (rnd) { s ^= s << 13; s ^= s >> 7; s ^= s << 17; o = (off_t)(s % nblk) * bs; }
        else { o = pos * bs; pos = (pos + 1) % nblk; }
        if (pread(fd, buf, bs, o) != (ssize_t)bs) { perror("pread"); exit(1); }
        double b = now();
        if (t->n == t->cap) { t->cap = t->cap ? 2 * t->cap : 1 << 16; t->lat = realloc(t->lat, t->cap * sizeof(double)); }
        t->lat[t->n++] = b - a;
    }
    close(fd);
    return 0;
}
static int cmp(const void *a, const void *b) { double x = *(double *)a, y = *(double *)b; return x < y ? -1 : x > y; }

int main(int argc, char **argv) {
    if (argc != 6) { fprintf(stderr, "usage: diskbench FILE BLOCK_BYTES rand|seq THREADS SECONDS\n"); return 2; }
    path = argv[1]; bs = atol(argv[2]); rnd = !strcmp(argv[3], "rand"); nthr = atoi(argv[4]); secs = atof(argv[5]);
    struct stat st; stat(path, &st); fsize = st.st_size;
    pthread_t th[256]; T ts[256]; memset(ts, 0, sizeof ts);
    double t0 = now();
    for (int i = 0; i < nthr; i++) { ts[i].id = i; pthread_create(&th[i], 0, run, &ts[i]); }
    for (int i = 0; i < nthr; i++) pthread_join(th[i], 0);
    double el = now() - t0;
    size_t n = 0; for (int i = 0; i < nthr; i++) n += ts[i].n;
    double *all = malloc(n * sizeof(double)); size_t k = 0; double sum = 0;
    for (int i = 0; i < nthr; i++) for (size_t j = 0; j < ts[i].n; j++) { all[k++] = ts[i].lat[j]; sum += ts[i].lat[j]; }
    qsort(all, n, sizeof(double), cmp);
    printf("%s %zu B x %d threads: %zu reads in %.1fs, %.0f IOPS, %.1f MB/s; latency mean %.1f us, p50 %.1f, p90 %.1f, p99 %.1f\n",
           argv[3], bs, nthr, n, el, n / el, n * bs / el / 1e6, sum / n * 1e6, all[n / 2] * 1e6, all[n * 9 / 10] * 1e6, all[n * 99 / 100] * 1e6);
    return 0;
}
