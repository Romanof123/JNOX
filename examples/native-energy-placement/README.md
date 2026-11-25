# Modeled Pi 4 placement / Pi 4 모델 기반 배치

Package one ordinary AArch64 ELF and its argument vector with a two-CPU model. From the repository root, use a Linux host with the native SDK, AArch64 binutils and QEMU with the raspi4b machine. Use fresh output directories. For byte comparisons with the original deployment, supply the retained prime ELF. The SDK command below rebuilds it from source.

저장소 루트에서 일반 AArch64 ELF 하나와 인자 벡터를 두 CPU 모델과 함께 패키징한다. 새 출력 디렉터리를 사용한다. 기존 실행과 바이트 단위로 비교하려면 보관한 ELF를 사용하고 해시를 기록한다.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-fixed/prime_low.c prime.elf
sha256sum prime.elf
python3 examples/native-energy-placement/profiles.py profiles
for model in base thermal shared fallback empty deadline zero-mask; do
    python3 tools/build_native_el0.py --board pi4 --policy energy-placement \
        --elf prime.elf --placement-profile "profiles/$model.json" --out "image-$model" || exit 1
done
qemu-system-aarch64 -M raspi4b -kernel image-base/kernel8.img -display none -serial stdio -monitor none
~~~

For another program, supply its ELF and repeat `--arg VALUE` for each argument after argv[0]. The manifest records the SHA-256 of the compact UTF-8 JSON argv array, the ELF, the original profile in `placement-source.json` and its canonical form in `placement-v1.json`. One model tick is floor(CNTFRQ/10) physical counter ticks. The seven inputs cover baseline CPU0, CPU0 thermal exclusion, CPU0 shared load, an empty catalogue with a trusted CPU1 fallback, an empty catalogue alone, a predicted deadline miss and a zero eligible mask. When dispatched, the prime program expects a private count of 78,498 and exits 12. The eighth input omits a mandatory CPU limit and must be rejected at packaging.

다른 앱은 해당 ELF와 추가 인자별 `--arg VALUE`를 사용한다. 매니페스트는 공백 없는 UTF-8 JSON argv 배열, ELF, 원본·정규화 프로필 각각의 SHA-256을 기록한다. 모델 한 틱은 물리 카운터 floor(CNTFRQ/10)틱이다. 일곱 입력은 기본 CPU0, CPU0 열 제한, CPU0 공유 부하, 신뢰 CPU1 대체점, 빈 목록, 예상 기한 초과, 빈 CPU 마스크를 다룬다. 실행 시 기대 소수 개수는 78,498이고 종료값은 12이다. 여덟 번째 입력은 필수 CPU 한도를 누락해 패키징 단계에서 거부돼야 한다.

~~~sh
python3 tools/build_native_el0.py --board pi4 --policy energy-placement \
    --elf prime.elf --placement-profile profiles/missing-limit.json --out invalid-limit
~~~

Operating points and energy costs are model inputs. Physical calibration, clock control and deployment approval require separate evidence. 운용점과 에너지 비용은 모델 입력이며 실물 교정·클록 제어·제품 승인은 별도 근거가 필요하다. [Case definitions / 사례 정의](../../docs/pi4-native-energy-placement.en-ko.md)
