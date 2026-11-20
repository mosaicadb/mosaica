
### glove, visible set none, k = 10 (100 queries; K 592, n 128; the centre scan is 0.08 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 22.25 (34; 0; 86,297) | 41.47 (66; 0; 161,096) | - | - |
| direct, the library's stop | 28.72 (46; 0; 111,451) | 46.52 (75; 0; 180,753) | - | - |
| certified levels, the library's stop | 7.60 (46; 111,451; 13,137) | 10.68 (75; 180,751; 15,916) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 2.89 (47; 113,115; 279) | 4.61 (77; 183,821; 408) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 2.89 (46; 112,755; 298) | 4.60 (76; 182,291; 455) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 2.89 (47; 113,266; 273) | 4.61 (77; 183,821; 397) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 2.88 (46; 112,755; 291) | 4.59 (76; 182,291; 443) | - | - |
| statistical (rho measured), one dial, levels 16 B | 3.37 (51; 122,557; 2,025) | 5.52 (80; 190,170; 4,556) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 3.54 (84; 129,122; 481) | 5.10 (107; 192,963; 636) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 46.27 / 0.00 / 0.00 / 0.17 | - |
| certified | 5.57 / 4.07 / 0.32 / 0.46 / 0.17 | - |
| statistical, one dial | 3.46 / 0.10 / 0.33 / 0.47 / 0.17 | - |

rho measured per level [0.0994, 0.0992]; 1 / sqrt(d) = 0.1000

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.3520 | 0.18 | 1.0 | 2,633 | 2,633 | 2,633 | 46 | 40 |
| 2.5 | 0.4050 | 0.20 | 1.3 | 3,415 | 3,415 | 3,415 | 129 | 59 |
| 3 | 0.5890 | 0.41 | 4.3 | 11,359 | 11,359 | 11,359 | 390 | 109 |
| 3.5 | 0.7980 | 1.13 | 15.7 | 40,101 | 40,101 | 40,101 | 949 | 186 |
| 4 | 0.9270 | 3.35 | 54.8 | 132,633 | 132,633 | 132,633 | 2,143 | 296 |
| 4.5 | 0.9610 | 5.21 | 87.3 | 208,304 | 208,302 | 208,302 | 4,107 | 446 |
| 5 | 0.9760 | 6.49 | 109.2 | 258,180 | 258,169 | 258,169 | 7,346 | 657 |
| 6 | 0.9820 | 8.47 | 138.4 | 324,658 | 324,523 | 324,523 | 20,189 | 1,383 |
| 7 | 0.9830 | 9.96 | 152.0 | 355,482 | 355,133 | 355,133 | 45,307 | 2,790 |
| 8 | 0.9830 | 11.56 | 158.0 | 368,683 | 367,728 | 367,728 | 83,788 | 5,345 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3670 | 0.76 | 1.0 | 2,593 |
| 2 | 0.3710 | 0.77 | 1.0 | 2,633 |
| 2.5 | 0.4120 | 0.97 | 1.3 | 3,415 |
| 3 | 0.5940 | 3.01 | 4.3 | 11,359 |
| 3.5 | 0.8020 | 10.33 | 15.7 | 39,870 |
| 4 | 0.9290 | 34.16 | 54.8 | 132,633 |
| 4.5 | 0.9620 | 53.58 | 87.2 | 208,250 |
| 5 | 0.9760 | 66.41 | 109.2 | 258,180 |
| 6 | 0.9820 | 83.49 | 138.4 | 324,658 |
| 7 | 0.9830 | 91.41 | 152.0 | 355,482 |
| 8 | 0.9830 | 94.80 | 158.0 | 368,683 |

### glove, visible set none, k = 100 (100 queries; K 592, n 128; the centre scan is 0.08 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 29.27 (46; 0; 113,613) | 53.39 (86; 0; 207,519) | - | - |
| direct, the library's stop | 28.39 (45; 0; 110,157) | 39.41 (64; 0; 153,077) | - | - |
| certified levels, the library's stop | 12.40 (45; 110,157; 31,223) | 16.09 (64; 153,077; 39,172) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 3.23 (47; 113,068; 1,370) | 4.54 (69; 164,425; 1,645) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 3.25 (49; 117,701; 1,111) | 4.48 (64; 153,960; 2,140) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 3.23 (47; 113,240; 1,350) | 4.55 (69; 165,036; 1,622) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 3.25 (46; 112,182; 1,482) | 4.47 (64; 153,960; 2,100) | - | - |
| statistical (rho measured), one dial, levels 16 B | 5.25 (62; 147,465; 7,180) | 7.40 (90; 213,470; 9,830) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 3.99 (81; 130,956; 1,819) | 4.92 (99; 165,200; 2,080) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 39.19 / 0.00 / 0.00 / 0.15 | - |
| certified | 5.15 / 10.03 / 0.29 / 0.39 / 0.15 | - |
| statistical, one dial | 3.17 / 0.42 / 0.31 / 0.42 / 0.16 | - |

rho measured per level [0.0994, 0.0992]; 1 / sqrt(d) = 0.1000

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.4513 | 0.34 | 2.0 | 5,065 | 5,065 | 5,065 | 533 | 414 |
| 2.5 | 0.6391 | 0.67 | 5.8 | 15,282 | 15,282 | 15,282 | 1,388 | 678 |
| 3 | 0.8250 | 1.60 | 19.3 | 48,986 | 48,986 | 48,986 | 3,019 | 1,042 |
| 3.5 | 0.9466 | 4.24 | 63.8 | 153,163 | 153,163 | 153,163 | 6,176 | 1,542 |
| 4 | 0.9723 | 6.58 | 102.8 | 242,909 | 242,908 | 242,908 | 10,804 | 2,144 |
| 4.5 | 0.9777 | 8.19 | 126.9 | 297,987 | 297,983 | 297,983 | 17,696 | 2,928 |
| 5 | 0.9794 | 9.31 | 139.4 | 326,849 | 326,840 | 326,840 | 27,650 | 3,963 |
| 6 | 0.9799 | 11.45 | 154.9 | 361,707 | 361,664 | 361,664 | 59,194 | 7,082 |
| 7 | 0.9799 | 13.78 | 159.5 | 371,601 | 371,511 | 371,511 | 105,946 | 12,251 |
| 8 | 0.9799 | 16.77 | 160.0 | 372,735 | 372,632 | 372,632 | 161,523 | 20,267 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3221 | 0.76 | 1.0 | 2,593 |
| 2 | 0.4452 | 1.35 | 1.9 | 4,913 |
| 2.5 | 0.6423 | 3.98 | 5.7 | 15,150 |
| 3 | 0.8287 | 12.66 | 19.3 | 48,953 |
| 3.5 | 0.9501 | 39.43 | 63.8 | 153,163 |
| 4 | 0.9726 | 62.49 | 102.8 | 242,909 |
| 4.5 | 0.9779 | 76.64 | 126.9 | 297,987 |
| 5 | 0.9795 | 84.05 | 139.4 | 326,849 |
| 6 | 0.9799 | 93.01 | 154.9 | 361,707 |
| 7 | 0.9799 | 95.55 | 159.5 | 371,601 |
| 8 | 0.9799 | 95.84 | 160.0 | 372,735 |

### glove, visible set random-1, k = 10 (100 queries; K 592, n 128; the centre scan is 0.08 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 0.90 (90; 0; 2,422) | 1.44 (156; 0; 4,004) | - | - |
| direct, the library's stop | 0.73 (74; 0; 1,921) | 1.17 (127; 0; 3,192) | - | - |
| certified levels, the library's stop | 0.73 (74; 0; 1,921) | 1.17 (127; 0; 3,192) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 0.73 (74; 0; 1,921) | 1.17 (127; 0; 3,192) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 0.73 (74; 0; 1,921) | 1.17 (127; 0; 3,192) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 0.73 (74; 0; 1,921) | 1.17 (127; 0; 3,192) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 0.73 (74; 0; 1,921) | 1.17 (127; 0; 3,192) | - | - |
| statistical (rho measured), one dial, levels 16 B | 0.92 (96; 0; 2,468) | 1.20 (131; 0; 3,286) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 0.61 (99; 0; 1,233) | 1.27 (172; 0; 3,218) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 0.82 / 0.00 / 0.00 / 0.28 | - |
| certified | 0.00 / 0.82 / 0.00 / 0.00 / 0.28 | - |
| statistical, one dial | 0.00 / 0.82 / 0.00 / 0.00 / 0.28 | - |

rho measured per level [0.0994, 0.1001]; 1 / sqrt(d) = 0.1000

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.6620 | 0.21 | 12.1 | 349 | 318 | 0 | 0 | 349 |
| 2.5 | 0.8320 | 0.41 | 35.0 | 966 | 936 | 0 | 0 | 966 |
| 3 | 0.9390 | 0.92 | 95.7 | 2,468 | 2,437 | 0 | 0 | 2,468 |
| 3.5 | 0.9680 | 1.58 | 178.9 | 4,377 | 4,346 | 0 | 0 | 4,377 |
| 4 | 0.9760 | 1.87 | 215.0 | 5,219 | 5,188 | 0 | 0 | 5,219 |
| 4.5 | 0.9780 | 2.03 | 234.7 | 5,677 | 5,646 | 0 | 0 | 5,677 |
| 5 | 0.9780 | 2.14 | 249.1 | 6,009 | 5,978 | 0 | 0 | 6,009 |
| 6 | 0.9780 | 2.19 | 255.7 | 6,164 | 6,133 | 0 | 0 | 6,163 |
| 7 | 0.9780 | 2.20 | 256.0 | 6,170 | 6,139 | 0 | 0 | 6,170 |
| 8 | 0.9780 | 2.20 | 256.0 | 6,170 | 6,139 | 0 | 0 | 6,170 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3600 | 0.11 | 2.2 | 64 |
| 2 | 0.6620 | 0.21 | 12.1 | 349 |
| 2.5 | 0.8320 | 0.41 | 35.0 | 966 |
| 3 | 0.9390 | 0.92 | 95.7 | 2,468 |
| 3.5 | 0.9680 | 1.58 | 178.9 | 4,377 |
| 4 | 0.9760 | 1.87 | 215.0 | 5,219 |
| 4.5 | 0.9780 | 2.03 | 234.7 | 5,677 |
| 5 | 0.9780 | 2.14 | 249.1 | 6,009 |
| 6 | 0.9780 | 2.19 | 255.7 | 6,164 |
| 7 | 0.9780 | 2.20 | 256.0 | 6,170 |
| 8 | 0.9780 | 2.20 | 256.0 | 6,170 |

