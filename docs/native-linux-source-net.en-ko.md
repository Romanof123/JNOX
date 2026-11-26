# CHG-326 · Native virtio-net feature validation / 네이티브 virtio-net 기능 검증

**REQ-004 · REQ-011 · DES-002 · IMP-326 · VER-004 · VER-011**  
EVD-326 records two native QEMU cold boots and independent replay of the original feature-validation callback at EL0. Both captures used the same ELF and different device settings. [Change request #426](https://github.com/Romanof123/JNOX/issues/426) · [Evidence and replay](evidence/native-linux-source-net/README.md) · [Measurements](evidence/native-linux-source-net/result.json)

## Design and acceptance

The importer hashes the unchanged Linux stable v6.18.65 `drivers/net/virtio_net.c` and compiles the full translation unit with the pinned AArch64 headers. It copies the original object, exposes the local `virtnet_validate` symbol for linkage and compares its function-section bytes before and after this change. The native ELF links the callback and its required adapter functions. Retain the original object, derivative object, link map, input identities and final ELF hash to establish which upstream code can execute. [Import profile](../drivers/linux-compat/profiles/virtio-net.json) · [Importer](../tools/build_linux_source_virtio_net.py)

AArch64 QEMU virt boots J-NOX directly. The trusted package selects two ordinary FIFO processes. Slot 0 runs the callback at EL0 under the J-NOX ABI. The kernel mediates access to the modern virtio-net MMIO transport at `0x0a003e00` (512 bytes) through calls for claim/release, feature reads, configuration reads, feature commit and status. Slot 1 has its own address space and no device grant. The package owner is responsible for the supplied ELFs. Preserve their identities with the capture. [Device interface](../kernel/src/native_linux_net.rs)

In each boot the CPU trace showed three executions of the linked callback. It returned 0 for the device offer and −EINVAL for both error cases. The device reached `FEATURES_OK` and slot 0 released and reclaimed the device. Slot 1 received −EACCES on all five device calls during the first claim and recorded six timestamps across five timed sleeps. [EVD-326](evidence/native-linux-source-net/README.md)

Packet transmission and reception through the original Linux driver remain open. They require native virtqueues and packet buffers, enforced DMA bounds, and IRQ delivery and acknowledgement. Restart acceptance also requires a new service instance, retirement of its predecessor's outstanding buffers and completions, and resumed packet exchange with an independent peer. [Packet-path design](native-linux-net-packet-design.en-ko.md)

## 설계 및 인수 기준

EVD-326에는 장치 설정을 바꾼 두 차례의 콜드 부팅 기록과 동일 ELF에 대한 독립 재검증 결과를 수록했다. 임포터는 Linux stable v6.18.65의 `drivers/net/virtio_net.c` 원본 해시를 검사하고, 고정된 AArch64 헤더로 전체 파일을 수정 없이 컴파일한다. 원본 오브젝트의 복사본에서 `virtnet_validate` 심볼을 외부 링크가 가능하도록 바꾸고 전후 함수 섹션 바이트를 대조한다. 네이티브 ELF에는 이 콜백과 필요한 어댑터 함수가 링크된다. 원본·파생 오브젝트, 링크 맵, 빌드 입력 식별값과 ELF 해시로 실행 코드의 출처를 확인한다. [임포트 프로파일](../drivers/linux-compat/profiles/virtio-net.json) · [빌드 도구](../tools/build_linux_source_virtio_net.py)

AArch64 QEMU virt에서 J-NOX를 직접 부팅한다. 신뢰하는 패키지 구성이 일반 FIFO 프로세스 두 개를 선택한다. 슬롯 0은 J-NOX ABI로 EL0에서 원본 콜백을 실행한다. 커널은 `0x0a003e00`부터 512바이트인 virtio-net MMIO 영역에서 장치 점유·반환, 기능·설정 읽기, 기능 승인 및 상태 조회를 중재한다. 슬롯 1은 별도 주소 공간에서 실행하며 장치 권한이 없다. 패키지 작성자가 공급한 두 ELF의 식별값을 실행 기록에 함께 보존한다. [장치 인터페이스](../kernel/src/native_linux_net.rs)

두 부팅 모두 CPU 추적에서 콜백 실행 세 번을 확인했다. 정상 장치 구성은 0, 오류 사례 두 건은 각각 −EINVAL을 반환했다. 장치는 `FEATURES_OK`에 도달했고 슬롯 0이 같은 프로세스에서 장치를 반환하고 다시 점유했다. 최초 점유 중 슬롯 1이 시도한 장치 호출 다섯 가지는 모두 −EACCES였고, 슬롯 1은 시각 대기 다섯 번에 걸쳐 단조 시각 여섯 개를 남겼다. [EVD-326](evidence/native-linux-source-net/README.md)

원본 Linux 드라이버를 통한 패킷 송수신은 후속 과제다. 네이티브 virtqueue와 패킷 버퍼, DMA 접근 범위 강제, IRQ 전달·확인을 검증해야 한다. 재시작 인수에는 새 서비스 인스턴스의 기동, 이전 인스턴스의 미완료 버퍼 회수와 오래된 완료 통지 거부, 독립된 상대와의 패킷 교환 재개가 필요하다. [패킷 경로 설계](native-linux-net-packet-design.en-ko.md)
