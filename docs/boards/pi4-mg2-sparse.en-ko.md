# Clustered MG2 sparse simulator pair / 클러스터 MG2 희소 관측

Before either first attempt, freeze SHA-256 of QEMU and GNU build ID, plugin and source, configure/build record, header, both complete argv arrays and the existing 30 s / 1 s quiet capture contract. Use the same plugin-capable 11.1.3 executable, raspi4b four CPUs / MTTCG and archived clustered inputs from pi4-migration-mg2-first.zip. First position has the observer off, second on. Preserve QMP RAM/journals and per-thread exception logs in both. Enable plugin log routing in both. Retain every failure. No replacement guest or replay under the first-attempt label.

| Archived input | SHA-256 |
|---|---|
| #198 plugin-capable QEMU (GNU build ID ef73da8a108e97b7d11566630ac676893c2f3ef8) | 2e465436514c0dc203eb2e9c3440187252cb84ebcdcc3fee6de76c9fa1c95fe6 |
| ELF | 8660010cca701e54f23a6af3c1782dd24bfd8da66d345147b5b4daba95f0002f |
| raw | cc3dbb8ececa7f00c01164b716310596d73bdad1c8eed49753f9f3a5ea60ce79 |
| linker.map | 28eef4b2abd972fd79f9ffb69267881aa659301b5f9288f94a209074296d9666 |
| disassembly | aaa48104cf61ea6d1de88b2441929cd2ff0528fa7d2b13f9c42a7ae5f24e683a |

The linked pi4_migration_log::event CPU0 STLR at 0x81a00 publishes the completed row count to LOG (0xa9178) + 6912. Require the exact original instruction word 0xc89ffd28, eight-byte RAM store, matching VA/PA and count 1..96. Read back count, committed row and prior dispatch at fixed bounded RAM addresses on the same masked vCPU. Require kind 5, task 0, positive equal before/after lease, epoch-zero latest kind-11 dispatch matching task/lease, index 65536 and sum 2147516416. The sparse checkpoint logged PC is 0x801f0. Require successful independent terminal RAM reconciliation before credit. A failed read is incomplete. Returning from the logger alone cannot prove its successful path.

The linked pi4_smp_bsp::signal STR at 0x83954 (0xb9000100) on CPU1 must issue a little-endian four-byte MMIO write at VA/PA 0xff841f00 with observed value 0x18001. The pinned tcg/tcg-op-ldst.c emits the memory callback after the guest store operation. Synchronous arm_gic distributor handling has returned by this point. This does not timestamp the internal pending-bit mutation or receiver exception. No device readback. Confirm exact linked words at translation. Sample the other actor's release flag with acquire immediately before each candidate instruction. Publish a release flag only after that actor's transaction and identity checks. A positive pre-sample on the independently validated later transaction proves one order. Two zero samples leave overlap or absence unresolved. Reject duplicates and inconsistent evidence. Host order of log lines is immaterial. No arithmetic-loop hook, timings or qualification credit.

두 순번을 시작하기 전에 동일 실행 파일의 해시·빌드 ID, 플러그인 및 소스, 빌드 기록, 전체 인자와 원본 네 입력을 고정한다. 첫 번째는 플러그인을 끄고 두 번째는 켠다. 같은 MTTCG 설정과 시간 한도를 쓰며 실패 및 RAM·예외 원본을 보존한다. CPU0의 실제 커밋 쓰기와 읽어 확인한 행에서 태스크·임대·인덱스·합·배정 세대를 검사한다. CPU1의 실제 장치 쓰기에서 주소·폭·데이터를 확인한다. 각 쓰기 직전 상대 완료 플래그를 acquire로 읽고 해당 쓰기가 검증된 뒤에만 release로 공개한다. 양의 표식과 독립 RAM 대조가 모두 있을 때만 순서를 인정한다. 양쪽 모두 0이면 미결이며 내부 pending 시각, 실물 지연 또는 이주 성공을 의미하지 않는다.

## Integration commands / 통합 실행

Use Integration's independently recorded model SHA and build ID, model/plugin build receipt (including compiler version, argv, source and output digests), and the exact matching public header. Freeze under a short native Linux path. QEMU API 7 supplies the two fixed translation-selected sites. The observer takes zero arguments. Use glib-2.0 from the matched build environment.

```sh
cc -std=c11 -O2 -Wall -Wextra -Werror -fPIC -shared -I"$QEMU_SRC/include/plugins" $(pkg-config --cflags glib-2.0) tools/pi4_mg2_sparse.c -o observer.so $(pkg-config --libs glib-2.0)
python3 tools/test_pi4_mg2_sparse.py
python3 tools/pi4_mg2_sparse.py freeze NEW_DIR "$QEMU" observer.so "$RECEIPT" "$MODEL_ATTESTATION" "$QEMU_SRC/include/plugins/qemu-plugin.h" "$MODEL_SHA256" "$MODEL_BUILD_ID"
python3 tools/pi4_mg2_sparse.py run NEW_DIR "$FREEZE_SHA256" 1
python3 tools/pi4_mg2_sparse.py replay NEW_DIR "$FREEZE_SHA256" 1 "$CONTROL_SESSION_SHA256"
python3 tools/pi4_mg2_sparse.py run NEW_DIR "$FREEZE_SHA256" 2
python3 tools/pi4_mg2_sparse.py replay NEW_DIR "$FREEZE_SHA256" 2 "$OBSERVED_SESSION_SHA256"
```

Position-specific filesystem paths distinguish the two sessions. The sole instrumentation change is loading the plugin. Replay returns 0 for a coherent completed capture (including two complete overlapping brackets), 2 for an incomplete observation, and 1 for a rejected artifact or contradiction. Independently retain the per-position replay and digests. No follow-up launch substitutes for an incomplete first attempt.

통합 담당자가 별도로 확인한 모델 SHA 및 빌드 ID, 헤더, 컴파일러 버전·인자·입출력 해시를 포함한 빌드 기록을 사용한다. 위 순서로 두 세션과 재생 결과를 각각 보존한다. 반환값 0이어도 두 구간이 겹치면 인과 순서는 미결이다. 2는 관측 누락, 1은 근거 거부를 뜻한다.

The Integration plugin build receipt is a JSON object with model_sha256, model_build_id, model_attestation_sha256, header_sha256, plugin_source_sha256, plugin_sha256, compiler_version (text) and compiler_argv (string array). Keep the underlying QEMU build attestation file with the receipt. Obtain digests from the actual source/header/object on the build host. The pinned linker map assigns event to 0x818d8..0x81a0b and signal to 0x838cc..0x8396f. LIVE[0] at 0x8d130 and TASK_LEASE[0] at 0x8d108 must both equal the observed M lease.

플러그인 빌드 기록의 JSON 필드는 위와 같다. 원본 모델 증명 파일의 해시와 실제 빌드 입력·출력 해시를 기록한다. 링커 맵의 두 함수 범위와 CPU0의 LIVE 및 M의 TASK_LEASE를 확인한다.
[Matched model build receipt / 일치 모델 빌드 증거](../evidence/pi4-mg2-plugin-model-build.en-ko.md) · #198 · #199.
[Matched off/on first positions and independent causal assessment / 동일 모델의 비계측·계측 최초 실행 및 독립 판정](../evidence/pi4-mg2-sparse-first.en-ko.md).