### glove, visible set random-25, k = 10 (100 queries; K 592, n 128; the centre scan is 0.08 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 7.37 (44; 0; 28,081) | 13.19 (81; 0; 50,509) | - | - |
| direct, the library's stop | 6.07 (37; 0; 23,046) | 11.41 (72; 0; 43,615) | - | - |
| certified levels, the library's stop | 2.72 (37; 23,046; 5,332) | 4.25 (72; 43,615; 7,176) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 1.08 (37; 23,421; 168) | 1.93 (75; 45,072; 226) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.08 (37; 23,372; 172) | 1.92 (74; 44,600; 253) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.08 (37; 23,442; 165) | 1.93 (75; 45,072; 222) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 1.08 (37; 23,372; 169) | 1.92 (74; 44,600; 247) | - | - |
| statistical (rho measured), one dial, levels 16 B | 1.79 (69; 41,221; 1,166) | 2.23 (88; 52,406; 1,428) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 1.45 (85; 26,526; 277) | 1.97 (108; 38,985; 340) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 11.17 / 0.00 / 0.00 / 0.16 | - |
| certified | 1.42 / 1.84 / 0.31 / 0.44 / 0.16 | - |
| statistical, one dial | 0.85 / 0.06 / 0.31 / 0.46 / 0.17 | - |

rho measured per level [0.0993, 0.0993]; 1 / sqrt(d) = 0.1000

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.3830 | 0.15 | 1.2 | 808 | 808 | 808 | 39 | 40 |
| 2.5 | 0.5330 | 0.20 | 2.9 | 1,965 | 1,965 | 1,965 | 131 | 66 |
| 3 | 0.7470 | 0.44 | 11.3 | 7,446 | 7,446 | 7,446 | 344 | 109 |
| 3.5 | 0.9170 | 1.15 | 40.2 | 25,220 | 25,220 | 25,220 | 766 | 171 |
| 4 | 0.9700 | 2.41 | 96.0 | 57,104 | 57,104 | 57,104 | 1,511 | 252 |
| 4.5 | 0.9820 | 3.20 | 129.7 | 76,630 | 76,629 | 76,629 | 2,655 | 358 |
| 5 | 0.9850 | 3.80 | 154.4 | 90,800 | 90,796 | 90,796 | 4,435 | 505 |
| 6 | 0.9870 | 4.57 | 179.1 | 104,840 | 104,815 | 104,815 | 10,722 | 979 |
| 7 | 0.9870 | 5.15 | 187.9 | 109,767 | 109,697 | 109,697 | 21,438 | 1,824 |
| 8 | 0.9870 | 5.84 | 192.0 | 112,057 | 111,917 | 111,917 | 35,949 | 3,237 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3570 | 0.27 | 1.0 | 672 |
| 2 | 0.3910 | 0.30 | 1.2 | 803 |
| 2.5 | 0.5380 | 0.60 | 2.9 | 1,956 |
| 3 | 0.7560 | 2.01 | 11.2 | 7,396 |
| 3.5 | 0.9200 | 6.63 | 40.2 | 25,220 |
| 4 | 0.9720 | 14.91 | 96.0 | 57,104 |
| 4.5 | 0.9820 | 19.97 | 129.7 | 76,630 |
| 5 | 0.9850 | 23.65 | 154.4 | 90,800 |
| 6 | 0.9870 | 27.30 | 179.1 | 104,840 |
| 7 | 0.9870 | 28.58 | 187.9 | 109,767 |
| 8 | 0.9870 | 29.17 | 192.0 | 112,057 |

### glove, visible set region-1, k = 10 (100 queries; K 592, n 128; the centre scan is 0.08 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 2.20 (36; 0; 7,962) | 2.66 (72; 0; 9,449) | 3.30 (179; 0; 11,097) | 3.50 (237; 0; 11,433) |
| direct, the library's stop | 2.26 (80; 0; 7,826) | 2.73 (134; 0; 9,220) | 3.27 (206; 0; 10,753) | 3.51 (246; 0; 11,381) |
| certified levels, the library's stop | 1.60 (80; 7,213; 3,431) | 1.93 (134; 8,242; 3,946) | 2.30 (206; 9,311; 4,516) | 2.48 (246; 9,697; 4,772) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 0.85 (89; 7,500; 787) | 1.14 (146; 8,431; 1,189) | 1.46 (210; 9,356; 1,655) | 1.64 (246; 9,697; 1,961) |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 0.83 (82; 7,280; 774) | 1.11 (134; 8,242; 1,182) | 1.44 (206; 9,311; 1,647) | 1.62 (246; 9,697; 1,888) |
| statistical levels (16, 32 B), tau = rho measured, one dial | 0.85 (89; 7,500; 786) | 1.14 (146; 8,431; 1,187) | 1.46 (210; 9,356; 1,652) | 1.64 (246; 9,697; 1,956) |
| statistical levels, tau = rho measured, best row psi per recall | 0.82 (82; 7,280; 771) | 1.10 (134; 8,242; 1,179) | 1.44 (206; 9,311; 1,643) | 1.61 (246; 9,697; 1,885) |
| statistical (rho measured), one dial, levels 16 B | 1.27 (177; 9,164; 1,561) | 1.27 (177; 9,164; 1,561) | 1.58 (224; 9,719; 2,280) | 1.72 (246; 9,971; 2,606) |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 0.93 (184; 4,284; 905) | 1.24 (216; 6,142; 1,392) | 1.49 (242; 7,629; 1,782) | 1.52 (246; 7,815; 1,831) |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 2.36 / 0.00 / 0.00 / 0.29 | 0.00 / 2.75 / 0.00 / 0.00 / 0.44 |
| certified | 0.28 / 1.01 / 0.10 / 0.17 / 0.29 | 0.31 / 1.16 / 0.11 / 0.21 / 0.44 |
| statistical, one dial | 0.16 / 0.30 / 0.10 / 0.18 / 0.31 | 0.18 / 0.42 / 0.11 / 0.22 / 0.45 |

rho measured per level [0.0994, 0.0992]; 1 / sqrt(d) = 0.1000

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.7230 | 0.43 | 22.9 | 5,186 | 5,180 | 5,008 | 142 | 252 |
| 2.5 | 0.8980 | 0.84 | 86.6 | 8,126 | 8,120 | 7,462 | 276 | 770 |
| 3 | 0.9780 | 1.30 | 177.2 | 10,216 | 10,210 | 8,952 | 476 | 1,411 |
| 3.5 | 0.9940 | 1.51 | 220.3 | 11,021 | 11,016 | 9,490 | 762 | 1,733 |
| 4 | 0.9950 | 1.64 | 245.7 | 11,381 | 11,375 | 9,697 | 1,149 | 1,956 |
| 4.5 | 0.9960 | 1.69 | 251.8 | 11,466 | 11,461 | 9,756 | 1,644 | 2,071 |
| 5 | 0.9960 | 1.74 | 253.7 | 11,491 | 11,486 | 9,766 | 2,243 | 2,195 |
| 6 | 0.9960 | 1.84 | 256.0 | 11,504 | 11,499 | 9,768 | 3,663 | 2,498 |
| 7 | 0.9960 | 1.97 | 256.0 | 11,504 | 11,499 | 9,768 | 5,183 | 2,898 |
| 8 | 0.9960 | 2.13 | 256.0 | 11,504 | 11,499 | 9,768 | 6,568 | 3,417 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.4920 | 0.74 | 3.8 | 2,497 |
| 2 | 0.7610 | 1.45 | 22.4 | 5,111 |
| 2.5 | 0.9140 | 2.34 | 86.0 | 8,100 |
| 3 | 0.9820 | 3.07 | 177.2 | 10,216 |
| 3.5 | 0.9940 | 3.36 | 220.3 | 11,021 |
| 4 | 0.9950 | 3.51 | 245.7 | 11,381 |
| 4.5 | 0.9960 | 3.54 | 251.8 | 11,466 |
| 5 | 0.9960 | 3.55 | 253.7 | 11,491 |
| 6 | 0.9960 | 3.56 | 256.0 | 11,504 |
| 7 | 0.9960 | 3.56 | 256.0 | 11,504 |
| 8 | 0.9960 | 3.56 | 256.0 | 11,504 |

### glove, visible set region-25, k = 10 (100 queries; K 592, n 128; the centre scan is 0.08 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 14.91 (46; 0; 57,531) | 25.92 (89; 0; 100,178) | - | - |
| direct, the library's stop | 13.13 (47; 0; 50,555) | 20.87 (82; 0; 80,517) | - | - |
| certified levels, the library's stop | 4.68 (47; 50,522; 9,024) | 6.54 (82; 80,474; 11,136) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 1.78 (48; 51,521; 284) | 2.82 (85; 83,114; 373) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.78 (48; 51,294; 295) | 2.76 (82; 80,474; 426) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.78 (48; 51,521; 279) | 2.82 (85; 83,114; 365) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 1.77 (48; 51,294; 290) | 2.75 (82; 80,474; 416) | - | - |
| statistical (rho measured), one dial, levels 16 B | 2.82 (84; 80,055; 1,821) | 3.48 (106; 99,696; 2,210) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 1.91 (107; 40,438; 476) | 3.09 (136; 76,813; 645) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 20.61 / 0.00 / 0.00 / 0.18 | - |
| certified | 2.57 / 2.85 / 0.36 / 0.50 / 0.18 | - |
| statistical, one dial | 1.57 / 0.09 / 0.37 / 0.52 / 0.19 | - |

