# #225 independent model assessment / 독립 모델 검증

Candidate c2ef99630950ecc598d157360e666bd96342772d. Prospective protocol 708e32722813fe1606cc2efb84e310d7e6fefe5e.

All three sequential WSL ext4 stream builds exited 101. The Pi 4 allowlist in kernel/build.rs:467 rejects CARGO_FEATURE_PI4_STREAM_V1. Original no-feature global, partitioned and clustered builds succeeded and froze against the specified stock executable. The stock SHA matched. The prospective deadlines and capture settings are preserved in the candidate source diff. Two offline vector tests passed, covering 4096 reference prefixes and the expected terminal tuple.

| Position | Result / 결과 |
|---|---|
| global 1 | Blocked before launch / 빌드 실패 |
| global 2 | Blocked before launch / 빌드 실패 |
| partitioned 1 | Blocked before launch / 빌드 실패 |
| partitioned 2 | Blocked before launch / 빌드 실패 |
| clustered 1 | Blocked before launch / 빌드 실패 |
| clustered 2 | Blocked before launch / 빌드 실패 |

No linked stream ELF or freeze exists for these builds. Zero launches, zero qualified positions. Actual IRQ/ERET byte-loop PC, X21, CPU IDs and leases, guest checkpoint CRCs and terminal product are unobservable. Original historical six early-finish results retain their independent status. No replacements, state injections, hardware trials, or GitHub writes.

세 가지 스트림 설정이 모두 Pi 4 기능 허용 목록에서 실패했다. 기존 합산 앱 세 가지는 빌드와 동결에 성공했다. 독립 참조 계산의 테스트 두 건이 통과했으며, 실행 전 동결 조건을 충족하지 못해 예정된 여섯 위치 모두 미실행이다. 실제 인터럽트와 복귀 주소, X21, CPU와 리스, 게스트 CRC 및 최종 결과는 미검증이다.

All original evidence members can be opened under first/. The 29 MB source member is reproducible from the public candidate. To restore the exact original ZIP, run `python3 reconstruct/restore.py PATH_TO_JNOX NEW_ORIGINAL_ZIP` after checkout using Ubuntu Git. The PM checked the recovered source and byte-for-byte equality with the original first archive. 모든 측정·컴파일 로그와 기존 이미지는 first/에 들어 있다. 후보의 소스 묶음은 지정 공개 커밋에서 재생성하며, 복구한 전체 ZIP의 SHA-256은 원본과 동일하다.

[First build outputs and exact ZIP reconstruction / 최초 빌드 원본 및 ZIP 복원](pi4-crc-stream-first-builds.zip) · SHA-256 9b59a61e3208b16d32052ed82b25b43c3739152943e244b83178d877632f03af.
