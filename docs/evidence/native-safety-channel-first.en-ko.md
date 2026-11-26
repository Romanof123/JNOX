# CHG-311 original frozen image · 최초 동결본
172cbb8412ddfa6cc776f61b292ec022a7ad6e9f, tree 6b41aadfac77c9840517a037ac8e548ee3f409e6. All three Rust process images built. Each of 16 cases in each of FIFO and RR failed in Rust 2024 kernel compilation at native_safety_channel.rs lines 60, 63, 69, 97, 109 (reserved gen). See all 32 original compiler logs and package invocation in run.sh. Zero safety guest boots. All native timing, virtual-device, fault, recovery, gate and frame claims HOLD. The independent analytical admission for both policies remains unknown.
세 프로세스 ELF를 빌드했다. FIFO 및 RR의 각 16개 시나리오 모두 커널 컴파일에서 실패했다. 보호 채널 부팅은 0회다. 모든 게스트 시간, 출력 상태 및 결함·복구 판정은 보류하며 정책별 분석적 승인도 미정이다.

The unchanged startup-probe default independently booted and exited 37 with retained UART, RAM, ELF and manifest. Host library test result 5/5. Additional 62.5 MHz controls directly invoked the original library at exact 5/25/30 ms and one counter tick beyond. Separate flags are recorded in raw/boundary.log. These host observations support the verdict predicates only.
기본 시작 프로브의 종료값은 37이며 UART, RAM, ELF 및 매니페스트를 보관했다. 호스트 테스트 5개 통과. 원본 라이브러리의 5/25/30 ms 경곗값과 1틱 초과 결과는 호스트 판정 함수에 한정한다.

[Raw captures and offline replay](native-safety-channel-first.tar.gz) (SHA-256 `53599c6c99ea09cba2859b24781c83f866e35701ef061a9eceabea74cf16eff7`).
