# Frozen deployment screen / 동결 실행 구성 검증
57b7f317fb1e7767a683099c3284541b406b9a0f. All five source unit tests passed. All 24 original captures from permit389-first-fbbb yielded byte-identical JSON and identical exit 1 to the first checker. All 960 segment and dispatch checks against the raw journal and session files matched. Observed timing failures and unknown analytical bounds stand.

All 25 malformed capture controls returned exit 2 UNKNOWN/HOLD with no timing segments. These include CPU, GIC/machine, SMP, accelerator, icount, duplicate override, emulator digest, counter frequency, RAM, missing probe, ordered/embedded ELF identity, kernel and launch linkage, new profile, build arguments and declared priority. Correct optional ordered peer files retained the expected 40 timing segments. The 24 allowlisted deployment fingerprints restrict conclusions to those frozen packets.

소스 단위 시험 5개가 통과했고 24개 원본의 960개 구간 및 스케줄 선택이 기존 독립 결과와 일치했다. 변형 25건은 모두 종료값 2, 구간 0개로 거부됐다. 실제 시간 초과와 분석적 수용 보류는 유지된다. 새 배포 프로필은 별도의 검증이 필요하다.

Commands: python3 -m unittest -v test_native_safety_admission; python3 verify.py OUTPUT EXTRACTED_ROOT ORIGINAL_REVIEW ORIGINAL_CAPTURES; python3 audit.py OUTPUT EXTRACTED_ROOT ORIGINAL_CAPTURES. The original 24 capture SHA-256 digests are in crosscheck.json. The original independent report SHA-256 was caae53761b568c72e2e18d6608f13d9413703f2ca8cf9717874d15ef76471250; first corrected receipt was 919167326c3f247a41491757ac61e8b2ad7527d6dc46b0dbfadb76d48713885c.

[Raw captures and offline replay](native-safety-admission-platform.tar.gz) (SHA-256 `f376adc58b3c37ff6d031d19fa4cfbe1679f7224b5f27b285f6a6c87ad7ee760`).