rho measured per level [0.0993, 0.099]; 1 / sqrt(d) = 0.1000

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.3760 | 0.22 | 2.5 | 2,970 | 2,964 | 2,955 | 107 | 81 |
| 2.5 | 0.5390 | 0.36 | 5.9 | 7,068 | 7,063 | 7,048 | 269 | 126 |
| 3 | 0.7480 | 0.78 | 16.7 | 20,362 | 20,356 | 20,336 | 596 | 195 |
| 3.5 | 0.9290 | 1.97 | 54.1 | 57,506 | 57,500 | 57,471 | 1,223 | 295 |
| 4 | 0.9700 | 3.63 | 114.8 | 107,589 | 107,584 | 107,536 | 2,299 | 432 |
| 4.5 | 0.9770 | 4.59 | 148.6 | 135,348 | 135,343 | 135,285 | 3,975 | 603 |
| 5 | 0.9790 | 5.13 | 165.4 | 149,457 | 149,451 | 149,389 | 6,559 | 825 |
| 6 | 0.9800 | 5.89 | 181.1 | 162,858 | 162,848 | 162,777 | 15,784 | 1,531 |
| 7 | 0.9800 | 6.66 | 188.3 | 168,922 | 168,874 | 168,800 | 31,891 | 2,761 |
| 8 | 0.9800 | 7.61 | 190.5 | 170,649 | 170,558 | 170,483 | 54,352 | 4,824 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3120 | 0.50 | 1.1 | 1,600 |
| 2 | 0.3930 | 0.85 | 2.5 | 2,928 |
| 2.5 | 0.5480 | 1.89 | 5.8 | 6,987 |
| 3 | 0.7560 | 5.30 | 16.6 | 20,225 |
| 3.5 | 0.9330 | 14.92 | 54.1 | 57,506 |
| 4 | 0.9700 | 27.87 | 114.8 | 107,589 |
| 4.5 | 0.9770 | 35.05 | 148.6 | 135,348 |
| 5 | 0.9790 | 38.69 | 165.4 | 149,457 |
| 6 | 0.9800 | 42.15 | 181.1 | 162,858 |
| 7 | 0.9800 | 43.72 | 188.3 | 168,922 |
| 8 | 0.9800 | 44.17 | 190.5 | 170,649 |

### laion, visible set none, k = 10 (100 queries; K 500, n 512; the centre scan is 0.26 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 29.91 (13; 0; 28,795) | 56.49 (25; 0; 54,656) | 193 (88; 0; 186,989) | - |
| direct, the library's stop | 46.67 (21; 0; 45,093) | 78.90 (36; 0; 76,449) | 196 (90; 0; 190,221) | - |
| certified levels, the library's stop | 49.13 (21; 45,093; 42,353) | 81.16 (36; 76,449; 70,227) | 184 (90; 190,221; 159,142) | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 8.42 (50; 104,754; 1,050) | 12.68 (76; 160,358; 2,155) | 19.55 (96; 202,762; 6,504) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 6.84 (25; 52,669; 2,618) | 9.75 (46; 96,027; 2,807) | 19.55 (96; 202,762; 6,504) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 7.39 (42; 87,982; 1,030) | 10.83 (63; 132,794; 1,802) | 20.98 (96; 202,762; 7,775) | - |
| statistical levels, tau = rho measured, best row psi per recall | 6.33 (31; 64,326; 1,413) | 9.53 (40; 83,195; 3,300) | 20.98 (96; 202,762; 7,775) | - |
| statistical (rho measured), one dial, levels 16 B | 9.10 (44; 92,641; 2,655) | 12.88 (61; 129,178; 4,246) | 33.35 (95; 200,731; 20,600) | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 7.79 (45; 95,043; 807) | 10.56 (63; 132,794; 1,202) | 18.38 (96; 202,762; 4,581) | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 6.56 (57; 52,782; 1,474) | 11.18 (79; 100,587; 2,834) | 29.55 (96; 197,851; 15,872) | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 78.28 / 0.00 / 0.00 / 0.36 | 0.00 / 195 / 0.00 / 0.00 / 0.80 |
| certified | 4.90 / 71.91 / 3.34 / 0.39 / 0.36 | 12.05 / 163 / 6.88 / 0.97 / 0.80 |
| statistical, one dial | 2.53 / 1.85 / 5.23 / 0.39 / 0.58 | 4.13 / 7.96 / 7.20 / 0.59 / 0.85 |

rho measured per level [0.0478, 0.0455, 0.0508]; 1 / sqrt(d) = 0.0442

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.3770 | 0.74 | 1.2 | 2,756 | 2,756 | 2,756 | 75 | 69 |
| 2.5 | 0.4610 | 0.96 | 2.2 | 4,883 | 4,883 | 4,883 | 176 | 121 |
| 3 | 0.5870 | 1.45 | 4.4 | 9,609 | 9,609 | 9,609 | 354 | 217 |
| 3.5 | 0.7120 | 2.40 | 9.7 | 20,604 | 20,604 | 20,604 | 652 | 360 |
| 4 | 0.8150 | 4.18 | 21.4 | 44,647 | 44,647 | 44,647 | 1,161 | 585 |
| 4.5 | 0.8900 | 6.83 | 38.4 | 80,411 | 80,411 | 80,411 | 2,028 | 932 |
| 5 | 0.9430 | 9.80 | 57.2 | 120,537 | 120,537 | 120,537 | 3,464 | 1,453 |
| 6 | 0.9830 | 15.72 | 90.0 | 190,575 | 190,575 | 190,575 | 9,536 | 3,445 |
| 7 | 0.9900 | 20.98 | 95.7 | 202,762 | 202,762 | 202,762 | 22,785 | 7,775 |
| 8 | 0.9910 | 29.79 | 96.0 | 203,515 | 203,515 | 203,515 | 44,906 | 15,975 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.4430 | 2.74 | 1.0 | 2,349 |
| 2 | 0.4720 | 3.04 | 1.1 | 2,646 |
| 2.5 | 0.5350 | 4.32 | 1.7 | 3,889 |
| 3 | 0.6760 | 8.16 | 3.5 | 7,629 |
| 3.5 | 0.7980 | 18.25 | 8.2 | 17,440 |
| 4 | 0.8920 | 41.52 | 19.1 | 40,077 |
| 4.5 | 0.9530 | 80.84 | 37.4 | 78,331 |
| 5 | 0.9780 | 123 | 56.9 | 119,835 |
| 6 | 0.9900 | 196 | 89.8 | 190,221 |
| 7 | 0.9910 | 209 | 95.7 | 202,762 |
| 8 | 0.9910 | 210 | 96.0 | 203,515 |

### laion, visible set none, k = 100 (100 queries; K 500, n 512; the centre scan is 0.26 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 45.58 (20; 0; 44,043) | 93.28 (42; 0; 90,445) | - | - |
| direct, the library's stop | 57.34 (26; 0; 55,468) | 94.18 (44; 0; 91,307) | - | - |
| certified levels, the library's stop | 62.58 (26; 55,468; 54,344) | 102 (44; 91,307; 88,601) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 9.29 (43; 90,717; 2,706) | 13.90 (67; 141,465; 4,304) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 8.80 (37; 77,771; 2,999) | 13.34 (61; 127,696; 4,509) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 8.70 (38; 79,762; 2,778) | 13.09 (60; 125,809; 4,346) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 8.31 (30; 63,319; 3,365) | 12.61 (49; 102,503; 5,139) | - | - |
| statistical (rho measured), one dial, levels 16 B | 11.73 (41; 85,669; 5,602) | 18.60 (61; 128,872; 9,911) | - | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 8.91 (42; 87,019; 2,294) | 13.16 (63; 132,774; 3,618) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 9.50 (55; 61,299; 3,891) | 15.64 (79; 110,058; 6,773) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 93.50 / 0.00 / 0.00 / 0.42 | - |
| certified | 5.87 / 90.73 / 3.85 / 0.47 / 0.42 | - |
| statistical, one dial | 2.47 / 4.45 / 4.99 / 0.37 / 0.56 | - |

rho measured per level [0.0478, 0.0455, 0.0508]; 1 / sqrt(d) = 0.0442

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.5382 | 1.66 | 3.0 | 6,632 | 6,632 | 6,632 | 681 | 633 |
| 2.5 | 0.6694 | 2.61 | 6.4 | 13,577 | 13,577 | 13,577 | 1,288 | 986 |
| 3 | 0.7812 | 4.23 | 14.3 | 30,037 | 30,037 | 30,037 | 2,228 | 1,505 |
| 3.5 | 0.8750 | 7.15 | 30.0 | 62,830 | 62,830 | 62,830 | 3,825 | 2,294 |
| 4 | 0.9356 | 10.91 | 49.6 | 103,874 | 103,874 | 103,874 | 6,429 | 3,467 |
| 4.5 | 0.9643 | 15.26 | 70.0 | 147,592 | 147,592 | 147,592 | 10,591 | 5,219 |
| 5 | 0.9761 | 19.92 | 86.9 | 183,609 | 183,609 | 183,609 | 16,855 | 7,772 |
| 6 | 0.9829 | 29.85 | 95.4 | 202,169 | 202,169 | 202,169 | 36,694 | 16,232 |
| 7 | 0.9838 | 44.73 | 96.0 | 203,545 | 203,545 | 203,545 | 64,724 | 30,245 |
| 8 | 0.9838 | 65.00 | 96.0 | 203,593 | 203,593 | 203,593 | 95,456 | 49,543 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3951 | 2.94 | 1.1 | 2,550 |
| 2 | 0.5634 | 6.45 | 2.6 | 5,964 |
| 2.5 | 0.7038 | 12.94 | 5.7 | 12,281 |
| 3 | 0.8232 | 29.69 | 13.6 | 28,576 |
| 3.5 | 0.9175 | 63.64 | 29.4 | 61,596 |
| 4 | 0.9621 | 106 | 48.9 | 102,368 |
| 4.5 | 0.9795 | 152 | 69.8 | 147,204 |
| 5 | 0.9835 | 189 | 86.8 | 183,375 |
| 6 | 0.9838 | 208 | 95.4 | 202,169 |
| 7 | 0.9838 | 210 | 96.0 | 203,545 |
| 8 | 0.9838 | 210 | 96.0 | 203,593 |

