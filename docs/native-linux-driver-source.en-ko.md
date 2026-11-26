# CHG-325 · Native services built from Linux driver source

[Driver-source issue #424](https://github.com/Romanof123/JNOX/issues/424) · REQ-004 · REQ-011 · DES-002 · VER-004 · VER-011

Rebuild unchanged, hash-pinned upstream driver source with a bounded adapter as an unprivileged native J-NOX ELF. Reject unsupported APIs at build time. The J-NOX Rust kernel owns grants for device memory, IRQs and DMA. Native Rust drivers continue to use the native driver path. Historical Linux-module and gateway trials remain reference comparisons. Native services require neither Linux nor Bao and assume no compatibility with existing .ko binaries.

IMP-325 targets a PL031 read through an assigned MMIO range. EVD-325 is the planned evidence record for source and ELF identity, an actual RTC read and a denied device access. IRQ and DMA grant enforcement each require separate device exercise. A later virtio_net port depends on kernel networking support and enforced DMA mappings for the device queues.

## CHG-325 · Linux 드라이버 소스로 빌드하는 네이티브 서비스

해시로 고정한 원본 Linux 드라이버 소스를 수정 없이 제한된 어댑터와 함께 비특권 J-NOX 네이티브 ELF로 빌드한다. 미지원 API는 빌드에서 거부한다. J-NOX Rust 커널이 장치 메모리·IRQ·DMA 권한의 부여를 담당한다. 네이티브 Rust 드라이버는 자체 경로를 계속 사용한다. 기존 Linux 모듈 및 게이트웨이 시험은 참조 비교로 보존한다. 네이티브 서비스에는 Linux 커널이나 Bao가 필요하지 않으며 기존 .ko 바이너리 호환성을 가정하지 않는다.

IMP-325의 첫 대상은 할당된 MMIO 범위를 통한 PL031 읽기다. EVD-325에는 소스와 ELF 식별값, 실제 RTC 읽기, 비인가 장치 접근 거부 결과를 기록할 계획이다. IRQ·DMA 권한 강제는 각각 장치 시험이 필요하다. 이후 virtio_net 이관은 커널 네트워킹 지원과 장치 큐용 DMA 매핑 강제에 의존한다.
