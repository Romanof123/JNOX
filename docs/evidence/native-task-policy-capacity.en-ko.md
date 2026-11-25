# J-NOX #394 | Native deployment capacity / 네이티브 배포 수용 한도

**PASS / 통과.** The normal text packager rejected both over-limit deployments before publishing the requested directory. The two adjacent, fitting configurations each packaged there and booted to `JNOX POLICY DONE`. A fresh replay produced byte-identical policies and kernels and the same per-task exit results.

일반 텍스트 패키저가 한도를 넘은 배포 두 건을 각각 거부했다. 두 번 모두 출력 경로가 생기지 않았고 상위 디렉터리의 항목도 그대로였다. 그 경로로 한도 바로 아래의 두 구성을 차례로 패키징하고 부팅했다. 독립 작업 디렉터리에서 전체 절차를 반복해 정책·커널 바이트와 태스크별 종료 결과의 일치를 확인했다.

| Text input / 정책 | Distinct apps × tasks per app / 앱 × 태스크 | Packager exit | Observed result / 관찰 결과 |
|---|---:|---:|---|
| `ram31.txt` | 31 × 1 | 2 | `policy exceeds virt RAM`. Output absent / 출력 없음. |
| `stack31.txt` | 1 × 31 | 2 | `policy count exceeds ASID or stack layout`. Output absent / 출력 없음. |
| `ram30.txt` | 30 × 1 | 0 | `node01/work01` … `node30/work30` exited with 51 … 80 respectively. |
| `stack30.txt` | 1 × 30 | 0 | `stacks/slot01` … `stacks/slot30` exited with 101 … 130 respectively. |

The fixture SDK build produced 31 distinct single-task ELFs plus one ELF exporting 31 distinct task functions. Every ELF SHA-256 was unique. `nm` and `readelf` verified each named symbol at an aligned executable address. The RAM check requires 133,408,520 bytes for these 31 apps against 132,120,576 bytes available to the kernel on the 128 MiB target. The 31-task test uses just one ASID and exceeds the documented per-app 30 guarded stacks by one. The packager's literal combined ASID/stack diagnostic is quoted above.

31개 앱은 서로 다른 실행 파일과 진입 심볼을 사용했다. 스택 시험에서는 실행 파일 하나에 서로 다른 함수 31개를 넣었다. 31개 앱의 합산 요구량은 128 MiB 머신의 커널 가용량보다 1,287,944바이트 컸다. 스택 시험은 앱 식별자 하나만 사용하므로 거부 원인은 앱당 보호 스택 30개 한도다.

Both fitting runs used the documented Cortex-A72, TCG, GICv2 and single-CPU launch, with a local QMP socket for observation. QMP reported CPU index 0 as the only CPU, GIC version 2 and 134,217,728 bytes of base memory. For each of the 60 declared task instances, UART showed two releases, two starts, one completion, the fixture's own exit status and a summary with one exit and zero misses, watchdogs or faults. Both bootable packages, original UART and QMP replies are included. Scope covers QEMU functional behavior. Physical timing and product safety approval fall outside this verification.

적합한 두 구성에서 QMP로 단일 CPU·GICv2·128 MiB를 확인했다. UART에서 총 60개 태스크 각각의 이름을 대조했고, 릴리스 2회·시작 2회·완료 1회·지정 코드 종료 1회와 마감 누락·감시 만료·고장 0회를 확인했다. 검증 범위는 QEMU 기능 동작이다. 실물 시간 성능과 제품 안전 승인은 별도다.

## Source and hashes / 소스와 해시

Clean detached commit `002c47c1ad311ec347310662aa256011f4f5388f`. The complete Git tree is `cf7e7792da0c9dd12636c8b28daf711178e89c85`. The inventory `source-tree.txt` lists all 1,840 tracked paths and object IDs, including paths outside the sparse build checkout. Its SHA-256 is `0d3bc0b98ca84bf9f7d4b1bc4dbae35a7301bc7ea299f0a32919b0be76e0eb63`. Git status was clean before and after both runs. [Source commit](https://github.com/Romanof123/JNOX/commit/002c47c1ad311ec347310662aa256011f4f5388f)

| Accepted package | kernel.elf SHA-256 | policy.bin SHA-256 |
|---|---|---|
| 30 apps | `daba398dcec27fb17119e5bed893ee74bf64db49292a4f50cb22aff7ea225447` | `f214c885aff66a5da933bc620ea2b61be2ff845e8876641e51e860e1ea3b801e` |
| 30 stacks | `f204153e85b68411a14009a9cab4a443448e33c9e47c5d3c6cbe5b76a1f1c9e4` | `470332fc87d6132eefb4a58f7bb0121a61c7d249b4a4a856161cf6ca5e678e91` |

The verifier matched every packaged ELF byte range, manifest entry address, source text digest and named task against its fixture. Each complete policy was present once in its kernel. `SHA256SUMS` covers the original inputs, both output packages, raw logs and verifier scripts.

## Commands / 재현 명령

From the unpacked evidence directory, supply the fresh run path and the clean source checkout. Use the versions in `versions.txt` with the kernel's Rust target `aarch64-unknown-none`. The replay builds the app ELFs sequentially with the checked-out SDK, executes both rejections, then builds and boots each fitting case. After each boot it preserves the successful package and frees the shared output path for the next case.

압축을 푼 증거 디렉터리에서 아래 명령을 실행한다. 마지막 인자는 새로 만들 경로다. 각 실행의 실제 인자, 종료 코드와 진단은 `logs/`에 있고, UART 전체 기록과 태스크별 대조표도 함께 들어 있다.

```sh
sha256sum -c SHA256SUMS
git clone --filter=blob:none --no-checkout https://github.com/Romanof123/JNOX.git /path/to/source
git -C /path/to/source sparse-checkout init --cone
git -C /path/to/source sparse-checkout set arch boards examples/native-task-policy examples/portability kernel sdk services tools
git -C /path/to/source checkout --detach 002c47c1ad311ec347310662aa256011f4f5388f
bash replay.sh /path/to/source /path/to/new-run
```

The public packager and guest commands used by the replay have this form. `CASE` is one of the four table rows without its extension. The guest runs only after a successful package.

```sh
python3 tools/build_native_el0.py --board virt --task-policy "$inputs/$CASE.txt" --out "$out/package"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
  -accel tcg -m 128M -nodefaults -display none -serial mon:stdio \
  -nic none -no-reboot -kernel "$out/package/kernel.elf" \
  -qmp "unix:$log/$CASE.sock,server=on,wait=off"
```

[Raw captures and offline replay](native-task-policy-capacity.tar.gz) (SHA-256 `0e7868c73bcec9d20738a66e58f195873331d87cf3e0545139d49f30febfe823`).