### laion, visible set random-1, k = 10 (100 queries; K 500, n 512; the centre scan is 0.26 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 1.69 (41; 0; 1,006) | 2.79 (76; 0; 1,807) | 6.34 (194; 0; 4,327) | - |
| direct, the library's stop | 1.80 (46; 0; 1,075) | 2.69 (75; 0; 1,711) | 5.81 (177; 0; 3,944) | - |
| certified levels, the library's stop | 1.80 (46; 0; 1,075) | 2.69 (75; 0; 1,711) | 5.81 (177; 0; 3,944) | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 1.80 (46; 0; 1,075) | 2.69 (75; 0; 1,711) | 5.81 (177; 0; 3,944) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.80 (46; 0; 1,075) | 2.69 (75; 0; 1,711) | 5.81 (177; 0; 3,944) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.80 (46; 0; 1,075) | 2.69 (75; 0; 1,711) | 5.81 (177; 0; 3,944) | - |
| statistical levels, tau = rho measured, best row psi per recall | 1.80 (46; 0; 1,075) | 2.69 (75; 0; 1,711) | 5.81 (177; 0; 3,944) | - |
| statistical (rho measured), one dial, levels 16 B | 2.40 (66; 0; 1,503) | 2.74 (77; 0; 1,748) | 5.63 (171; 0; 3,817) | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 2.40 (66; 0; 1,503) | 2.74 (77; 0; 1,748) | 5.63 (171; 0; 3,817) | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 2.15 (94; 0; 1,033) | 3.18 (135; 0; 1,710) | 6.22 (200; 0; 4,165) | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 1.75 / 0.00 / 0.00 / 0.68 | 0.00 / 4.04 / 0.00 / 0.00 / 1.51 |
| certified | 0.00 / 1.75 / 0.00 / 0.00 / 0.68 | 0.00 / 4.04 / 0.00 / 0.00 / 1.51 |
| statistical, one dial | 0.00 / 1.75 / 0.00 / 0.00 / 0.68 | 0.00 / 4.04 / 0.00 / 0.00 / 1.51 |

rho measured per level [0.0456, 0.044, 0.0485]; 1 / sqrt(d) = 0.0442

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.7550 | 0.73 | 12.7 | 302 | 275 | 0 | 0 | 302 |
| 2.5 | 0.8640 | 1.32 | 30.9 | 724 | 697 | 0 | 0 | 724 |
| 3 | 0.9440 | 2.40 | 65.6 | 1,503 | 1,476 | 0 | 0 | 1,503 |
| 3.5 | 0.9720 | 3.76 | 110.0 | 2,474 | 2,447 | 0 | 0 | 2,474 |
| 4 | 0.9890 | 4.96 | 149.2 | 3,341 | 3,314 | 0 | 0 | 3,341 |
| 4.5 | 0.9900 | 5.81 | 176.7 | 3,944 | 3,918 | 0 | 0 | 3,944 |
| 5 | 0.9910 | 6.30 | 192.7 | 4,293 | 4,266 | 0 | 0 | 4,293 |
| 6 | 0.9910 | 6.51 | 199.7 | 4,443 | 4,416 | 0 | 0 | 4,443 |
| 7 | 0.9910 | 6.52 | 200.0 | 4,450 | 4,423 | 0 | 0 | 4,450 |
| 8 | 0.9910 | 6.52 | 200.0 | 4,450 | 4,423 | 0 | 0 | 4,450 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.4350 | 0.41 | 2.5 | 63 |
| 2 | 0.7550 | 0.73 | 12.7 | 302 |
| 2.5 | 0.8640 | 1.32 | 30.9 | 724 |
| 3 | 0.9440 | 2.40 | 65.6 | 1,503 |
| 3.5 | 0.9720 | 3.76 | 110.0 | 2,474 |
| 4 | 0.9890 | 4.96 | 149.2 | 3,341 |
| 4.5 | 0.9900 | 5.81 | 176.7 | 3,944 |
| 5 | 0.9910 | 6.30 | 192.7 | 4,293 |
| 6 | 0.9910 | 6.51 | 199.7 | 4,443 |
| 7 | 0.9910 | 6.52 | 200.0 | 4,450 |
| 8 | 0.9910 | 6.52 | 200.0 | 4,450 |

### laion, visible set random-25, k = 10 (100 queries; K 500, n 512; the centre scan is 0.26 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 10.04 (16; 0; 9,355) | 22.27 (38; 0; 21,130) | - | - |
| direct, the library's stop | 12.23 (21; 0; 11,457) | 23.79 (42; 0; 22,583) | - | - |
| certified levels, the library's stop | 14.66 (21; 11,165; 10,920) | 27.75 (42; 22,125; 21,153) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 5.92 (46; 24,376; 618) | 8.29 (68; 36,504; 975) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 4.34 (27; 14,079; 877) | 6.91 (52; 27,687; 1,002) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 5.30 (40; 20,897; 595) | 7.34 (58; 31,046; 886) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 4.22 (24; 12,886; 970) | 6.61 (48; 25,245; 1,116) | - | - |
| statistical (rho measured), one dial, levels 16 B | 5.64 (39; 20,916; 1,062) | 8.62 (63; 33,436; 2,019) | - | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 5.53 (41; 21,649; 513) | 8.03 (64; 34,161; 813) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 5.31 (57; 15,300; 1,138) | 6.90 (73; 21,512; 1,455) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 23.12 / 0.00 / 0.00 / 0.41 | - |
| certified | 1.42 / 21.66 / 3.56 / 0.44 / 0.41 | - |
| statistical, one dial | 0.60 / 0.91 / 4.68 / 0.35 / 0.54 | - |

rho measured per level [0.0478, 0.0455, 0.0508]; 1 / sqrt(d) = 0.0442

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.4490 | 0.79 | 1.7 | 982 | 980 | 974 | 61 | 70 |
| 2.5 | 0.5760 | 1.17 | 4.0 | 2,246 | 2,243 | 2,225 | 132 | 123 |
| 3 | 0.7130 | 1.83 | 8.9 | 4,822 | 4,819 | 4,789 | 250 | 198 |
| 3.5 | 0.8350 | 3.12 | 20.1 | 10,720 | 10,717 | 10,640 | 445 | 340 |
| 4 | 0.8920 | 5.00 | 36.7 | 19,555 | 19,552 | 19,405 | 777 | 555 |
| 4.5 | 0.9460 | 7.03 | 55.5 | 29,671 | 29,668 | 29,475 | 1,327 | 825 |
| 5 | 0.9720 | 9.07 | 74.2 | 39,951 | 39,948 | 39,685 | 2,215 | 1,227 |
| 6 | 0.9860 | 11.94 | 93.5 | 50,441 | 50,438 | 50,113 | 5,458 | 2,499 |
| 7 | 0.9850 | 14.55 | 96.0 | 51,781 | 51,778 | 51,450 | 10,953 | 4,775 |
| 8 | 0.9860 | 18.34 | 96.0 | 51,794 | 51,791 | 51,463 | 18,027 | 8,361 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3930 | 0.95 | 1.0 | 604 |
| 2 | 0.5080 | 1.28 | 1.6 | 928 |
| 2.5 | 0.6490 | 2.18 | 3.2 | 1,793 |
| 3 | 0.7810 | 4.76 | 7.9 | 4,267 |
| 3.5 | 0.8930 | 10.74 | 18.7 | 10,026 |
| 4 | 0.9390 | 20.52 | 36.5 | 19,431 |
| 4.5 | 0.9740 | 30.94 | 55.1 | 29,460 |
| 5 | 0.9840 | 41.77 | 74.0 | 39,882 |
| 6 | 0.9850 | 52.74 | 93.5 | 50,441 |
| 7 | 0.9850 | 54.13 | 96.0 | 51,781 |
| 8 | 0.9850 | 54.14 | 96.0 | 51,794 |

### laion, visible set region-1, k = 10 (100 queries; K 500, n 512; the centre scan is 0.26 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 6.73 (28; 0; 6,036) | 8.53 (51; 0; 7,608) | 11.40 (144; 0; 9,662) | 11.76 (172; 0; 9,794) |
| direct, the library's stop | 7.24 (57; 0; 6,294) | 8.94 (90; 0; 7,696) | 10.92 (154; 0; 9,117) | 11.61 (183; 0; 9,563) |
| certified levels, the library's stop | 8.21 (57; 4,941; 5,843) | 9.85 (90; 5,957; 6,955) | 11.67 (154; 6,881; 8,018) | 12.28 (183; 7,143; 8,331) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 4.63 (100; 6,121; 2,073) | 5.44 (136; 6,694; 2,463) | 6.70 (187; 7,234; 3,192) | 7.27 (196; 7,302; 3,659) |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 3.99 (63; 5,191; 1,893) | 5.01 (98; 6,094; 2,447) | 6.37 (159; 6,943; 3,144) | 7.04 (183; 7,155; 3,565) |
| statistical levels (16, 32 B), tau = rho measured, one dial | 4.60 (98; 6,104; 2,048) | 5.41 (136; 6,693; 2,440) | 6.61 (187; 7,227; 3,115) | 7.18 (196; 7,302; 3,565) |
| statistical levels, tau = rho measured, best row psi per recall | 3.99 (64; 5,228; 1,876) | 4.96 (98; 6,097; 2,400) | 6.34 (162; 6,970; 3,089) | 6.94 (183; 7,155; 3,470) |
| statistical (rho measured), one dial, levels 16 B | 5.34 (128; 6,603; 2,506) | 5.53 (135; 6,682; 2,622) | 6.93 (186; 7,215; 3,512) | 7.73 (196; 7,309; 4,196) |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 5.27 (128; 6,593; 2,304) | 5.44 (136; 6,678; 2,393) | 6.44 (183; 7,166; 2,897) | 7.62 (199; 7,345; 3,826) |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 4.65 (166; 3,187; 2,039) | 5.31 (180; 3,807; 2,422) | 6.73 (195; 5,293; 3,368) | 7.48 (198; 6,059; 3,947) |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 7.88 / 0.00 / 0.00 / 0.80 | 0.00 / 9.34 / 0.00 / 0.00 / 1.33 |
| certified | 0.38 / 7.12 / 1.14 / 0.15 / 0.80 | 0.43 / 8.21 / 1.25 / 0.19 / 1.33 |
| statistical, one dial | 0.13 / 2.50 / 1.23 / 0.11 / 1.18 | 0.15 / 3.19 / 1.29 / 0.13 / 1.59 |

