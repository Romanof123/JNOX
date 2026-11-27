# Native XKB archive and link / 네이티브 XKB 아카이브와 링크

CHG-340 · [Xwayland port #448](https://github.com/Romanof123/JNOX/issues/448) · [Integration #452](https://github.com/Romanof123/JNOX/pull/452)

The first successful link used the original libxkbfile 1.1.3 release, the public headers from libX11 1.8.10, the staged xorgproto/Xtrans target prefix and the native 6764dfe Picolibc/Pixman graphics runtime. The build selected all thirteen C translation units from libxkbfile's upstream Meson list. Its archive check inspected the machine type of every object. The linked JNOX ELF has entry point `0x400000`, three R-X/R--/RW- load segments, a native ABI note and zero undefined symbols. [Source archives and pinned digests](xkb-sources.sha256) · [Member list](archive.members) · [ELF headers](probe.headers)

| Original artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| libxkbfile.a | 259138 | `72875f0138602d01291e833d6640f2b752a32c5ff16fab9b2cd3babadbf76b72` |
| probe-archive.elf | 94816 | `e6419aadf632c02a189bfeec807c0033becae79effb9c6737a24b0dfd1eee52f` |
| probe-archive.map | 94575 | `7bebd80ee4fd03ba063bb1cafee82f69626fac04e12e17f831cafbe705abe99c` |

The link map resolves `XkbRF_Create` and `XkbRF_Free` to `maprules.c.o` and `XkbModIndexText` to `xkbtext.c.o` in that archive. The first runner attempt failed to resolve a Windows Git worktree path from WSL before compilation. The next attempt used the same source revision through its Git object directory. GNU readelf warned about unrecognized debug forms; the retained LLVM header report identifies the target segments. Guest exit, the full X11 dependency link, Xwayland configure and guest rendering require separate gates. [First attempt](attempt01.log) · [Successful attempt](build.log) · [Link map](probe-archive.map) · [Symbols](probe.symbols)

## Verify and reproduce

From the repository root, verify all ten retained text records. The frozen build retains the two binaries separately. Supply their directory to check both against the original digest ledger.

```sh
evidence=verification/xwayland/evidence/xkb-archive
python3 "$evidence/verify.py"
python3 "$evidence/verify.py" --binaries "$FROZEN_BINARY_DIRECTORY"
```

The captured runner used local commit `2f8e3071bf5f57a51b1cfd79fabdd729947deb4d`. The builder, probe and source pins are identical in public revision `49013d671b401b0208f9a25c0216b5bdb38a22d1`. In a checkout of that revision, use the graphics runtime at its original absolute path and a fresh output directory. The link map embeds absolute build paths, so a different directory changes its hash. [Recorded lab runner](recorded-runner.sh) · [Runtime and protocol preparation](../../xkbfile.en-ko.md) · [Public source revision](https://github.com/Romanof123/JNOX/tree/49013d671b401b0208f9a25c0216b5bdb38a22d1)

```sh
sh verification/xwayland/xkb-fetch.sh "$CACHE"
python3 verification/xwayland/xkb-build.py --cache "$CACHE" --runtime "$RUNTIME" \
  --protocols "$PROTOCOL_TARGET" --out "$NEW_XKB_WORK"
```

## 한국어

고정된 libxkbfile 1.1.3 원본의 C 파일 13개를 AArch64 오브젝트로 빌드했다. 정적 프로브의 진입점은 `0x400000`이다. 링크 맵에서 규칙 구조체 할당·해제 함수와 수정키 이름 변환 함수가 해당 아카이브에서 선택되었음을 확인할 수 있다. 첫 시도는 WSL의 Git 경로 해석에서 멈췄고 두 번째 시도에서 링크가 완료됐다. GNU readelf의 디버그 정보 경고와 LLVM 도구로 별도 추출한 ELF 헤더를 함께 보존했다.

위 검증 명령은 공개한 텍스트 기록의 해시와 함수 출처를 확인한다. 별도 보관한 원래 ELF 및 아카이브가 있으면 `--binaries`로 두 파일까지 대조할 수 있다. 이 기록은 타깃 컴파일과 링크까지 다룬다. 게스트에서 프로브의 종료 코드 확인, 전체 X11 의존성 링크, 서버 구성과 실제 화면 출력은 후속 검증 항목이다.
