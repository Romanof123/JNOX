# #190 third timer-observation cohort / 세 번째 타이머 관측 집단

[#190 retention](https://github.com/Romanof123/JNOX/issues/190) · [#186 observation](https://github.com/Romanof123/JNOX/pull/186) · [#184 timing](https://github.com/Romanof123/JNOX/issues/184) · [#6](https://github.com/Romanof123/JNOX/issues/6) · [Second-cohort contract / 두 번째 집단 계약](native-weighted-timer-second.en-ko.md)

The first pair lost its raw recordings. In the second cohort the plain capture exited 0, the official replay exited 2 because its Python lacked pyelftools, and the wrapper exited 1 after its Windows verifier could not resolve Get-FileHash. Linux reopened the archive and independent Windows Python verified all 19 retained members. The instrumented position never launched. Both prior paired timing results remain unqualified.

Freeze a distinct third directory on D: and two first positions in the same WSL invocation: plain share/default, then instrumented share/default. Use the original #183 input hashes and guest source, pinned QEMU, 20-second watchdog and second-cohort retention and exit gates. At 62.5 MHz Q=625,000, H=781,250 and G=156,250 counts. Require Q≤I−R≤H at each of 36 timer closures, every gap ≤G, every prefix −H≤2A−B≤2H, T=A+B≥36Q and |A/T−1/3|≤2H/(108Q). Preserve per-position raw files and any failure, including absent or ambiguous instrumentation.

Before launching either position, use the wrapper's actual Linux Python and its inherited environment to import the capture, timer observer and replay dependencies, including ELFFile, and read the pinned user ELF. Repeat that dependency probe in a child of the same executable; preserve the executable, package location/version and input identity. Exercise the production ZIP and verification path on a fixed small fixture on the actual empty D: destination. Native Windows PowerShell must independently hash the archive, manifest and decompressed members with .NET SHA256 streams and reopen both digest sidecars. Preserve the runtime identity and receipts. Preflight or retention infrastructure failure stops the sequence with a distinct infrastructure record and exit 2. A measurement miss remains exit 1, and paired success requires both qualified positions and complete retention (exit 0). No retry in the reserved directory.

첫 집단은 원시 기록을 잃었다. 두 번째 집단 일반 위치에서 캡처는 0, 공식 재생은 Python의 pyelftools 부재로 2, 래퍼는 윈도우 검증기의 Get-FileHash 확인 실패로 1을 반환했다. 리눅스가 ZIP을 다시 열었고 독립 윈도우 Python이 보존된 멤버 19개를 확인했다. 계측 위치는 실행되지 않았다. 앞선 두 집단의 쌍별 시간 판정은 미충족으로 남긴다.

별도의 빈 D: 디렉터리에서 한 번의 WSL 호출로 일반 share/default, 계측 share/default 순서로 각각 한 번 실행한다. #183 입력 해시와 게스트 소스, 고정 QEMU, 20초 제한, 두 번째 집단의 보존·종료 기준을 유지한다. 62.5 MHz에서 Q=625,000, H=781,250, G=156,250틱이다. 36개 타이머 구간마다 Q≤I−R≤H, 모든 공백 ≤G, 모든 누적 편차 −H≤2A−B≤2H, T=A+B≥36Q 및 |A/T−1/3|≤2H/(108Q)를 요구한다. 각 위치의 원시 자료·실패·누락·모호한 계측을 보존한다.

첫 실행 전에 래퍼의 실제 리눅스 Python과 동일한 환경에서 ELFFile을 포함한 캡처·계측·재생 의존 모듈을 불러오고 고정 사용자 ELF를 읽는다. 동일 실행 파일의 자식 프로세스에서도 반복해 인터프리터·패키지 위치와 버전·입력 식별자를 기록한다. 고정된 작은 자료에 실제 ZIP 보존 경로를 적용한다. 윈도우 PowerShell에서 D:의 ZIP·명세·압축 해제한 멤버를 .NET SHA256 스트림으로 다시 해싱하고 두 외부 해시 파일을 확인한다. 실행 환경과 검증 영수증을 보존한다. 사전점검·보존 인프라 실패는 기록 후 코드 2로 중단한다. 측정 미충족은 1, 두 위치 판정과 보존이 모두 완료된 성공은 0이다. 예약된 디렉터리에서 재시도하지 않는다.

```bash
PYTHONPATH=/home/ywcho/jnox-169-first-e203caa/deps /usr/bin/python3 tools/native_weighted_timer_pair.py /path/to/183/first/share-default /mnt/d/<fresh-third-directory> --scratch /tmp
```