rho measured per level [0.044, 0.0425, 0.0495]; 1 / sqrt(d) = 0.0442

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.7430 | 2.81 | 34.1 | 5,088 | 5,015 | 4,044 | 171 | 1,173 |
| 2.5 | 0.8700 | 4.14 | 77.8 | 7,381 | 7,307 | 5,749 | 285 | 1,832 |
| 3 | 0.9420 | 5.24 | 127.5 | 8,652 | 8,578 | 6,600 | 445 | 2,351 |
| 3.5 | 0.9760 | 5.99 | 164.7 | 9,293 | 9,220 | 6,997 | 661 | 2,728 |
| 4 | 0.9890 | 6.48 | 183.7 | 9,613 | 9,540 | 7,190 | 945 | 3,023 |
| 4.5 | 0.9920 | 6.87 | 192.2 | 9,789 | 9,715 | 7,301 | 1,288 | 3,300 |
| 5 | 0.9950 | 7.18 | 195.7 | 9,804 | 9,731 | 7,302 | 1,686 | 3,565 |
| 6 | 0.9960 | 7.86 | 198.7 | 9,879 | 9,805 | 7,358 | 2,577 | 4,188 |
| 7 | 0.9960 | 8.60 | 199.8 | 9,884 | 9,811 | 7,358 | 3,443 | 4,884 |
| 8 | 0.9960 | 9.32 | 200.0 | 9,885 | 9,812 | 7,358 | 4,155 | 5,579 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.5260 | 1.88 | 4.0 | 1,489 |
| 2 | 0.8460 | 5.56 | 31.9 | 4,861 |
| 2.5 | 0.9380 | 8.41 | 75.0 | 7,303 |
| 3 | 0.9790 | 10.21 | 127.0 | 8,645 |
| 3.5 | 0.9940 | 11.18 | 164.0 | 9,289 |
| 4 | 0.9950 | 11.61 | 183.2 | 9,563 |
| 4.5 | 0.9950 | 11.92 | 192.2 | 9,789 |
| 5 | 0.9950 | 11.96 | 195.7 | 9,804 |
| 6 | 0.9950 | 12.07 | 198.7 | 9,879 |
| 7 | 0.9950 | 12.08 | 199.8 | 9,884 |
| 8 | 0.9950 | 12.08 | 200.0 | 9,885 |

### laion, visible set region-25, k = 10 (100 queries; K 500, n 512; the centre scan is 0.26 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 26.61 (27; 0; 25,456) | 49.20 (53; 0; 47,312) | - | - |
| direct, the library's stop | 28.99 (32; 0; 27,744) | 49.58 (55; 0; 47,665) | - | - |
| certified levels, the library's stop | 32.61 (32; 27,265; 26,581) | 54.39 (55; 46,852; 44,681) | - | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 8.66 (63; 52,983; 1,586) | 11.41 (83; 69,587; 2,431) | - | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 6.74 (40; 33,905; 1,857) | 10.42 (57; 48,163; 3,787) | - | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 7.72 (55; 46,162; 1,435) | 10.74 (77; 64,833; 2,284) | - | - |
| statistical levels, tau = rho measured, best row psi per recall | 6.47 (36; 30,760; 1,939) | 9.71 (65; 54,732; 2,391) | - | - |
| statistical (rho measured), one dial, levels 16 B | 8.20 (54; 45,300; 2,168) | 12.24 (76; 64,253; 4,079) | - | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 7.86 (55; 46,444; 1,317) | 10.89 (78; 65,655; 2,027) | - | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 7.71 (75; 30,510; 2,355) | 11.12 (87; 47,946; 3,649) | - | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 48.81 / 0.00 / 0.00 / 0.52 | - |
| certified | 3.01 / 45.75 / 4.32 / 0.53 / 0.52 | - |
| statistical, one dial | 1.25 / 2.34 / 5.78 / 0.43 / 0.69 | - |

rho measured per level [0.0471, 0.045, 0.05]; 1 / sqrt(d) = 0.0442

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.4340 | 1.03 | 3.5 | 3,221 | 3,198 | 3,132 | 82 | 158 |
| 2.5 | 0.5500 | 1.61 | 7.3 | 6,687 | 6,664 | 6,553 | 183 | 256 |
| 3 | 0.7030 | 2.71 | 14.6 | 12,974 | 12,952 | 12,728 | 371 | 462 |
| 3.5 | 0.8100 | 4.60 | 28.8 | 25,256 | 25,233 | 24,817 | 685 | 806 |
| 4 | 0.8920 | 7.30 | 51.4 | 44,136 | 44,113 | 43,405 | 1,224 | 1,333 |
| 4.5 | 0.9380 | 9.73 | 70.0 | 60,207 | 60,184 | 59,258 | 2,120 | 1,916 |
| 5 | 0.9610 | 11.66 | 83.0 | 71,050 | 71,028 | 69,944 | 3,525 | 2,621 |
| 6 | 0.9780 | 14.71 | 92.7 | 78,995 | 78,972 | 77,784 | 8,425 | 4,654 |
| 7 | 0.9790 | 18.79 | 96.0 | 81,729 | 81,706 | 80,474 | 16,445 | 8,209 |
| 8 | 0.9790 | 24.45 | 96.0 | 81,750 | 81,727 | 80,494 | 26,881 | 13,572 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.3830 | 1.86 | 1.3 | 1,488 |
| 2 | 0.5030 | 3.30 | 3.2 | 2,885 |
| 2.5 | 0.6240 | 6.36 | 6.4 | 5,844 |
| 3 | 0.8010 | 12.90 | 13.8 | 12,169 |
| 3.5 | 0.8900 | 25.24 | 27.2 | 24,117 |
| 4 | 0.9430 | 45.11 | 50.3 | 43,340 |
| 4.5 | 0.9690 | 61.72 | 69.0 | 59,404 |
| 5 | 0.9760 | 73.67 | 82.9 | 70,968 |
| 6 | 0.9790 | 81.97 | 92.7 | 78,995 |
| 7 | 0.9790 | 84.80 | 96.0 | 81,729 |
| 8 | 0.9790 | 84.82 | 96.0 | 81,750 |

### yahoo_fit, visible set none, k = 10 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 9.74 (6; 0; 12,398) | 18.55 (12; 0; 23,834) | 67.55 (43; 0; 87,382) | 84.91 (54; 0; 109,900) |
| direct, the library's stop | 10.39 (6; 0; 13,240) | 23.32 (15; 0; 30,009) | 51.19 (33; 0; 66,165) | 65.73 (42; 0; 85,015) |
| certified levels, the library's stop | 8.91 (6; 13,240; 9,960) | 17.12 (15; 30,009; 19,127) | 31.33 (33; 66,165; 34,489) | 36.69 (42; 85,015; 39,931) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 2.04 (14; 28,729; 170) | 3.04 (22; 45,804; 241) | 6.47 (51; 103,431; 724) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.38 (8; 16,040; 230) | 2.50 (16; 33,559; 414) | 6.47 (51; 103,431; 724) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.40 (8; 16,919; 197) | 2.54 (17; 35,540; 328) | 4.92 (37; 74,147; 702) | 6.56 (48; 97,774; 1,181) |
| statistical levels, tau = rho measured, best row psi per recall | 1.36 (7; 13,693; 366) | 2.44 (16; 32,751; 384) | 4.90 (36; 73,467; 720) | 6.47 (46; 94,129; 1,294) |
| statistical (rho measured), one dial, levels 16 B | 1.58 (8; 16,919; 474) | 2.89 (17; 35,146; 897) | 6.02 (37; 74,044; 2,336) | 8.83 (48; 97,774; 4,413) |
| statistical (rho measured), one dial, levels 4, 32 B | 1.81 (12; 24,687; 436) | 2.63 (19; 39,601; 571) | 5.02 (39; 79,842; 1,081) | 6.46 (49; 99,342; 1,516) |
| statistical (rho measured), one dial, levels 8, 32 B | 1.53 (10; 20,514; 286) | 2.48 (18; 37,644; 420) | 5.10 (41; 82,767; 972) | 6.25 (49; 99,830; 1,338) |
| statistical (rho measured), one dial, levels 4, 16, 32 B | 1.82 (12; 25,622; 439) | 2.58 (20; 40,046; 570) | 4.98 (41; 82,767; 1,143) | 6.11 (49; 99,830; 1,525) |
| statistical (rho measured), one dial, levels 8, 16, 32 B | 1.57 (10; 21,440; 289) | 2.48 (19; 38,148; 419) | 5.01 (41; 82,767; 969) | 6.11 (49; 99,830; 1,336) |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 2.02 (17; 22,527; 318) | 3.06 (26; 37,619; 479) | 6.09 (48; 82,625; 1,198) | - |
| statistical (rho measured), one dial, levels 4, 8, 12, 16, 24, 32 B | 1.85 (13; 26,710; 439) | 2.61 (20; 41,307; 572) | 5.13 (43; 86,862; 1,226) | 5.99 (50; 100,512; 1,532) |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 23.05 / 0.00 / 0.00 / 0.14 | 0.00 / 50.81 / 0.00 / 0.00 / 0.25 |
| certified | 1.02 / 14.69 / 1.06 / 0.09 / 0.14 | 2.14 / 26.49 / 2.11 / 0.20 / 0.25 |
| statistical, one dial | 0.67 / 0.25 / 1.23 / 0.11 / 0.16 | 1.41 / 0.54 / 2.34 / 0.23 / 0.27 |

