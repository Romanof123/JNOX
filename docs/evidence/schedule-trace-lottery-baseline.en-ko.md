# CPU0 baseline · CPU0 기준 측정

Source / 소스 `ec00258bad43e510e8fd9ee609a98845a4107231`. Six consecutive first boots, Rust 1.95.0 release --locked on WSL ext4, one virt Cortex-A72, GICv2, TCG default clock. 6개 구성마다 새 ELF를 빌드하고 최초 부팅 결과를 보존했다.

| Case | Terminal UART | A/B/C grants | A/B/C ticks | IRQ/SVC/ERET | RAM bytes |
|---|---|---|---|---|---:|
| lottery1 | PROP END 0 0 0 0 0 | 10/8/14 | 10/8/14 | 33/0/33 | 96272 |
| lottery2 | PROP END 0 0 0 0 0 | 5/11/16 | 5/11/16 | 33/0/33 | 96273 |
| stride | PROP END 0 0 0 0 0 | 6/6/4 | 6/12/8 | 11/6/17 | 96281 |
| lottery-miss | PROP END 446 3 0 2 1 | 0/1/0 | 0/2/0 | 2/0/2 | 96273 |
| stride-delay | PROP END 0 0 0 0 0 | 1/0/0 | 3/0/0 | 2/0/2 | 96281 |
| proportional-bench | PROPORTIONAL PASS | — | — | 75/6/81 | 10432 |

Lottery-miss expected refusal at seed fixture slot 0, two ticks against one. Stride-delay charged A three ticks and advanced pass by 2,162,160. 복권 거부와 지연 stride의 실제 서비스량을 각각 기록했다. Benchmark RAM has 81 populated rows. Ordinary runs retained symbol RAM and terminal registers. Benchmark observer retained witness RAM and QMP status. Its profile did not collect terminal PC. Guest measurements establish virtual CPU observations only.

## Published multilevel records · 공개된 다단계 측정

All 20 rows below come from feedback-wake-first.zip. The older strict-class-owner-window-combined.zip contains 16 corresponding configurations. baseline.json records each original path, both archives’ ELF hashes, capture inventory, sizes, terminal PC, IRQ/SVC/ERET counts and UART digest. Reused observations retain their original build provenance. 아래 20건은 공개 원본에서 읽었고 재부팅하지 않았다. 처음 16개 구성은 두 자료 모두에 존재하며 ELF별 해시는 JSON에 있다.

- feedback-wake-first.zip `fe2b54357aa55c0a5caadf4fe07345454ce29ad70736fe9a163d633befeca281` (20 cases)
- strict-class-owner-window-combined.zip `9cf7cc517b806350c229eaf0eb5d5dd015caf495f46ad5731c6f80b003100ad0` (16 cases)

| Case | Terminal UART | IRQ/SVC/ERET | RAM bytes | ELF SHA-256 |
|---|---|---|---:|---|
| mlfq-delayed-plain | MULTILEVEL DONE | 11/3/14 | 65928 | 72d82776edeeaf43f2f8ebeb86d99242b7570c053c9f6f2984e8363b47f62c90 |
| mlfq-delayed-trace | MULTILEVEL DONE | 11/3/14 | 356776 | 5c756d3a7f03511c2c480050133bbcb1ec0d6f6882ecded3aaa3e6ca6d656efd |
| mlfq-plain | MULTILEVEL DONE | 13/28/41 | 65904 | fe2183629c69e6f44af62eccd353f199dba920a8c1ef6bf92196bd5461c5654b |
| mlfq-trace | MULTILEVEL DONE | 13/28/41 | 334208 | ec9b1b39b12b43de7d1274fe60445f1c25880af993dce34a6f9ce8c64704322e |
| mlq-admit-plain | MULTILEVEL DONE | 1/4/5 | 66160 | df74659fddd117a5e347768d0ecab72d83b65b24239a257f376d7cdf1f8c3322 |
| mlq-admit-trace | MULTILEVEL DONE | 1/4/5 | 357008 | a71444fb794a466132158394f551a9e37583f36f003b1b8c1407f3cfc37e857e |
| mlq-blocked-plain | MULTILEVEL STALL | 0/2/2 | 65920 | 7b4dc8a34eb4ffa1535e5213c3a6f373c49f69b373bb255c1ca9f7c5e9b45e93 |
| mlq-blocked-trace | MULTILEVEL STALL | 0/2/2 | 356768 | b7121315960818639849e91e19496ed935e9666002c7bc6a32ca0f0c31279762 |
| mlq-exited-plain | MULTILEVEL DONE | 0/2/2 | 65920 | d512600693c8c04014d14dcb7bfd92308532ba86f42874b84736a85f3379b2f1 |
| mlq-exited-trace | MULTILEVEL DONE | 0/2/2 | 356768 | d345eafedf53977a193bfabcb5d11d14e2c15a27fac36f052f377bf0e6f7c5f5 |
| mlq-plain | MULTILEVEL DONE | 11/4/15 | 65904 | ada310197521d1d7aaa385702b2167dfe3c971bb864aff0268ed806c25f9a7f3 |
| mlq-refuse-plain | MLQ REFUSE | 0/0/0 | 66160 | f48d377aec576f839dc670a468a72ec0e543e89e9322a1549ae0a37b87eac746 |
| mlq-refuse-trace | MLQ REFUSE | 0/0/0 | 357008 | d26cf554149dfdedc7c47e3a95b360bdd9b812d2bdb039d6a8c1e365d3a7afa9 |
| mlq-saturated-plain | MULTILEVEL DONE | 21/3/24 | 65928 | df95dab76aea5a3fb38dacff1d46f24f9f5e3b3aaaf9f454ad91487d270a41f3 |
| mlq-saturated-trace | MULTILEVEL DONE | 21/3/24 | 356776 | 9c0757603a905362f57f21bf14abd1ca892c91547e81e40a34d7e8c2928849a4 |
| mlq-trace | MULTILEVEL DONE | 10/4/14 | 334208 | a3ef80e0ae7587a848eb12e8fe5f2e749529c5e117f860da455b982dc0240872 |
| mlfq-wake-plain | MULTILEVEL DONE | 6/6/12 | 8000 | dd4d11b34788f1ee7115bb157bcaac66674a0e6126a76cfd3f224a09d59a1464 |
| mlfq-wake-refuse-plain | MLFQ WAKE REFUSE | 0/0/0 | 8000 | e876999e3dcbbeda01b0cf332cf8142422e2bb731ea9a4bde6dda6b7ab15070f |
| mlfq-wake-refuse-trace | MLFQ WAKE REFUSE | 0/0/0 | 8000 | e876999e3dcbbeda01b0cf332cf8142422e2bb731ea9a4bde6dda6b7ab15070f |
| mlfq-wake-trace | MULTILEVEL DONE | 6/6/12 | 298832 | f3c7c14b3c226d6aeef26b7f3fb2cfecbf86c8a44c1001676e32343da65db137 |

[Baseline ELF images, records and provenance / 기준 이미지와 원본](schedule-trace-lottery-baseline.zip) · SHA-256 5e21b68e1d7f0d6c897873ce459b44423370fbf640666e6a5f096b52cab2b61e.
