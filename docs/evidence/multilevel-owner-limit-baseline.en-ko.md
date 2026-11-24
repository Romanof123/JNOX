# Ordinary scheduling baseline / 일반 스케줄러 기준 측정

Pinned source / 기준 커밋 e5ce8a2c49068236a13683bd86846fafa3e8cc82

28 release ELF originals, one Cargo job, six diagnostic guest acquisitions. Kernel build.rs selects 6 multilevel apps and 8 deadline apps. Each app uses an unset trace variable for plain and 1 for trace. Each row below lists section bytes. Stack symbols inside .bss are already included in its size. The .stack column contains only the separate linker reservation. matrix.json retains full ELF, both file-backed PT_LOAD segment and section digests, virtual addresses, segment file/memory sizes, and stack symbols. NOBITS has no file payload. readelf.txt retains the full linked tables.

일반 빌드는 추적 변수를 설정하지 않았고 진단 빌드는 1로 설정했다. .bss에 포함된 작업 스택과 링커가 별도 확보한 .stack을 구별했다. 각 이미지 원본·로드 세그먼트 원본·해시·링크 심볼은 개별 폴더 및 matrix.json에 있다. 각 build.json과 guest.json은 명령과 실행 시각(Unix 초)을 기록한다.

|App|Mode|ELF|.text|.rodata|.bss|.stack|
|---|---|---:|---:|---:|---:|---:|
| mlq | plain | 90488 | 13088 | 1075 | 65544 | 16384 |
| mlq | trace | 93200 | 12704 | 1219 | 333856 | 16384 |
| mlfq | plain | 90600 | 13460 | 1075 | 65544 | 16384 |
| mlfq | trace | 93296 | 14752 | 1219 | 333856 | 16384 |
| mlq-blocked | plain | 90568 | 13084 | 1067 | 65544 | 16384 |
| mlq-blocked | trace | 93848 | 13116 | 1283 | 356384 | 16384 |
| mlq-exited | plain | 90568 | 13080 | 1067 | 65544 | 16384 |
| mlq-exited | trace | 93848 | 13116 | 1283 | 356384 | 16384 |
| mlq-saturated | plain | 90856 | 13760 | 1083 | 65560 | 16384 |
| mlq-saturated | trace | 94104 | 13248 | 1323 | 356384 | 16384 |
| mlfq-delayed | plain | 90744 | 13716 | 1083 | 65560 | 16384 |
| mlfq-delayed | trace | 93992 | 13316 | 1323 | 356384 | 16384 |
| ordinary-edf | plain | 103064 | 18372 | 2373 | 76344 | 16384 |
| ordinary-edf | trace | 103064 | 18376 | 2373 | 76344 | 16384 |
| ordinary-llf | plain | 103064 | 18172 | 2373 | 76344 | 16384 |
| ordinary-llf | trace | 103064 | 18176 | 2373 | 76344 | 16384 |
| ordinary-llf-overload | plain | 103064 | 18172 | 2373 | 76344 | 16384 |
| ordinary-llf-overload | trace | 103064 | 18176 | 2373 | 76344 | 16384 |
| ordinary-edf-overrun | plain | 103064 | 18372 | 2373 | 76344 | 16384 |
| ordinary-edf-overrun | trace | 103064 | 18376 | 2373 | 76344 | 16384 |
| ordinary-edf-supply | plain | 105456 | 20664 | 2741 | 92760 | 16384 |
| ordinary-edf-supply | trace | 105456 | 20668 | 2741 | 92760 | 16384 |
| ordinary-llf-supply | plain | 105456 | 20668 | 2741 | 92760 | 16384 |
| ordinary-llf-supply | trace | 105456 | 20672 | 2741 | 92760 | 16384 |
| ordinary-edf-supply-refuse | plain | 104448 | 19708 | 2381 | 92744 | 16384 |
| ordinary-edf-supply-refuse | trace | 104448 | 19712 | 2381 | 92744 | 16384 |
| ordinary-llf-supply-refuse | plain | 104448 | 19508 | 2381 | 92744 | 16384 |
| ordinary-llf-supply-refuse | trace | 104448 | 19512 | 2381 | 92744 | 16384 |

## Guest references / 게스트 기준 실행

Each capture used the public acquisition recipe retained under source/. Each timing value is from one QEMU TCG guest on this host. 거부 게스트에는 작업 실행 시간이 없으며, 허용 게스트의 관측값은 해당 단일 실행에 한정된다.

- mlfq-trace · host rc 0 · replay rc 0 · 43 decisions replayed
- mlq-trace · host rc 0 · replay rc 0 · 17 decisions replayed
- ordinary-edf-supply-refuse-trace · host rc 124 · replay rc 0 · { "refusal": 8, "full_supply_admitted": true }
- ordinary-edf-supply-trace · host rc 124 · replay rc 0 · { "service_counts": 6246061, "grid_counts": 6250000, "worker_counts": [ 9326251, 1882629 ], "worker_blocks": [ 860628, 155083 ], "worker_products": [ 5135156342284207361, 16430211217450858178 ], "service_sum": 89440 }
- ordinary-llf-supply-refuse-trace · host rc 124 · replay rc 0 · { "refusal": 9, "full_supply_admitted": true }
- ordinary-llf-supply-trace · host rc 124 · replay rc 0 · { "service_counts": 6240998, "grid_counts": 6250000, "worker_counts": [ 9327355, 1883499 ], "worker_blocks": [ 804405, 156018 ], "worker_products": [ 15155830834237792577, 17755507081706845314 ], "service_sum": 89440 }

[Original 28 ELF load images, section boundaries and six guest acquisitions](multilevel-owner-limit-baseline.zip) · SHA-256 e6b226c7e8f8d430bbe0167b437acbb7626d771eb090f28e2f7f71971861f20e.
