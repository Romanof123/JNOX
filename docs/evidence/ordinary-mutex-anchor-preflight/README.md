# Default ordinary PI comparison / 기본 PI 비교

Candidate da8005510e3d240660fc1f8f445161826852a291. First build succeeded, zero launches. Reference is the unchanged first PI image in the CHG-067 archive from 47ea89049b3efd2dd74642562a9973e15ad26a15. Both files are 99,464 bytes. Whole-file and first PT_LOAD byte equality failed. All seven differing bytes occur in six little-endian u32 line fields within .rodata. Each 24-byte record has the same pointer to the 21-byte path src\ordinary_mutex.rs, length, and source column across the two images. Each line increases by 18. The extra differing byte at 0x142a1 is the carry from 241 to 259.

| File offset(s) | Original bytes | Candidate bytes | Original → candidate line | Column |
| --- | --- | --- | --- | --- |
| 14270 | d8 | ea | 216 → 234 | 17 |
| 14288 | cf | e1 | 207 → 225 | 45 |
| 142a0–142a1 | f1 00 | 03 01 | 241 → 259 | 13 |
| 142b8 | e9 | fb | 233 → 251 | 17 |
| 142d0 | 2f | 41 | 303 → 321 | 31 |
| 142e8 | 5c | 6e | 348 → 366 | 90 |

Offsets and bytes are hexadecimal. File offsets map to virtual addresses by adding 0x401f0000 in this segment. The .text section, loaded .data, all symbol and section tables and headers match byte for byte. The default image therefore executes the same linked instructions as its retained predecessor, with unchanged transfer targets and no added executable hook. Panic location diagnostics can report different source lines. No guest or timing outcome follows from this comparison. Ceiling, legacy and optional images require separate checks.

최초 기본 PI 빌드는 성공했으며 게스트 실행은 0회였다. 전체 ELF와 첫 번째 PT_LOAD의 바이트 동일성 검사는 실패했다. 차이 7바이트는 모두 .rodata에 있는 소스 위치 레코드 여섯 개의 행 번호에 해당한다. 각 행 번호는 18 증가했고 파일 경로, 열 번호, 실행 코드와 나머지 모든 바이트는 동일했다. 따라서 기본 PI에는 실행 가능한 추가 훅이 없으며 패닉 진단에 표시되는 행 번호가 달라질 수 있다. 이 비교에는 실행 시간 측정값이 없다. 기본 ceiling, 기존 벤치마크 및 선택형 이미지의 검사는 별도로 남아 있다.

## Build log provenance / 빌드 로그 출처

The public log replaces one absolute temporary workspace prefix in the first Cargo status line with `<WORKSPACE>`. The relative suffix `\source\kernel`, diagnostics, arguments present in the log and original CRLF endings are preserved. The distributed ELF is byte-identical to the measured image. The remaining evidence text and ELF contain no identified personal home or temporary workspace path.

공개 로그에서는 첫 Cargo 상태 행에 있는 임시 작업 공간의 절대 경로 접두부 한 곳을 `<WORKSPACE>`로 대체했다. 그 뒤의 `\source\kernel`, 진단 메시지, 기록된 인수 및 원본 CRLF 줄 끝을 보존했다. 공개 ELF는 측정한 원본 바이트와 일치한다. 나머지 증거 텍스트와 ELF에서 개인 홈이나 임시 작업 공간 경로는 발견되지 않았다.

| Build log / 빌드 로그 | Bytes | SHA-256 |
|---|---:|---|
| Original / 원본 | 3971 | fb2f03a7c2877b3b75b5fafd116f0f3bc6d7628ca97fdb4cf39072ae8be76652 |
| Public / 공개본 | 3933 | e4e105745b6c40bb10308dc50711be1d39d0b615e5180c3396d4d95b2ba05894 |