rho measured per level [0.0684, 0.0647, 0.0625, 0.0611, 0.0592, 0.0581]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.4650 | 0.40 | 1.0 | 2,192 | 2,192 | 2,192 | 3 | 29 |
| 2.5 | 0.4920 | 0.41 | 1.0 | 2,233 | 2,233 | 2,233 | 13 | 34 |
| 3 | 0.5410 | 0.43 | 1.1 | 2,396 | 2,396 | 2,396 | 33 | 43 |
| 3.5 | 0.6280 | 0.50 | 1.5 | 3,238 | 3,238 | 3,238 | 80 | 63 |
| 4 | 0.7510 | 0.67 | 2.6 | 5,615 | 5,615 | 5,615 | 157 | 94 |
| 4.5 | 0.8440 | 0.94 | 4.5 | 9,441 | 9,441 | 9,441 | 275 | 139 |
| 5 | 0.9020 | 1.42 | 8.3 | 17,186 | 17,186 | 17,186 | 452 | 199 |
| 6 | 0.9730 | 3.08 | 21.8 | 44,335 | 44,335 | 44,335 | 1,082 | 390 |
| 7 | 0.9910 | 5.03 | 37.6 | 75,901 | 75,901 | 75,901 | 2,384 | 721 |
| 8 | 0.9960 | 6.95 | 50.9 | 103,244 | 103,242 | 103,242 | 4,886 | 1,296 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.5600 | 1.87 | 1.0 | 2,192 |
| 2 | 0.5600 | 1.87 | 1.0 | 2,192 |
| 2.5 | 0.5620 | 1.88 | 1.0 | 2,208 |
| 3 | 0.6040 | 2.03 | 1.1 | 2,396 |
| 3.5 | 0.6750 | 2.62 | 1.5 | 3,166 |
| 4 | 0.7900 | 4.48 | 2.6 | 5,575 |
| 4.5 | 0.8780 | 7.38 | 4.5 | 9,342 |
| 5 | 0.9220 | 13.39 | 8.3 | 17,138 |
| 6 | 0.9810 | 34.30 | 21.7 | 44,258 |
| 7 | 0.9940 | 58.70 | 37.6 | 75,901 |
| 8 | 0.9970 | 79.78 | 50.9 | 103,244 |

### yahoo, visible set none, k = 10 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 9.74 (6; 0; 12,398) | 18.55 (12; 0; 23,834) | 67.55 (43; 0; 87,382) | 84.91 (54; 0; 109,900) |
| direct, the library's stop | 10.39 (6; 0; 13,240) | 23.32 (15; 0; 30,009) | 51.19 (33; 0; 66,165) | 65.73 (42; 0; 85,015) |
| certified levels, the library's stop | 5.63 (6; 13,240; 5,301) | 9.74 (15; 30,009; 8,727) | 16.28 (33; 66,165; 13,379) | 18.68 (42; 85,015; 14,661) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 2.04 (14; 28,729; 170) | 3.04 (22; 45,804; 241) | 6.47 (51; 103,431; 724) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.38 (8; 16,040; 230) | 2.50 (16; 33,559; 414) | 6.47 (51; 103,431; 724) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.40 (8; 16,919; 197) | 2.54 (17; 35,540; 328) | 4.92 (37; 74,147; 702) | 6.56 (48; 97,774; 1,181) |
| statistical levels, tau = rho measured, best row psi per recall | 1.36 (7; 13,693; 366) | 2.44 (16; 32,751; 384) | 4.90 (36; 73,467; 720) | 6.47 (46; 94,129; 1,294) |
| statistical (rho measured), one dial, levels 16 B | 1.58 (8; 16,919; 474) | 2.89 (17; 35,146; 897) | 6.02 (37; 74,044; 2,336) | 8.83 (48; 97,774; 4,413) |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 1.40 (8; 17,033; 138) | 2.54 (17; 35,540; 209) | 4.87 (37; 74,147; 391) | 6.37 (48; 97,774; 599) |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 2.02 (17; 22,527; 318) | 3.06 (26; 37,619; 479) | 6.09 (48; 82,625; 1,198) | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 23.05 / 0.00 / 0.00 / 0.14 | 0.00 / 50.81 / 0.00 / 0.00 / 0.25 |
| certified | 1.56 / 6.70 / 1.06 / 0.16 / 0.14 | 3.16 / 10.28 / 2.11 / 0.35 / 0.25 |
| statistical, one dial | 0.67 / 0.25 / 1.23 / 0.11 / 0.16 | 1.41 / 0.54 / 2.34 / 0.23 / 0.27 |

rho measured per level [0.0611, 0.0581, 0.0617]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.4650 | 0.40 | 1.0 | 2,192 | 2,192 | 2,192 | 3 | 29 |
| 2.5 | 0.4920 | 0.41 | 1.0 | 2,233 | 2,233 | 2,233 | 13 | 34 |
| 3 | 0.5410 | 0.43 | 1.1 | 2,396 | 2,396 | 2,396 | 33 | 43 |
| 3.5 | 0.6280 | 0.50 | 1.5 | 3,238 | 3,238 | 3,238 | 80 | 63 |
| 4 | 0.7510 | 0.67 | 2.6 | 5,615 | 5,615 | 5,615 | 157 | 94 |
| 4.5 | 0.8440 | 0.94 | 4.5 | 9,441 | 9,441 | 9,441 | 275 | 139 |
| 5 | 0.9020 | 1.42 | 8.3 | 17,186 | 17,186 | 17,186 | 452 | 199 |
| 6 | 0.9730 | 3.08 | 21.8 | 44,335 | 44,335 | 44,335 | 1,082 | 390 |
| 7 | 0.9910 | 5.03 | 37.6 | 75,901 | 75,901 | 75,901 | 2,384 | 721 |
| 8 | 0.9960 | 6.95 | 50.9 | 103,244 | 103,242 | 103,242 | 4,886 | 1,295 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.5600 | 1.87 | 1.0 | 2,192 |
| 2 | 0.5600 | 1.87 | 1.0 | 2,192 |
| 2.5 | 0.5620 | 1.88 | 1.0 | 2,208 |
| 3 | 0.6040 | 2.03 | 1.1 | 2,396 |
| 3.5 | 0.6750 | 2.62 | 1.5 | 3,166 |
| 4 | 0.7900 | 4.48 | 2.6 | 5,575 |
| 4.5 | 0.8780 | 7.38 | 4.5 | 9,342 |
| 5 | 0.9220 | 13.39 | 8.3 | 17,138 |
| 6 | 0.9810 | 34.30 | 21.7 | 44,258 |
| 7 | 0.9940 | 58.70 | 37.6 | 75,901 |
| 8 | 0.9970 | 79.78 | 50.9 | 103,244 |

### yahoo, visible set none, k = 100 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 16.31 (10; 0; 20,928) | 29.81 (19; 0; 38,437) | 91.81 (58; 0; 118,841) | - |
| direct, the library's stop | 14.02 (9; 0; 17,953) | 26.48 (17; 0; 34,107) | 78.90 (50; 0; 102,103) | - |
| certified levels, the library's stop | 11.74 (9; 17,953; 12,499) | 19.25 (17; 34,107; 20,196) | 39.08 (50; 102,103; 38,168) | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 2.31 (12; 23,895; 829) | 3.94 (24; 48,238; 1,182) | 10.27 (60; 120,937; 4,344) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 2.26 (10; 21,199; 946) | 3.83 (17; 35,691; 1,898) | 9.98 (54; 110,310; 4,649) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 2.21 (9; 19,474; 1,003) | 3.59 (19; 37,955; 1,432) | 9.70 (53; 108,119; 4,401) | - |
| statistical levels, tau = rho measured, best row psi per recall | 2.21 (10; 19,689; 986) | 3.60 (20; 39,961; 1,315) | 9.74 (52; 105,658; 4,618) | - |
| statistical (rho measured), one dial, levels 16 B | 2.80 (10; 19,775; 1,826) | 5.02 (21; 42,540; 3,124) | 14.81 (53; 107,831; 11,539) | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 2.14 (10; 20,056; 786) | 3.72 (21; 42,680; 1,098) | 8.69 (53; 108,119; 2,610) | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 2.89 (17; 24,853; 1,283) | 4.06 (25; 38,558; 1,684) | 10.60 (57; 105,290; 5,454) | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 26.19 / 0.00 / 0.00 / 0.15 | 0.00 / 78.42 / 0.00 / 0.00 / 0.36 |
| certified | 2.06 / 15.51 / 1.22 / 0.18 / 0.15 | 5.61 / 29.31 / 3.13 / 0.54 / 0.36 |
| statistical, one dial | 0.75 / 1.10 / 1.33 / 0.11 / 0.16 | 2.19 / 3.38 / 3.29 / 0.33 / 0.38 |

rho measured per level [0.0611, 0.0581, 0.0617]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.5430 | 0.63 | 1.2 | 2,689 | 2,689 | 2,689 | 130 | 282 |
| 2.5 | 0.6306 | 0.78 | 1.7 | 3,735 | 3,735 | 3,735 | 307 | 381 |
| 3 | 0.7406 | 1.07 | 3.0 | 6,354 | 6,354 | 6,354 | 599 | 535 |
| 3.5 | 0.8324 | 1.48 | 5.1 | 10,777 | 10,777 | 10,777 | 994 | 730 |
| 4 | 0.8974 | 2.15 | 9.1 | 18,682 | 18,682 | 18,682 | 1,556 | 985 |
| 4.5 | 0.9422 | 3.18 | 15.8 | 32,331 | 32,331 | 32,331 | 2,330 | 1,308 |
| 5 | 0.9676 | 4.50 | 24.9 | 50,644 | 50,644 | 50,644 | 3,378 | 1,713 |
| 6 | 0.9858 | 7.27 | 42.4 | 85,970 | 85,970 | 85,970 | 6,641 | 2,852 |
| 7 | 0.9906 | 10.04 | 54.9 | 111,283 | 111,283 | 111,283 | 12,072 | 4,622 |
| 8 | 0.9911 | 12.82 | 60.6 | 123,182 | 123,182 | 123,182 | 20,162 | 7,288 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.4984 | 1.88 | 1.0 | 2,208 |
| 2 | 0.5385 | 2.13 | 1.2 | 2,531 |
| 2.5 | 0.6357 | 2.99 | 1.7 | 3,649 |
| 3 | 0.7481 | 5.04 | 3.0 | 6,311 |
| 3.5 | 0.8409 | 8.40 | 5.1 | 10,669 |
| 4 | 0.9048 | 14.48 | 9.0 | 18,545 |
| 4.5 | 0.9475 | 25.00 | 15.7 | 32,199 |
| 5 | 0.9716 | 39.19 | 24.9 | 50,597 |
| 6 | 0.9880 | 66.46 | 42.4 | 85,970 |
| 7 | 0.9911 | 85.75 | 54.7 | 110,977 |
| 8 | 0.9915 | 95.16 | 60.6 | 123,182 |

