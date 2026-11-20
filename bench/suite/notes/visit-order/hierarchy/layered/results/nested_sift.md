12:04:57 sift leaves 4000
12:05:15 queries 100
12:05:30 queries 200
12:05:43 queries 300
12:05:57 queries 400
12:06:08 queries 500

### sift: 1,000,000 rows, 4000 leaves of about 250 rows

| leaves per parent | recall | parents only: rows (parents read) | flat leaves: rows, centres | nested, fewest centres within 5% of flat's rows: P, centres, rows / flat | nested within 25%: P, centres, rows / flat |
|---|---|---|---|---|---|
| 8 (500 parents) | 0.9 | 40,154 (18.0) | 9,130, 4000 | 32, 781, 1.01 | 32, 781, 1.01 |
| 8 (500 parents) | 0.95 | 60,813 (27.3) | 14,204, 4000 | 64, 1055, 1.01 | 32, 781, 1.22 |
| 8 (500 parents) | 0.99 | 120,405 (55.3) | 31,409, 4000 | 128, 1585, 1.00 | 64, 1055, 1.11 |
| 32 (125 parents) | 0.9 | 69,436 (8.1) | 9,130, 4000 | 16, 665, 1.02 | 16, 665, 1.02 |
| 32 (125 parents) | 0.95 | 102,020 (12.1) | 14,204, 4000 | 32, 1196, 1.01 | 16, 665, 1.15 |
| 32 (125 parents) | 0.99 | 183,176 (22.1) | 31,409, 4000 | 64, 2159, 1.00 | 32, 1196, 1.10 |
