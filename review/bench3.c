// Tuned scalar escape decoder: unaligned 64-bit window, no per-coordinate value state beyond the signed query weight and the depth-2 index,
// two accumulators. Same stream format and reference check as bench.c (a). Also: item-at-a-time progressive decode (resume across levels).
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>
#include <time.h>
static double now(){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+1e-9*t.tv_nsec;}
static void* rd(const char*f,size_t*n){FILE*p=fopen(f,"rb");fseek(p,0,SEEK_END);*n=ftell(p);rewind(p);void*b=malloc(*n);if(fread(b,1,*n,p)!=*n)exit(1);fclose(p);return b;}
#define N 768
#define W 0.6745f
static inline float fsgn(float x,uint64_t w){uint32_t u;memcpy(&u,&x,4);u^=((uint32_t)(~w&1))<<31;memcpy(&x,&u,4);return x;}
int main(){
  size_t sz; int*se=rd("b_se.bin",&sz); int ns=sz/4; int*sdep=rd("b_sdep.bin",&sz); float*U=rd("b_u.bin",&sz); int ni=sz/4/N;
  float*qs=rd("b_qs.bin",&sz); float*cent2=rd("b_cent2.bin",&sz);
  float wd[16]; for(int d=2;d<16;d++) wd[d]=W*powf(2.f,2-d);
  uint8_t*code=calloc((size_t)ni*40,1); float*ref=malloc(sizeof(float)*ni*4); int LV[4]={32,64,128,256};
  for(int i=0;i<ni;i++){ const float*u=U+(size_t)i*N; int pos=0; double est=0; int lv=0; static float val[N]; uint8_t*cw=code+(size_t)i*40;
    for(int c=0;c<N;c++) val[c]=0;
    for(int s=0;s<ns;s++){ int c=se[s],d=sdep[s]; float a=fabsf(u[c]); int nb; uint64_t bits; float nv;
      if(d==1){nb=1;bits=u[c]>=0;nv=0.7979f;}
      else if(d==2){int k=(int)(a/W); if(k>55)k=55; nb=k+1; bits=1ull<<k; nv=cent2[k];}
      else {int idx=(int)(a/wd[d]); if(d==3){int k=(int)(a/W); if(k>55){idx=2*55+1;}} nb=1; bits=idx&1; nv=(idx+0.5f)*wd[d];}
      while(lv<4 && pos+nb>LV[lv]){ref[i*4+lv]=est;lv++;}
      if(lv==4)break;
      for(int b=0;b<nb;b++){ if((bits>>b)&1) cw[(pos+b)>>3]|=1u<<((pos+b)&7);} pos+=nb;
      est+=qs[c]*(u[c]>=0?1:-1)*(nv-val[c]); val[c]=nv; }
    while(lv<4){ref[i*4+lv]=est;lv++;} }
  // static tables: d3 delta given k and bit; half-widths for d>=4
  static float d3[64][2]; for(int k=0;k<64;k++)for(int b=0;b<2;b++) d3[k][b]=(2*k+b+0.5f)*wd[3]-cent2[k<56?k:55];
  float hw[16]; for(int d=4;d<16;d++) hw[d]=0.5f*wd[d];
  float*out=malloc(sizeof(float)*ni*4); int reps=60;
  for(int L=0;L<4;L++){ int B=LV[L]; double best=1e9;
    for(int r=0;r<reps;r++){ double t0=now();
      for(int i=0;i<ni;i++){ const uint8_t*cw=code+(size_t)i*40; int pos=0; float e0=0,e1=0; float sg[N]; uint8_t ix[N];
        for(int s=0;s<ns;s++){ int c=se[s],d=sdep[s]; uint64_t w; memcpy(&w,cw+(pos>>3),8); w>>=(pos&7);
          if(d==1){ if(pos+1>B)break; float v=fsgn(qs[c],w); pos++; sg[c]=v; e0+=v*0.7979f; }
          else if(d==2){ int k=__builtin_ctzll(w|(1ull<<56)); if(pos+k+1>B)break; pos+=k+1; ix[c]=k; e1+=sg[c]*(cent2[k]-0.7979f); }
          else if(d==3){ if(pos+1>B)break; e0+=sg[c]*d3[ix[c]][w&1]; pos++; }
          else { if(pos+1>B)break; float h=hw[d]; e1+=fsgn(sg[c]*h,w); pos++; } }
        out[i*4+L]=e0+e1; }
      double t=now()-t0; if(t<best)best=t; }
    double err=0; for(int i=0;i<ni;i++){double e=fabs(out[i*4+L]-ref[i*4+L]); if(e>err)err=e;}
    printf("(a3) branch-free escape decode to %3d bits (%2d B): %.1f ns/item, %.2f ns/byte   max |decoded-reference| %.2g\n",B,B/8,1e9*best/ni,1e9*best/ni/(B/8),err); }
  return 0; }