### yahoo, visible set random-1, k = 10 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 0.63 (20; 0; 424) | 1.03 (38; 0; 810) | 2.39 (99; 0; 2,085) | 3.40 (145; 0; 3,031) |
| direct, the library's stop | 0.59 (18; 0; 385) | 0.91 (33; 0; 684) | 1.92 (79; 0; 1,637) | 3.08 (132; 0; 2,724) |
| certified levels, the library's stop | 0.59 (18; 0; 385) | 0.91 (33; 0; 684) | 1.92 (79; 0; 1,637) | 3.08 (132; 0; 2,724) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 0.59 (18; 0; 385) | 0.91 (33; 0; 684) | 1.92 (79; 0; 1,637) | 3.08 (132; 0; 2,724) |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 0.59 (18; 0; 385) | 0.91 (33; 0; 684) | 1.92 (79; 0; 1,637) | 3.08 (132; 0; 2,724) |
| statistical levels (16, 32 B), tau = rho measured, one dial | 0.59 (18; 0; 385) | 0.91 (33; 0; 684) | 1.92 (79; 0; 1,637) | 3.08 (132; 0; 2,724) |
| statistical levels, tau = rho measured, best row psi per recall | 0.59 (18; 0; 385) | 0.91 (33; 0; 684) | 1.92 (79; 0; 1,637) | 3.08 (132; 0; 2,724) |
| statistical (rho measured), one dial, levels 16 B | 0.61 (19; 0; 409) | 0.94 (34; 0; 717) | 1.94 (79; 0; 1,653) | 3.08 (132; 0; 2,724) |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 0.61 (19; 0; 409) | 0.94 (34; 0; 717) | 1.94 (79; 0; 1,653) | 3.08 (132; 0; 2,724) |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 0.69 (29; 0; 433) | 0.99 (46; 0; 686) | 2.22 (110; 0; 1,779) | 2.99 (146; 0; 2,488) |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 0.53 / 0.00 / 0.00 / 0.25 | 0.00 / 1.26 / 0.00 / 0.00 / 0.53 |
| certified | 0.00 / 0.53 / 0.00 / 0.00 / 0.25 | 0.00 / 1.26 / 0.00 / 0.00 / 0.53 |
| statistical, one dial | 0.00 / 0.53 / 0.00 / 0.00 / 0.25 | 0.00 / 1.26 / 0.00 / 0.00 / 0.53 |

rho measured per level [0.0592, 0.0581, 0.0615]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.7220 | 0.29 | 4.9 | 105 | 82 | 0 | 0 | 105 |
| 2.5 | 0.8070 | 0.37 | 8.4 | 181 | 157 | 0 | 0 | 181 |
| 3 | 0.8850 | 0.51 | 14.9 | 316 | 293 | 0 | 0 | 316 |
| 3.5 | 0.9310 | 0.74 | 25.0 | 527 | 503 | 0 | 0 | 527 |
| 4 | 0.9660 | 1.05 | 39.1 | 816 | 792 | 0 | 0 | 816 |
| 4.5 | 0.9820 | 1.48 | 58.8 | 1,226 | 1,202 | 0 | 0 | 1,226 |
| 5 | 0.9910 | 1.97 | 81.1 | 1,688 | 1,665 | 0 | 0 | 1,688 |
| 6 | 0.9950 | 3.08 | 131.5 | 2,724 | 2,700 | 0 | 0 | 2,724 |
| 7 | 0.9980 | 3.88 | 168.0 | 3,476 | 3,452 | 0 | 0 | 3,476 |
| 8 | 0.9980 | 4.35 | 189.1 | 3,920 | 3,896 | 0 | 0 | 3,920 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.5240 | 0.23 | 2.0 | 46 |
| 2 | 0.7220 | 0.29 | 4.9 | 105 |
| 2.5 | 0.8070 | 0.37 | 8.4 | 181 |
| 3 | 0.8850 | 0.51 | 14.9 | 316 |
| 3.5 | 0.9310 | 0.74 | 25.0 | 527 |
| 4 | 0.9660 | 1.05 | 39.1 | 816 |
| 4.5 | 0.9820 | 1.48 | 58.8 | 1,226 |
| 5 | 0.9910 | 1.97 | 81.1 | 1,688 |
| 6 | 0.9950 | 3.08 | 131.5 | 2,724 |
| 7 | 0.9980 | 3.88 | 168.0 | 3,476 |
| 8 | 0.9980 | 4.35 | 189.1 | 3,920 |

### yahoo, visible set random-25, k = 10 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 3.46 (8; 0; 4,207) | 6.22 (15; 0; 7,742) | 21.35 (53; 0; 27,140) | 35.22 (88; 0; 44,928) |
| direct, the library's stop | 3.21 (7; 0; 3,884) | 5.53 (13; 0; 6,866) | 20.32 (51; 0; 25,814) | 30.84 (77; 0; 39,300) |
| certified levels, the library's stop | 2.87 (7; 3,884; 2,254) | 4.41 (13; 6,866; 3,407) | 10.95 (51; 25,814; 7,265) | 14.11 (77; 39,300; 8,362) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 1.26 (10; 5,342; 95) | 2.71 (28; 14,022; 170) | 5.87 (68; 34,560; 444) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.13 (8; 4,085; 199) | 1.76 (14; 6,994; 337) | 5.09 (56; 28,252; 576) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.14 (9; 4,513; 121) | 1.88 (16; 8,424; 178) | 5.16 (57; 29,082; 515) | - |
| statistical levels, tau = rho measured, best row psi per recall | 1.12 (8; 4,136; 173) | 1.73 (14; 6,994; 308) | 5.08 (56; 28,252; 556) | - |
| statistical (rho measured), one dial, levels 16 B | 1.38 (10; 5,380; 274) | 2.01 (17; 8,511; 397) | 5.73 (57; 29,082; 1,517) | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 1.34 (10; 5,406; 100) | 1.95 (17; 8,573; 129) | 5.28 (57; 29,082; 301) | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 1.49 (16; 5,259; 216) | 2.48 (28; 10,352; 331) | 5.08 (62; 24,961; 709) | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 5.27 / 0.00 / 0.00 / 0.13 | 0.00 / 19.83 / 0.00 / 0.00 / 0.36 |
| certified | 0.40 / 2.62 / 0.99 / 0.14 / 0.13 | 1.33 / 5.58 / 3.00 / 0.55 / 0.36 |
| statistical, one dial | 0.16 / 0.14 / 1.20 / 0.10 / 0.15 | 0.56 / 0.40 / 3.32 / 0.35 / 0.40 |

rho measured per level [0.0605, 0.0577, 0.0616]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.5270 | 0.37 | 1.0 | 562 | 562 | 562 | 5 | 30 |
| 2.5 | 0.5620 | 0.39 | 1.1 | 622 | 622 | 622 | 16 | 36 |
| 3 | 0.6540 | 0.47 | 1.8 | 940 | 940 | 940 | 44 | 49 |
| 3.5 | 0.7580 | 0.63 | 3.1 | 1,655 | 1,655 | 1,655 | 86 | 69 |
| 4 | 0.8620 | 0.86 | 5.7 | 3,000 | 3,000 | 3,000 | 152 | 97 |
| 4.5 | 0.9210 | 1.30 | 10.3 | 5,349 | 5,349 | 5,349 | 245 | 134 |
| 5 | 0.9520 | 1.92 | 16.8 | 8,636 | 8,636 | 8,636 | 374 | 181 |
| 6 | 0.9810 | 3.58 | 37.7 | 19,138 | 19,138 | 19,138 | 811 | 322 |
| 7 | 0.9920 | 5.52 | 61.6 | 31,292 | 31,292 | 31,292 | 1,640 | 558 |
| 8 | 0.9940 | 6.94 | 77.4 | 39,300 | 39,300 | 39,300 | 3,078 | 941 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.5430 | 0.61 | 1.0 | 553 |
| 2 | 0.5480 | 0.61 | 1.0 | 559 |
| 2.5 | 0.5850 | 0.66 | 1.1 | 618 |
| 3 | 0.6710 | 0.90 | 1.7 | 919 |
| 3.5 | 0.7770 | 1.47 | 3.1 | 1,655 |
| 4 | 0.8750 | 2.50 | 5.6 | 2,979 |
| 4.5 | 0.9370 | 4.25 | 10.1 | 5,223 |
| 5 | 0.9630 | 6.82 | 16.5 | 8,509 |
| 6 | 0.9850 | 15.08 | 37.6 | 19,096 |
| 7 | 0.9940 | 24.51 | 61.5 | 31,189 |
| 8 | 0.9950 | 30.84 | 77.4 | 39,300 |

