# #224 finite sample stream v1 / 유한 샘플 스트림 v1

Base cc864a392828d37cf2fb933566e93e20bfeaa455. Default M remains the sum of 1..65536, stride 16, count 4096, sum 2147516416, digest 18080812676020141577. All original six ended before IRQ (0x1d), 0/6 qualified. Keep those records and the original fallback. Build the three migration cases with optional kernel feature pi4-stream-v1 to select a different application.

M verifies the integrity of a deterministic 4 MiB calibration sample stream. Zero-based byte i is the low byte of i XOR (i >> 8) XOR (i >> 16). Process 4096 contiguous 1024-byte blocks once in order, publishing the cumulative standard CRC-32/ISO-HDLC at each block end. Initial internal state and final XOR are 0xffffffff, reflected polynomial 0xedb88320. Final byte count 4194304, CRC 0x78d22ad1, ordered checkpoint digest 8765611335505774534. The application geometry is fixed before execution. First complete block releases the existing peer milestone. Keep the 4-second peer deadline, 20-second idle timeout, 30-second capture and 1-second quiet interval, same P0/P1, IRQ, scheduling, MG2 and raw external observation. No extra core reservation, waits, extra devices or virtual-time performance threshold.

Freeze ELF, image, linked disassembly, source, workload name and stock QEMU SHA 3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862 before all six launches. Freeze all three cases first. Retain global 1,2 then partitioned 1,2 then clustered 1,2 sequentially, including failures, without replacement. A qualified position requires an external IRQ and ERET at the same linked byte-loop PC, X21 task-local state identity, owned progress before and after with exact prefix CRC, full digest and terminal product, global CPU1 or partitioned/clustered CPU0 resumed lease, and all inherited replay checks. An outside-loop PC, early finish, incomplete evidence or missing progress cannot qualify. Keep the independent per-position outcomes even on failure. Model evidence only.

검사용 결정적 4 MiB 샘플을 1024바이트씩 4096블록 처리한다. 각 블록의 누적 CRC와 전체 순서 다이제스트를 독립 계산한다. 첫 블록 완료가 피어 시작 조건이다. 기존 65536개 합산 앱과 0/6 결과를 보존하고 새 앱을 별도 명시한다. 위의 순서로 최초 여섯 건 전체를 보존한다. 실제 IRQ·복귀 PC, X21, CPU와 리스, 복귀 뒤 진행 및 최종 CRC를 대조한다. 크기 재조정이나 성공할 때까지 재실행하지 않는다. 기존 제한 시간과 스케줄러를 유지한다.

## Build and replay / 빌드 및 재생

On the established Linux toolchain, use the original migration build recipe with JNOX_BOARD=pi4, JNOX_PROFILE=smp and each JNOX_APP=migration-{global,partitioned,clustered}. From kernel use `cargo rustc --locked --release -j1 --features pi4-stream-v1`. Keep the ELF, GNU objcopy flat image and GNU objdump -dC disassembly for each case. The replay identifies pi4_stream_crc32_m_v1 and the unique backward byte-processing loop in pi4_stream_crc32_loop_v1. Inspect that loop and X21 use before launch. Every stream freeze requires the stock model hash and linked stream loop.

Use `python3 tools/pi4_migration_capture.py freeze CASE DIR ELF IMG DISASM QEMU --source COMMIT --workload sample-crc32-v1`. Retain all three freeze digests before launch. For each frozen position run `python3 tools/pi4_migration_capture.py run DIR FREEZE_SHA ORDINAL` once, retain and hash every raw file, then `python3 tools/pi4_migration_replay.py DIR ORDINAL SESSION_SHA`. The original default build and historical manifests select sum-65536. Replay derives all stream prefix CRCs with Python zlib independently of the guest's Rust CRC table. Verify reference vectors offline with `python3 -m unittest discover -s tools -p test_pi4_stream_v1.py`.

기존 Linux 환경에서 세 배치별 ELF와 이미지, 역어셈블리를 먼저 고정한다. 독립 재생은 zlib로 모든 블록 CRC를 다시 계산한다. 기본 빌드와 기존 캡처는 원래 합산 앱으로 재생한다.

[First build and freeze results / 최초 빌드 및 고정 결과](../evidence/pi4-crc-stream-first-builds.en-ko.md).

## #225 selector correction / 빌드 선택 조건 수정

At c2ef996 all three stream builds stopped at the Pi 4 linker allowlist. The three sum controls built and froze. Zero stream positions launched. Accept the sole stream feature for the global, partitioned and clustered Pi 4 SMP applications through both the profile and board checks. Exercise the real Cargo selectors for the valid cases, the corresponding legacy controls, and invalid board, application and mixed-feature requests. Preserve the documented selector-free virt cooperative image for the complete Cargo feature set, where the stream is inactive. The existing stream payload, deadlines, linked loop and replay criteria apply to the new candidate.

c2ef996에서 스트림 빌드 세 건은 보드 링커 선택 단계에서 거부됐다. 기존 합산 앱 세 건은 빌드·고정됐고 스트림 실행은 0건이었다. Pi 4 SMP의 전역·파티션·클러스터 앱에서만 스트림을 선택한다. Cargo의 실제 설정 경로에서 세 앱, 기존 합산 앱, 잘못된 보드·앱·기능 조합을 검사한다. 기존 virt 전체 기능 조합에서는 협력형 앱을 선택한다. 최초 실패 기록과 기존 CRC·PC·시간 제한을 유지한다.

[Corrected first stock-model series and linked context replay / 수정본 최초 모델 6회와 문맥 재생](../evidence/pi4-crc-stream-corrected-first.en-ko.md).
