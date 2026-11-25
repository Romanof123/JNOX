# Native joinable threads · independent verification / 네이티브 조인 스레드 독립 검증

Public original 304667baa498d12f0c99774891f32a28ca287b0c compiled the two application ELFs. Both first ordinary packaging paths failed in kernel rustc with three E0308 errors in native_threads.rs lines 25 and 93. No guest launched on that revision. Its host suite passed 169 tests. Original logs and application binaries are in original/.

공개 최초본에서 C와 Rust 앱 ELF는 생성되었으나 두 커널 패키징 모두 u64와 usize의 형식 불일치 세 건으로 실패했다. 이 버전에서 게스트를 부팅하지 않았다. 원본 기록은 original/에 보존했다.

On corrected public 3d53da8b72a3f58e44e02d256af815fa3159adb9, deployed the same C then Rust workloads from two newly built kernels on ext4. Cortex-A72 virt/GICv2, one CPU, TCG, 128 MiB, default virtual clock. Each workload ran first plain and then in a fresh cold boot with QEMU external exception/PPI tracing. Full UART and physical process memory (4,280,320 bytes where the process mappings exist) retained under runs/. Commands, manifests, source/ELF hashes, all first logs and independent parser are included. Source archive identifies public revision. Host clock/QEMU timing is observational.

수정본을 ext4에서 새로 빌드해 C 일반·추적, Rust 일반·추적 순으로 각각 콜드 부팅했다. 각 실행의 UART와 실제 프로세스 물리 메모리, 외부 추적 원본 및 독립 검증기를 함께 보존했다.

| Corrected cohort / 수정본 | Independent result / 독립 관찰 |
| --- | --- |
| C plain / traced | Exit 17 both. All 256 inputs and both 256-element outputs matched i+1 and i+2, sums 32896/33152, main 32640. Shared descriptor pointers 0x402000 and 0x402420; handles 3 and 5; stale ESRCH. |
| Rust plain / traced | Exit 29 both. All 256 atomic input/square pairs matched, main sum 32640, sum of squares 5559680, true joined address 0x402004. |
| Frames and memory | Common TTBR 0x1000040238000. Independently walked descriptors from the captured table through to the physical backing. Both guard pages unmapped. Main/worker initial SP 0x1010fb0 / 0x1009000. All observed nonzero frame PCs had aligned SP within their thread range and correct errno locator. Actual errno cells 0x1001000/0x1001004 contained 22/38. Both worker generations reported initial errno 0 and four completed quarters. |
| Handoffs and IRQ | First C join blocked; worker slept with main blocked; idle CPU acknowledged PPI30 in traced run. Timer compare 1562256 ticks, IRQ sample 1569055; app deadline 23068368 ns, measurement 23352912 ns. Generation two explicitly exited before immediate join. Normal C deadline 22804304 ns and wake 23105232 ns. |
| External evidence | Complete C traced 28 SVC entry ELRs and 28 actual EL0 return PCs exactly equal the ordered journal projections. Rust matched 8 and 8. All four principal journals had zero drops. |
| Additional cold controls | z=30 live and zombie EAGAIN; s=31 self join EDEADLK; u=32 NULL wrapper arguments EINVAL and unsupported attributes ENOTSUP; n=33 NULL output join; m=0 main pthread_exit continued worker; e=36 early process exit while worker queued; old=17 preexisting #312 batch ELF, explicit argv with one block; o=37 original singleton raw services 10–12 returned −95 and errno preserved; probe=37 legacy default argv and ENOSYS. |
| Host and packaging | Corrected suite 169 passed. CLI count, duplicate, probe, peer, policy and priority all refused (2). Raw count, probe, peer, policy, priority all refused (101 with matched reason). |

독립 검사에서 C 배열 전체와 합계, 세대가 다른 핸들과 결과 포인터, 스택과 errno 주소 및 값, 최초 차단 조인과 이후 즉시 조인을 확인했다. 메인이 조인하고 작업자가 잠든 동안 유휴 IRQ 30이 발생했고 만기 이후 작업자가 복귀했다. Rust 배열 전체와 공유 결과 포인터도 일치했다. 추가 콜드 부팅 아홉 건에서 수명·오류·기존 바이너리 동작을 확인했다.

Boundaries / 관측 범위. CPU0 cooperative single process only. PPI30 was observed from idle EL1. No separate EL0-origin timer workload, explicit FP/SIMD/TLS register canary, physical board run, malformed entry test or direct first-instruction stack-scrub measurement. Reuse zero errno and ordinary continuations were observed. The old batch ELF was reused byte-for-byte in a new singleton package; previously archived kernel packages were not separately booted. The original startup probe checks argv and unsupported write/errno itself. No timing or speed claim.

단일 CPU 협력형 실행을 검증했다. EL0 실행 중 타이머 인터럽트, FP/SIMD 및 TLS 전체 레지스터 표식, 재사용 직후의 전체 스택 초기화, 실제 보드 성능은 별도로 측정하지 않았다. 기존 앱 ELF를 새 단일 스레드 커널 패키지에 그대로 넣어 실행했다.