### yahoo, visible set region-1, k = 10 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 3.02 (13; 0; 3,593) | 4.03 (21; 0; 4,840) | 5.30 (45; 0; 6,309) | 5.52 (56; 0; 6,501) |
| direct, the library's stop | 3.34 (22; 0; 3,944) | 3.99 (33; 0; 4,700) | 5.12 (69; 0; 5,879) | 5.55 (88; 0; 6,285) |
| certified levels, the library's stop | 3.71 (22; 3,506; 3,632) | 4.37 (33; 4,096; 4,285) | 5.41 (69; 4,928; 5,165) | 5.72 (88; 5,207; 5,363) |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 1.44 (33; 4,094; 758) | 1.83 (52; 4,680; 1,024) | 2.55 (94; 5,297; 1,521) | 2.66 (100; 5,367; 1,607) |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.35 (25; 3,649; 756) | 1.70 (41; 4,371; 977) | 2.53 (88; 5,227; 1,547) | 2.66 (100; 5,367; 1,607) |
| statistical levels (16, 32 B), tau = rho measured, one dial | 1.39 (30; 3,939; 733) | 1.74 (46; 4,505; 981) | 2.48 (86; 5,197; 1,500) | 2.71 (98; 5,339; 1,686) |
| statistical levels, tau = rho measured, best row psi per recall | 1.34 (25; 3,654; 733) | 1.69 (38; 4,267; 1,007) | 2.41 (80; 5,110; 1,477) | 2.62 (98; 5,339; 1,575) |
| statistical (rho measured), one dial, levels 16 B | 1.54 (33; 4,094; 926) | 1.94 (51; 4,507; 1,249) | 2.81 (87; 5,195; 1,997) | 3.09 (98; 5,338; 2,258) |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 1.49 (33; 4,088; 765) | 1.84 (52; 4,511; 999) | 2.49 (87; 5,175; 1,441) | 2.68 (98; 5,317; 1,573) |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 1.65 (74; 2,222; 886) | 2.08 (89; 2,955; 1,221) | 2.56 (104; 3,775; 1,624) | 2.85 (110; 4,223; 1,875) |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 3.61 / 0.00 / 0.00 / 0.25 | 0.00 / 4.51 / 0.00 / 0.00 / 0.48 |
| certified | 0.26 / 3.29 / 0.35 / 0.08 / 0.25 | 0.31 / 3.97 / 0.41 / 0.11 / 0.48 |
| statistical, one dial | 0.09 / 0.75 / 0.38 / 0.06 / 0.33 | 0.11 / 1.15 / 0.43 / 0.08 / 0.58 |

rho measured per level [0.0575, 0.0555, 0.0611]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.7230 | 0.84 | 11.2 | 2,794 | 2,761 | 2,550 | 124 | 342 |
| 2.5 | 0.8310 | 1.13 | 19.8 | 3,806 | 3,773 | 3,405 | 199 | 537 |
| 3 | 0.9200 | 1.47 | 32.8 | 4,693 | 4,660 | 4,094 | 311 | 790 |
| 3.5 | 0.9610 | 1.84 | 50.5 | 5,445 | 5,413 | 4,655 | 462 | 1,051 |
| 4 | 0.9830 | 2.22 | 72.7 | 5,953 | 5,920 | 4,994 | 656 | 1,312 |
| 4.5 | 0.9910 | 2.51 | 88.3 | 6,285 | 6,252 | 5,227 | 895 | 1,527 |
| 5 | 0.9960 | 2.76 | 100.0 | 6,485 | 6,452 | 5,367 | 1,176 | 1,725 |
| 6 | 0.9980 | 3.18 | 112.0 | 6,708 | 6,675 | 5,522 | 1,830 | 2,145 |
| 7 | 1.0000 | 3.57 | 118.3 | 6,739 | 6,706 | 5,531 | 2,533 | 2,594 |
| 8 | 1.0000 | 3.98 | 120.5 | 6,747 | 6,714 | 5,534 | 3,193 | 3,088 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.5840 | 1.23 | 4.1 | 1,341 |
| 2 | 0.8030 | 2.33 | 10.7 | 2,719 |
| 2.5 | 0.8840 | 3.14 | 18.7 | 3,706 |
| 3 | 0.9490 | 3.97 | 32.4 | 4,674 |
| 3.5 | 0.9780 | 4.66 | 50.5 | 5,434 |
| 4 | 0.9920 | 5.20 | 72.6 | 5,953 |
| 4.5 | 0.9950 | 5.55 | 88.3 | 6,285 |
| 5 | 1.0000 | 5.77 | 100.0 | 6,485 |
| 6 | 1.0000 | 6.02 | 112.0 | 6,708 |
| 7 | 1.0000 | 6.08 | 118.3 | 6,739 |
| 8 | 1.0000 | 6.10 | 120.5 | 6,747 |

### yahoo, visible set region-25, k = 10 (100 queries; K 339, n 384; the centre scan is 0.13 MB of every total)

Megabytes read per query to a recall (total = centres + sigmas + codes + stored vectors + group setups + tables), with: clusters read; rows scanned at the first level; stored vectors read.

| rule, stop | recall 0.9 | recall 0.95 | recall 0.99 | recall 0.995 |
|---|---|---|---|---|
| direct, same clusters for every query | 7.96 (13; 0; 10,027) | 14.87 (26; 0; 18,923) | 33.63 (62; 0; 43,064) | - |
| direct, the library's stop | 7.74 (14; 0; 9,735) | 13.28 (25; 0; 16,853) | 29.41 (56; 0; 37,613) | - |
| certified levels, the library's stop | 5.97 (14; 9,417; 5,473) | 8.91 (25; 16,351; 7,894) | 15.17 (56; 36,551; 12,282) | - |
| statistical levels (16, 32 B), tau = h / sqrt(d), one dial | 2.62 (24; 15,831; 690) | 4.38 (44; 28,805; 1,145) | 8.29 (86; 56,690; 2,419) | - |
| statistical levels, tau = h / sqrt(d), best row psi per recall | 1.98 (15; 9,958; 720) | 3.05 (25; 16,594; 1,119) | 7.21 (73; 47,278; 2,240) | - |
| statistical levels (16, 32 B), tau = rho measured, one dial | 2.03 (17; 11,133; 609) | 3.44 (32; 21,156; 991) | 6.92 (70; 45,514; 2,097) | - |
| statistical levels, tau = rho measured, best row psi per recall | 1.98 (16; 10,291; 670) | 2.97 (25; 16,403; 1,048) | 6.36 (56; 36,659; 2,536) | - |
| statistical (rho measured), one dial, levels 16 B | 2.49 (20; 13,374; 977) | 3.63 (31; 20,573; 1,442) | 7.93 (70; 45,518; 3,705) | - |
| statistical (rho measured), one dial, levels 16, 32, 56 B | 2.37 (20; 13,448; 645) | 3.49 (32; 21,092; 926) | 6.96 (70; 45,379; 1,859) | - |
| statistical (rho measured), one dial, levels 16, 32 B, the key's range statistical too | 2.44 (31; 9,754; 987) | 3.83 (49; 16,595; 1,508) | 6.76 (76; 34,118; 2,581) | - |

Parts in MB at recall 0.95 and 0.99 (codes / stored vectors / group setups / tables / sigmas), and the one dial's points:

| rule | 0.95 | 0.99 |
|---|---|---|
| direct | 0.00 / 12.94 / 0.00 / 0.00 / 0.20 | 0.00 / 28.89 / 0.00 / 0.00 / 0.40 |
| certified | 0.94 / 6.06 / 1.36 / 0.22 / 0.20 | 1.91 / 9.43 / 2.80 / 0.50 / 0.40 |
| statistical, one dial | 0.41 / 0.76 / 1.73 / 0.16 / 0.25 | 0.88 / 1.61 / 3.45 / 0.36 / 0.48 |

rho measured per level [0.06, 0.0573, 0.0616]; 1 / sqrt(d) = 0.0510

| psi (rows and stop), rho measured | recall | MB | clusters | rows offered | in the key's range | scanned at 16 B | at 32 B | stored vectors |
|---|---|---|---|---|---|---|---|---|
| 2 | 0.4450 | 0.48 | 1.9 | 1,474 | 1,444 | 1,416 | 73 | 131 |
| 2.5 | 0.5180 | 0.59 | 2.8 | 2,014 | 1,983 | 1,935 | 107 | 172 |
| 3 | 0.6410 | 0.77 | 4.4 | 2,948 | 2,918 | 2,830 | 163 | 240 |
| 3.5 | 0.7500 | 1.08 | 7.3 | 5,116 | 5,085 | 4,942 | 249 | 336 |
| 4 | 0.8410 | 1.48 | 11.2 | 7,791 | 7,761 | 7,549 | 362 | 455 |
| 4.5 | 0.9030 | 2.06 | 17.2 | 11,652 | 11,622 | 11,315 | 519 | 617 |
| 5 | 0.9400 | 2.93 | 26.6 | 17,993 | 17,962 | 17,518 | 737 | 841 |
| 6 | 0.9830 | 5.13 | 51.1 | 34,040 | 34,009 | 33,162 | 1,428 | 1,484 |
| 7 | 0.9910 | 7.17 | 72.6 | 48,470 | 48,440 | 47,278 | 2,696 | 2,184 |
| 8 | 0.9930 | 8.75 | 86.3 | 58,057 | 58,026 | 56,687 | 4,891 | 2,970 |

| psi (stop), direct reads | recall | MB | clusters | rows |
|---|---|---|---|---|
| 1 | 0.4370 | 1.06 | 1.3 | 1,135 |
| 2 | 0.4860 | 1.30 | 1.8 | 1,448 |
| 2.5 | 0.5530 | 1.70 | 2.7 | 1,958 |
| 3 | 0.6750 | 2.44 | 4.3 | 2,914 |
| 3.5 | 0.7910 | 4.13 | 7.2 | 5,084 |
| 4 | 0.8710 | 6.18 | 11.1 | 7,722 |
| 4.5 | 0.9270 | 9.20 | 17.1 | 11,609 |
| 5 | 0.9550 | 14.16 | 26.6 | 17,993 |
| 6 | 0.9890 | 26.63 | 51.1 | 34,038 |
| 7 | 0.9930 | 37.75 | 72.4 | 48,337 |
| 8 | 0.9940 | 45.18 | 86.2 | 57,908 |
