// Micro-benchmark (single thread): per-item decode costs behind the byte counts.
// (a) escape code: bit-serial decode with incremental dot, levels 4/8/16/32 B (item-at-a-time, state kept across levels)
// (b) byte-LUT accumulate over a fixed-rate code (one table lookup + add per byte)   (c) f16 full-vector dot, n=768
// (d) int8 centre dot, n=768   (e) 64 local bases, float32 768x768 matvec   (f) nibble-LUT (pshufb, 4-bit) accumulate, 32 B code
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>
#include <time.h>
#include <immintrin.h>
static double now(){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+1e-9*t.tv_nsec;}
static void* rd(const char*f,size_t*n){FILE*p=fopen(f,"rb");fseek(p,0,SEEK_END);*n=ftell(p);rewind(p);void*b=malloc(*n);if(fread(b,1,*n,p)!=*n)exit(1);fclose(p);return b;}
#define N 768
#define W 0.6745f
#define MAXB 256
int main(){
  size_t sz; int*se=rd("b_se.bin",&sz); int ns=sz/4; int*sdep=rd("b_sdep.bin",&sz); float*U=rd("b_u.bin",&sz); int ni=sz/4/N;
  float*qs=rd("b_qs.bin",&sz); float*cent2=rd("b_cent2.bin",&sz);
  float wd[16]; for(int d=2;d<16;d++) wd[d]=W*powf(2.f,2-d);
  // ---- encode every item to a 256-bit stream, nested (the first B bits are the B-bit code)
  uint64_t*code=calloc((size_t)ni*4,8); float*ref=malloc(sizeof(float)*ni*4); int LV[4]={32,64,128,256};
  for(int i=0;i<ni;i++){ const float*u=U+(size_t)i*N; int pos=0; double est=0; int lv=0; static float val[N];
    for(int c=0;c<N;c++) val[c]=0;
    for(int s=0;s<ns;s++){ int c=se[s],d=sdep[s]; float a=fabsf(u[c]); int nb; uint64_t bits; float nv;
      if(d==1){nb=1;bits=u[c]>=0;nv=0.7979f;}
      else if(d==2){int k=(int)(a/W); if(k>60)k=60; nb=k+1; bits=1ull<<k; nv=cent2[k];}      // k zeros then a one (LSB first)
      else {int idx=(int)(a/wd[d]); nb=1; bits=idx&1; nv=(idx+0.5f)*wd[d];}
      while(lv<4 && pos+nb>LV[lv]){ref[i*4+lv]=est;lv++;}
      if(lv==4)break;
      for(int b=0;b<nb;b++){ if((bits>>b)&1) code[(size_t)i*4+((pos+b)>>6)]|=1ull<<((pos+b)&63);} pos+=nb;
      est+=qs[c]*(u[c]>=0?1:-1)*(nv-val[c]); val[c]=nv; }
    while(lv<4){ref[i*4+lv]=est;lv++;} }
  // ---- (a) decode
  float*out=malloc(sizeof(float)*ni*4); int reps=40; double best[4]={1e9,1e9,1e9,1e9};
  for(int L=0;L<4;L++){ int B=LV[L];
    for(int r=0;r<reps;r++){ double t0=now();
      for(int i=0;i<ni;i++){ const uint64_t*cw=code+(size_t)i*4; int pos=0; float est=0; float sg[N]; int ix[N]; float val[N];
        for(int s=0;s<ns;s++){ int c=se[s],d=sdep[s];
          if(d==1){ if(pos+1>B)break; int b=(cw[pos>>6]>>(pos&63))&1; pos++; sg[c]=b?qs[c]:-qs[c]; val[c]=0.7979f; est+=sg[c]*0.7979f; }
          else if(d==2){ uint64_t w=cw[pos>>6]>>(pos&63); if((pos&63)&&(pos>>6)<3) w|=cw[(pos>>6)+1]<<(64-(pos&63)); int k=w?__builtin_ctzll(w):64;
            if(pos+k+1>B)break; pos+=k+1; ix[c]=k; float nv=cent2[k]; est+=sg[c]*(nv-val[c]); val[c]=nv; }
          else { if(pos+1>B)break; int b=(cw[pos>>6]>>(pos&63))&1; pos++; ix[c]=2*ix[c]+b; float nv=(ix[c]+0.5f)*wd[d]; est+=sg[c]*(nv-val[c]); val[c]=nv; } }
        out[i*4+L]=est; }
      double t=now()-t0; if(t<best[L])best[L]=t; }
    double err=0; for(int i=0;i<ni;i++){double e=fabs(out[i*4+L]-ref[i*4+L]); if(e>err)err=e;}
    printf("(a) escape decode to %3d bits (%2d B): %.1f ns/item, %.2f ns/byte   max |decoded-reference| %.2g\n",B,B/8,1e9*best[L]/ni,1e9*best[L]/ni/(B/8),err); }
  // ---- (b) byte LUT: 32 lookups per 32 B item, tables 32x256 floats (8 KB... 32 KB)
  { int nb=32; float*lut=malloc(sizeof(float)*nb*256); for(int i=0;i<nb*256;i++)lut[i]=(float)rand()/RAND_MAX; int M=1<<20; uint8_t*cd=malloc((size_t)M*nb); for(size_t i=0;i<(size_t)M*nb;i++)cd[i]=rand();
    float*o=malloc(sizeof(float)*M); double bt=1e9; for(int r=0;r<8;r++){double t0=now(); for(int i=0;i<M;i++){const uint8_t*c=cd+(size_t)i*nb; float s=0; for(int j=0;j<nb;j++) s+=lut[j*256+c[j]]; o[i]=s;} double t=now()-t0; if(t<bt)bt=t;}
    printf("(b) byte-LUT accumulate, 32 B fixed-rate code, 1M items: %.1f ns/item, %.2f ns/byte  (chk %g)\n",1e9*bt/M,1e9*bt/M/nb,o[7]);
    // ---- (f) nibble LUT with pshufb, 32 codes at a time (4-bit tables quantised to u8, sums in u16), 32 B code = 64 nibbles
    uint8_t*t4=aligned_alloc(64,64*32); for(int i=0;i<64*32;i++)t4[i]=rand()&15; uint8_t*blk=aligned_alloc(64,(size_t)(M/32)*64*32); for(size_t i=0;i<(size_t)(M/32)*64*32;i++)blk[i]=rand()&15;
    uint16_t*o2=aligned_alloc(64,sizeof(uint16_t)*M); bt=1e9;
    for(int r=0;r<8;r++){double t0=now(); for(int g=0;g<M/32;g++){ __m256i acc0=_mm256_setzero_si256(),acc1=_mm256_setzero_si256(); const uint8_t*b=blk+(size_t)g*64*32;
        for(int j=0;j<64;j++){ __m256i tb=_mm256_broadcastsi128_si256(_mm_load_si128((const __m128i*)(t4+j*32))); __m256i v=_mm256_shuffle_epi8(tb,_mm256_load_si256((const __m256i*)(b+j*32)));
          acc0=_mm256_add_epi16(acc0,_mm256_cvtepu8_epi16(_mm256_castsi256_si128(v))); acc1=_mm256_add_epi16(acc1,_mm256_cvtepu8_epi16(_mm256_extracti128_si256(v,1))); }
        _mm256_store_si256((__m256i*)(o2+g*32),acc0); _mm256_store_si256((__m256i*)(o2+g*32+16),acc1);} double t=now()-t0; if(t<bt)bt=t;}
    printf("(f) nibble-LUT (pshufb, 32 items per block), 32 B code, 1M items: %.2f ns/item, %.3f ns/byte  (chk %d)\n",1e9*bt/M,1e9*bt/M/32,o2[5]); }
  // ---- (c) f16 full vector dot
  { int M=40000; uint16_t*X=aligned_alloc(64,(size_t)M*N*2); for(size_t i=0;i<(size_t)M*N;i++)X[i]=0x3000+(rand()&0x3ff); float q[N]; for(int i=0;i<N;i++)q[i]=(float)rand()/RAND_MAX; float*o=malloc(4*M); double bt=1e9;
    for(int r=0;r<10;r++){double t0=now(); for(int i=0;i<M;i++){ __m512 s=_mm512_setzero_ps(); const uint16_t*x=X+(size_t)i*N; for(int j=0;j<N;j+=16) s=_mm512_fmadd_ps(_mm512_cvtph_ps(_mm256_loadu_si256((const __m256i*)(x+j))),_mm512_loadu_ps(q+j),s); o[i]=_mm512_reduce_add_ps(s);} double t=now()-t0; if(t<bt)bt=t;}
    printf("(c) f16 full-vector dot n=768 (sequential, 61 MB): %.1f ns/vector, %.3f ns/byte  (chk %g)\n",1e9*bt/M,1e9*bt/M/(2*N),o[3]);
    // random order (cache-missing) full reads
    int*perm=malloc(4*M); for(int i=0;i<M;i++)perm[i]=rand()%M; bt=1e9;
    for(int r=0;r<10;r++){double t0=now(); for(int i=0;i<M;i++){ __m512 s=_mm512_setzero_ps(); const uint16_t*x=X+(size_t)perm[i]*N; for(int j=0;j<N;j+=16) s=_mm512_fmadd_ps(_mm512_cvtph_ps(_mm256_loadu_si256((const __m256i*)(x+j))),_mm512_loadu_ps(q+j),s); o[i]=_mm512_reduce_add_ps(s);} double t=now()-t0; if(t<bt)bt=t;}
    printf("(c') same, random order: %.1f ns/vector\n",1e9*bt/M); }
  // ---- (d) int8 centre dot with int16 query (vpdpwssd after widening)
  { int M=200000; int8_t*C=aligned_alloc(64,(size_t)M*N); for(size_t i=0;i<(size_t)M*N;i++)C[i]=rand(); int16_t q[N]; for(int i=0;i<N;i++)q[i]=rand(); int*o=malloc(4*M); double bt=1e9;
    for(int r=0;r<10;r++){double t0=now(); for(int i=0;i<M;i++){ __m512i s=_mm512_setzero_si512(); const int8_t*x=C+(size_t)i*N; for(int j=0;j<N;j+=32) s=_mm512_dpwssd_epi32(s,_mm512_cvtepi8_epi16(_mm256_loadu_si256((const __m256i*)(x+j))),_mm512_loadu_si512(q+j)); o[i]=_mm512_reduce_add_epi32(s);} double t=now()-t0; if(t<bt)bt=t;}
    printf("(d) int8 centre . int16 query, n=768 (154 MB): %.1f ns/centre, %.3f ns/byte  (chk %d)\n",1e9*bt/M,1e9*bt/M/N,o[3]); }
  // ---- (e) 64 float32 bases matvec, and the same with int8 bases
  { int G=64; float*Bs=aligned_alloc(64,(size_t)G*N*N*4); for(size_t i=0;i<(size_t)G*N*N;i++)Bs[i]=(float)rand()/RAND_MAX-0.5f; float q[N],o[N]; for(int i=0;i<N;i++)q[i]=(float)rand()/RAND_MAX; double bt=1e9; float chk=0;
    for(int r=0;r<6;r++){double t0=now(); for(int g=0;g<G;g++){ const float*B=Bs+(size_t)g*N*N; for(int i=0;i<N;i++){ __m512 s=_mm512_setzero_ps(); for(int j=0;j<N;j+=16) s=_mm512_fmadd_ps(_mm512_loadu_ps(B+(size_t)i*N+j),_mm512_loadu_ps(q+j),s); o[i]=_mm512_reduce_add_ps(s);} chk+=o[5];} double t=now()-t0; if(t<bt)bt=t;}
    printf("(e) 64 float32 bases x query (151 MB): %.1f ms  (%.2f GB/s)  (chk %g)\n",1e3*bt,G*(double)N*N*4/bt/1e9,chk);
    int8_t*B8=aligned_alloc(64,(size_t)G*N*N); for(size_t i=0;i<(size_t)G*N*N;i++)B8[i]=rand(); int16_t q16[N]; for(int i=0;i<N;i++)q16[i]=rand(); int oi[N]; bt=1e9; int c2=0;
    for(int r=0;r<6;r++){double t0=now(); for(int g=0;g<G;g++){ const int8_t*B=B8+(size_t)g*N*N; for(int i=0;i<N;i++){ __m512i s=_mm512_setzero_si512(); for(int j=0;j<N;j+=32) s=_mm512_dpwssd_epi32(s,_mm512_cvtepi8_epi16(_mm256_loadu_si256((const __m256i*)(B+(size_t)i*N+j))),_mm512_loadu_si512(q16+j)); oi[i]=_mm512_reduce_add_epi32(s);} c2+=oi[5];} double t=now()-t0; if(t<bt)bt=t;}
    printf("(e') 64 int8 bases x int16 query (38 MB): %.1f ms  (chk %d)\n",1e3*bt,c2); }
  return 0; }
