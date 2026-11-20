### laion: K 500, n 512; residual variance explained by a code in the group's basis, by the number of groups

| groups | clusters per group | f at 16 B | f at 32 B | bits a row for G = 64's distortion at 16 B (128 bits there) | int8 bases, MB |
|---|---|---|---|---|---|
| 1 | 500 | 0.467 | 0.639 | 226 | 0.3 |
| 2 | 250 | 0.490 | 0.659 | 209 | 0.5 |
| 4 | 125 | 0.513 | 0.679 | 191 | 1.0 |
| 8 | 62 | 0.545 | 0.705 | 168 | 2.1 |
| 16 | 31 | 0.565 | 0.720 | 154 | 4.2 |
| 32 | 16 | 0.585 | 0.736 | 142 | 8.4 |
| 64 | 8 | 0.606 | 0.753 | 128 | 16.8 |
| 128 | 4 | 0.634 | 0.774 | 112 | 33.6 |
| 250 | 2 | 0.662 | 0.795 | 96 | 65.5 |
