# Native Picolibc and Pixman guest evidence

Three QEMU virt boots used kernel source 6764dfed9f0556e47df3a1f6069b2fa5c95cfcbb. The original 0.44.0 Pixman archive linked with Picolibc 1.8.10 into probe ELF 9f9b89dda8a807616d864effd19ec6031f955bfe92b8a54339084ac9dc7bf25f. The successful normal boot produced the following four ARGB8888 pixels in row-major order.

| Process | Output pixels | Exit |
|---|---|---|
| p | ff504040 ff123456 ff1267ab ff365d68 | 71 |
| q | ff303070 ff123456 ff1267ab ff365d68 | 72 |

The independent host integer oracle matched all eight pixels. Both local FP workloads began before either ended. The filtered trace recorded two original Pixman constructors, two original pixman_image_composite32 entries and 64 graphics_fp_yield entries. Both normal destructors reported completion. The intentionally failing assertion printed its source expression, called abort and exited 134. The grant control printed one allowed marker; the ordinary second process obtained EACCES for stdout and stderr, emitted no forbidden marker and exited 74. The granted process checked the expected EBADF and EFAULT paths before exiting 73.

The source trees, Picolibc developer signature, tool versions and static libraries are identified in runtime.json. runs.json lists the exact native kernel and user ELF hashes, arguments, grants, and SHA256 of the UART/trace inputs beside it. The per-boot symbols.json files come from the packaged user ELF. Reproduce and replay using the [runtime guide](../../../sdk/native-aarch64/graphics/README.md). The measurements cover this two-process target profile and assertion/permission controls. Running upstream Xwayland remains a separate milestone.

## 한국어

JNOX 네이티브 게스트에서 Picolibc와 Pixman 원본을 연결한 ELF를 실행했다. 두 프로세스의 실제 합성 결과 8개 픽셀이 정수 기반 독립 계산과 일치했다. 실행 추적에서 Pixman 생성자와 합성 함수에 각각 두 번 진입했고 두 프로세스가 스케줄러에 양보한 횟수는 총 64회였다. 별도 부트에서 assert 진단과 종료 코드 134, 선택적으로 부여한 콘솔 권한과 비인가 프로세스의 EACCES를 확인했다. 이 결과는 Xwayland 서버 자체의 실행을 포함하지 않는다.
