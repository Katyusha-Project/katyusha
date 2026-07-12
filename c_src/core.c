#include "core.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <dirent.h>
#include <errno.h>

/* ---------------------------------------------------------------- */
/* Privileges                                                        */
/* ---------------------------------------------------------------- */

int k_is_root(void) {
    return geteuid() == 0 ? 1 : 0;
}

/* ---------------------------------------------------------------- */
/* mkdir -p                                                          */
/* ---------------------------------------------------------------- */

int k_mkdir_p(const char *path) {
    char tmp[4096];
    size_t len;
    char *p = NULL;

    if (!path) return -1;
    len = strlen(path);
    if (len == 0 || len >= sizeof(tmp)) return -1;

    memcpy(tmp, path, len + 1);
    if (tmp[len - 1] == '/') tmp[len - 1] = '\0';

    for (p = tmp + 1; *p; p++) {
        if (*p == '/') {
            *p = '\0';
            if (mkdir(tmp, 0755) != 0 && errno != EEXIST) return -1;
            *p = '/';
        }
    }
    if (mkdir(tmp, 0755) != 0 && errno != EEXIST) return -1;
    return 0;
}

/* ---------------------------------------------------------------- */
/* rm -rf                                                             */
/* ---------------------------------------------------------------- */

int k_rm_rf(const char *path) {
    struct stat st;
    DIR *dir;
    struct dirent *entry;
    char child[4096];

    if (!path) return -1;
    if (lstat(path, &st) != 0) return -1;

    if (!S_ISDIR(st.st_mode)) {
        return unlink(path) == 0 ? 0 : -1;
    }

    dir = opendir(path);
    if (!dir) return -1;

    while ((entry = readdir(dir)) != NULL) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0)
            continue;

        int n = snprintf(child, sizeof(child), "%s/%s", path, entry->d_name);
        if (n < 0 || (size_t)n >= sizeof(child)) {
            closedir(dir);
            return -1;
        }
        if (k_rm_rf(child) != 0) {
            closedir(dir);
            return -1;
        }
    }
    closedir(dir);
    return rmdir(path) == 0 ? 0 : -1;
}

/* ---------------------------------------------------------------- */
/* Archive extraction via fork/exec (no system()/popen())            */
/* ---------------------------------------------------------------- */

int k_extract_targz(const char *archive_path, const char *dest_dir) {
    if (!archive_path || !dest_dir) return -1;
    if (k_mkdir_p(dest_dir) != 0) return -1;

    pid_t pid = fork();
    if (pid < 0) return -1;

    if (pid == 0) {
        /* Child process: direct exec, no shell involved.
         * Plain "-xf" (no explicit -z/-j/-J) lets GNU tar
         * auto-detect the compression format from the archive's
         * magic bytes, so this transparently handles .tar.gz,
         * .tar.xz, .tar.bz2, and uncompressed .tar alike. */
        execlp("tar", "tar", "-xf", archive_path, "-C", dest_dir, (char *)NULL);
        _exit(127); /* only reached if execlp fails */
    }

    int status = 0;
    if (waitpid(pid, &status, 0) < 0) return -1;
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) return -1;
    return 0;
}

/* ---------------------------------------------------------------- */
/* SHA-256 (self-contained implementation, based on the FIPS 180-4   */
/* specification, no external dependencies)                          */
/* ---------------------------------------------------------------- */

typedef struct {
    unsigned int state[8];
    unsigned long long bitlen;
    unsigned char data[64];
    unsigned int datalen;
} sha256_ctx;

