# Native XKB and bitmap font guest evidence

The archive-only XKB image executed XkbRF_Create, XkbRF_Free and XkbModIndexText in an AArch64 JNOX EL0 guest and exited 0. Its assertions verify allocated rule capacity and the original Control modifier name. The Xlib-dependent version call belongs to a separate probe.

The font image opened 6x13.builtin with the original libXfont2 implementation. QEMU recorded execution of the BuiltinFileOpen and zlib inflate entry PCs. UART reported the thirteen rows of uppercase A as 0000040a1111111f1111110000 before the native exit 0. This measures embedded font decoding. X11 draw requests and Xwayland scanout remain separate tests.

The full local bundle at D:/codex/jnox448-xwayland/font-xkb-gate contains the two packaged kernels, exact native executables, QEMU execution traces, UART and six attempts. The ELF and complete static archive digests are recorded in SHA256SUMS. Replay check-dep-guest.py on font.log or xkb.log with its matching trace and symbol file. font-xkb-guest.sh uses the serial WSL gate and a fresh attempt name.

Attempts 01 and 02 exposed checkout line endings after XKB execution. Attempt 03 reached real zlib gzclose dependencies on write and close. Attempt 04 exposed the missing compiler library search directory for -lm. Attempt 05 built the complete font archive and then failed to link the optional lsfontdir test utility, including missing open, stat and fstat. Attempt 06 installed the upstream archive through its library installation targets and passed the glyph guest.

한국어: JNOX에서 원본 XKB 함수 세 개의 실행 주소와 종료값 0을 확인했다. 실제 내장 PCF와 zlib를 사용한 글꼴 게스트는 대문자 A의 13줄과 종료값 0을 출력했다. 별도의 lsfontdir 도구에는 파일시스템 ABI 보완이 필요하다. X11 서버의 화면 출력은 후속 검증 대상이다.
