# Native C permit codec / 네이티브 C 허가 프레임 코덱

`main.c` encodes a permit into 44 private bytes and decodes it with the same Rust implementation as the ordinary Rust producer and receiver. Exit 0 means all four fields survived the round trip. The example uses a fixed sample tick.

`main.c`는 허가를 사설 버퍼 44바이트에 기록하고 Rust 송수신기가 쓰는 동일한 Rust 코덱으로 판독한다. 종료값 0은 네 필드가 원래 값과 일치함을 뜻한다. 예제의 발행 틱은 고정값이다.

## Build / 빌드

Run from the repository root with Clang, LLD, Cargo and the Rust target installed. The SDK script builds the local Rust static library offline and links the C application with the ordinary JNOX entry point, ABI note and linker script. Use the same option with your own C source.

저장소 루트에서 Clang, LLD, Cargo 및 아래 Rust 타깃을 사용한다. SDK는 로컬 Rust 정적 라이브러리를 오프라인으로 빌드하고 일반 JNOX 시작 코드, ABI 노트 및 링커 스크립트로 C 앱을 링크한다. 다른 C 앱도 같은 옵션을 사용한다.

```sh
rustup target add aarch64-unknown-none-softfloat
sh sdk/native-aarch64/build.sh --permit-codec examples/native-permit-codec/main.c permit-codec.elf
```

## Buffers and results / 버퍼와 반환값

Include `<jnox/permit_codec.h>`. Pass the four host fields by value to `jnox_permit_encode`. For `jnox_permit_decode`, pass an initialized private snapshot and a separate, aligned `jnox_permit_fields` destination. Each call returns 0 after writing its entire destination. On any error, the destination retains its previous contents. A destination may start uninitialized and can then be read only after success.

`<jnox/permit_codec.h>`를 포함한다. 인코딩에는 필드 네 개를 값으로 넘긴다. 디코딩에는 초기화된 사설 프레임과 별도로 정렬된 결과 구조체를 넘긴다. 성공 시 출력 전체를 쓰고 0을 반환한다. 실패 시 출력 메모리를 그대로 보존한다. 초기화하지 않은 출력은 성공한 뒤에만 읽을 수 있다.

| Status / 반환값 | Meaning in check order / 검사 순서와 의미 |
| --- | --- |
| `ARGUMENT` (−1) | Any required pointer is null / 필수 포인터가 널임 |
| `LENGTH` (1) | Buffer length differs from 44 / 버퍼 길이가 44와 다름 |
| `ARGUMENT` (−1) | Encode input permit is outside 0–1 / 인코딩할 허가 값이 0 또는 1의 범위를 벗어남 |
| `CRC` (2), `LENGTH` (1), `FORMAT` (3) | Decode checks CRC first, then embedded length, then fixed fields, permit and reserved bytes / 디코딩 시 CRC, 내장 길이, 고정 필드·허가·예약 바이트 순서로 검사 |
| `GENERATION` (7), `SEQUENCE` (8) | Either direction rejects zero generation first, then zero sequence / 양방향 모두 세대 0, 순번 0 순서로 거부 |

The C status constants have the prefix `JNOX_PERMIT_`. The producer supplies the actual issue time in common guest monotonic counter ticks and preserves it across publication retries. The wire validity field is 20 milliseconds. On receipt, validate the kernel-authenticated route and loan metadata, copy into private storage and release the loan. Successful decode establishes wire validity. The receiver checks the active generation, next sequence, issue ordering and freshness against the live counter before the controller requests COMMIT. OPEN and REARM use the separate supervisor authority.

C 상태 상수의 접두사는 `JNOX_PERMIT_`이다. 송신자는 공통 게스트 단조 카운터로 실제 발행 틱을 기록하며 게시를 재시도해도 그 값을 유지한다. 프레임의 유효 기간 필드는 20밀리초다. 수신자는 커널이 인증한 경로와 대여 메타데이터를 검증한 뒤 사설 버퍼에 복사하고 대여를 반환한다. 디코딩 성공은 프레임 형식과 내용 검사를 통과했다는 뜻이다. 제어기가 COMMIT을 요청하기 전에 수신자가 현재 세대, 다음 순번, 발행 순서 및 현재 카운터 기준 유효 기간을 검사한다. OPEN과 REARM은 별도 감독자 권한을 사용한다.

[Wire contract / 프레임 규약](../../docs/native-safety-channel.en-ko.md) · [C SDK #405](https://github.com/Romanof123/JNOX/issues/405) · [Channel #11](https://github.com/Romanof123/JNOX/issues/11) · [Expiry acceptance #401](https://github.com/Romanof123/JNOX/issues/401)
