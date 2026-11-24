# Pi 4 MG2 · Integration build of a plugin-capable QEMU model / 플러그인 모델 통합 빌드

Integration built the AArch64 system emulator from the matched QEMU 11.1.3 sources. The planning and build target in [#198](https://github.com/Romanof123/JNOX/issues/198) now has a distinct executable identity. This stage ran zero guests and loaded zero observer plugins. The six original migration positions remain 0/6. [Machine-readable build receipt, commands, compiler configuration and log digests](pi4-mg2-plugin-model-build.json)

| Artifact | SHA-256 | GNU build ID |
|---|---|---|
| Historical shipping ELF | `3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862` | `01d47c7221d14fdfe82890c76e1056d4cbfe1ee8` |
| Fresh plugin-capable ELF, unstripped | `2e465436514c0dc203eb2e9c3440187252cb84ebcdcc3fee6de76c9fa1c95fe6` | `ef73da8a108e97b7d11566630ac676893c2f3ef8` |

The source archive SHA-256 was `5a8289c7940dc77d3873c81a6c29ba3afe1446fc9d36e77fa1658cdbecb84b22`. The original project’s local release package contained the same source archive, patch inputs and shipping binary. Against the preserved historical build tree, 78,701 regular upstream files and 72 symlinks were checked during preflight. Seven upstream files had local edits and four ARM-Speed files were added. All eleven resulting files matched the frozen SHA-256 manifest `0c17eb979eb82cb0cfe87f86e52eba44ddb968426bec44d4882259e5d42580d5` before configure. One extra blank line from the release patch generator was removed by restoring the historical file. The historical build artifact matches the shipping ELF’s GNU build ID and all 29 allocated sections.

The configure arguments differed only at `--disable-plugins` → `--enable-plugins`. Host compiler/linker identification matched. Both historical target triples were configured. Ninja built only `qemu-system-aarch64`, running on one host CPU with `-j1`, through step 2207/2207. The resulting ELF exports 65 standard `qemu_plugin_*` API symbols. The host checked the translation, execution, memory, hardware-address, memory-value and register-access symbols by name. The shipping ELF still has its original SHA-256. Local raw logs are identified by digest in the JSON. External binary or source distribution did not occur.

Use the plugin-capable ELF’s SHA-256 for its future paired plugin-off and plugin-on model observations. The [historical first runs](pi4-migration-mg2-first.en.md) and the [Stage 1 receipt](pi4-mg2-terminal-first.en-ko.md) retain their original model attribution. Independent review, the actual checkpoint/SGIR causal observations and physical qualification remain pending.

## 한국어

통합팀은 QEMU 11.1.3 원본 보관본과 기준 ARM-Speed 변경 파일 11개의 해시를 확인한 뒤 별도 Linux 디렉터리에서 모델을 빌드했다. 준비 스크립트가 추가한 빈 줄 한 줄은 역사적 파일로 복구했다. 이전·신규 구성의 유일한 옵션 차이는 플러그인 활성화이며 호스트 컴파일러·링커 식별이 일치한다. 두 대상 구성을 유지한 채 AArch64 시스템 실행 파일 하나만 CPU 한 개·Ninja 한 작업으로 생성했다. 모델의 해시와 GNU 빌드 ID는 위 표에 있다.

신규 ELF에서 `qemu_plugin_*` 공개 심볼 65개와 관측에 필요한 메모리·레지스터·실행 콜백을 정적으로 확인했다. 기존 배포 파일의 해시는 유지되었다. 이번 단계의 게스트 실행은 0회이고 플러그인 로드는 0회다. 원래 MG2 이주 자격은 0/6이며, 신규 모델 안의 대조 실행·M 체크포인트와 SGIR의 인과 순서·실물 판정은 남아 있다.

[Compact model-build attestation / 모델 빌드 증빙 묶음](pi4-mg2-plugin-model-build.zip) · ZIP SHA-256 `f355ad833089768e24cba293555c39acf3e4e602a9a10f5aaa7da40527c30eca` · 8,926 bytes.