static const unsigned int K[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

#define ROTR(a,b) (((a) >> (b)) | ((a) << (32 - (b))))
#define CH(x,y,z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x,y,z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (ROTR(x,2) ^ ROTR(x,13) ^ ROTR(x,22))
#define EP1(x) (ROTR(x,6) ^ ROTR(x,11) ^ ROTR(x,25))
#define SIG0(x) (ROTR(x,7) ^ ROTR(x,18) ^ ((x) >> 3))
#define SIG1(x) (ROTR(x,17) ^ ROTR(x,19) ^ ((x) >> 10))

static void sha256_transform(sha256_ctx *ctx, const unsigned char data[]) {
    unsigned int a,b,c,d,e,f,g,h,i,j,t1,t2,m[64];

    for (i = 0, j = 0; i < 16; i++, j += 4)
        m[i] = ((unsigned int)data[j] << 24) | ((unsigned int)data[j+1] << 16) |
               ((unsigned int)data[j+2] << 8) | ((unsigned int)data[j+3]);
    for (; i < 64; i++)
        m[i] = SIG1(m[i-2]) + m[i-7] + SIG0(m[i-15]) + m[i-16];

    a = ctx->state[0]; b = ctx->state[1]; c = ctx->state[2]; d = ctx->state[3];
    e = ctx->state[4]; f = ctx->state[5]; g = ctx->state[6]; h = ctx->state[7];

    for (i = 0; i < 64; i++) {
        t1 = h + EP1(e) + CH(e,f,g) + K[i] + m[i];
        t2 = EP0(a) + MAJ(a,b,c);
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }

    ctx->state[0] += a; ctx->state[1] += b; ctx->state[2] += c; ctx->state[3] += d;
    ctx->state[4] += e; ctx->state[5] += f; ctx->state[6] += g; ctx->state[7] += h;
}

static void sha256_init(sha256_ctx *ctx) {
    ctx->datalen = 0;
    ctx->bitlen = 0;
    ctx->state[0]=0x6a09e667; ctx->state[1]=0xbb67ae85;
    ctx->state[2]=0x3c6ef372; ctx->state[3]=0xa54ff53a;
    ctx->state[4]=0x510e527f; ctx->state[5]=0x9b05688c;
    ctx->state[6]=0x1f83d9ab; ctx->state[7]=0x5be0cd19;
}

static void sha256_update(sha256_ctx *ctx, const unsigned char data[], unsigned long len) {
    for (unsigned long i = 0; i < len; i++) {
        ctx->data[ctx->datalen] = data[i];
        ctx->datalen++;
        if (ctx->datalen == 64) {
            sha256_transform(ctx, ctx->data);
            ctx->bitlen += 512;
            ctx->datalen = 0;
        }
    }
}

static void sha256_final(sha256_ctx *ctx, unsigned char hash[32]) {
    unsigned int i = ctx->datalen;

    if (ctx->datalen < 56) {
        ctx->data[i++] = 0x80;
        while (i < 56) ctx->data[i++] = 0x00;
    } else {
        ctx->data[i++] = 0x80;
        while (i < 64) ctx->data[i++] = 0x00;
        sha256_transform(ctx, ctx->data);
        memset(ctx->data, 0, 56);
    }

    ctx->bitlen += (unsigned long long)ctx->datalen * 8;
    ctx->data[63] = (unsigned char)(ctx->bitlen);
    ctx->data[62] = (unsigned char)(ctx->bitlen >> 8);
    ctx->data[61] = (unsigned char)(ctx->bitlen >> 16);
    ctx->data[60] = (unsigned char)(ctx->bitlen >> 24);
    ctx->data[59] = (unsigned char)(ctx->bitlen >> 32);
    ctx->data[58] = (unsigned char)(ctx->bitlen >> 40);
    ctx->data[57] = (unsigned char)(ctx->bitlen >> 48);
    ctx->data[56] = (unsigned char)(ctx->bitlen >> 56);
    sha256_transform(ctx, ctx->data);

    for (i = 0; i < 4; i++) {
        for (int j = 0; j < 8; j++) {
            hash[i + j*4] = (unsigned char)((ctx->state[j] >> (24 - i*8)) & 0xff);
        }
    }
}

int k_sha256_file(const char *path, char *out_hex, unsigned long out_hex_len) {
    if (!path || !out_hex || out_hex_len < 65) return -1;

    FILE *f = fopen(path, "rb");
    if (!f) return -1;

    sha256_ctx ctx;
    sha256_init(&ctx);

    unsigned char buf[8192];
    size_t n;
    while ((n = fread(buf, 1, sizeof(buf), f)) > 0) {
        sha256_update(&ctx, buf, (unsigned long)n);
    }
    if (ferror(f)) {
        fclose(f);
        return -1;
    }
    fclose(f);

    unsigned char hash[32];
    sha256_final(&ctx, hash);

    static const char *hexch = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out_hex[i*2]     = hexch[(hash[i] >> 4) & 0xf];
        out_hex[i*2 + 1] = hexch[hash[i] & 0xf];
    }
    out_hex[64] = '\0';
    return 0;
}

/* ---------------------------------------------------------------- */
/* File moving and permissions                                       */
/* ---------------------------------------------------------------- */

int k_move_file(const char *src, const char *dst) {
    if (!src || !dst) return -1;

    if (rename(src, dst) == 0) return 0;

    /* If it fails because it crosses filesystems (EXDEV), copy and delete */
    if (errno != EXDEV) return -1;

    FILE *in = fopen(src, "rb");
    if (!in) return -1;
    FILE *out = fopen(dst, "wb");
    if (!out) { fclose(in); return -1; }

    unsigned char buf[8192];
    size_t n;
    while ((n = fread(buf, 1, sizeof(buf), in)) > 0) {
        if (fwrite(buf, 1, n, out) != n) {
            fclose(in); fclose(out);
            return -1;
        }
    }
    fclose(in);
    fclose(out);
    unlink(src);
    return 0;
}

int k_make_executable(const char *path) {
    if (!path) return -1;
    return chmod(path, 0755) == 0 ? 0 : -1;
}
