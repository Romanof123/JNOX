# J-NOX #348 수정 커밋 개발 통합 점검 / Corrected developer integration preflight

공개 커밋 `84af5040cdf3d969484897e10c45926515075717`의 SDK로 네 슬롯 앱을 순차 빌드했다. The first corrected run produced all four AArch64 C ELFs and packaged the 2-slot within-lo and 4-slot normal-hi kernel ELFs. Both manifests match their input JSON, ordered ELF hashes, generation arguments, canonical deployment digest and kernel hash. The manifests record an empty tracked diff. Each package is included under `packages/`.

| 패키지 / Package | 슬롯 / Slots | kernel.elf SHA-256 |
| --- | ---: | --- |
| within-lo | 2 | `625ed21fd2f29a0f8e3a4d240e2d61484d1ab6fb37469fc3ac5a8771577db3d8` |
| normal-hi | 4 | `592ef9c8bcfce7c28b7e4d4e79e1da0b61fbd17fbce0a42be1b7a4f68d7b2d58` |

CLI는 잘못된 예산과 다섯 번째 슬롯을 각각 종료 코드 2로 거부했다. Raw Cargo builds rejected three inputs with status 101: a zero LO budget with a valid recomputed hash, a stale canonical digest, and a fifth application. Their diagnostics were `invalid AMC descriptor`, `canonical AMC SHA-256 mismatch`, and `at most four native slots`. The exact commands and outputs accompany the package logs in `raw/`.

WSL Ubuntu ext4에서 Clang·LLD 14.0.0, Rust·Cargo 1.95.0, Python 3.10.12와 GNU nm 2.38을 사용했다. This preflight ended before guest launch. The JSON manifests supply per-slot app identity, tier, priority and arguments. They omit explicit per-slot TTBR/root values. Actual root restoration, timing, slot results and policy behavior require guest observation.

원래 커밋 `da689d6356c9b97e585d1c3c0d0e5ae791594dc3`의 개별 첫 결과는 `prior/`에 있다. At that commit all four C builds failed with undeclared CLOCK_PROCESS_CPUTIME_ID. The preserved prior archive SHA-256 is `1b64134aa9e2841a0e4dde3ea0bbcde5c6f3145ec9f40b0f76102811d074b0de`.
