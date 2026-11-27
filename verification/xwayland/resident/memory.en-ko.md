# Resident Xwayland memory budget / 상주 Xwayland 메모리 예산

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. [Native server #471](https://github.com/Romanof123/JNOX/issues/471) · [Rootful Mosaic #448](https://github.com/Romanof123/JNOX/issues/448)

## Linked startup allocations

The original static AArch64 Xwayland 24.1.6 ELF (SHA-256 `033344648cb13a4bc011c3c057054c0164426afd6ee171cf9ba4a32418dad1f3`) had its heap at `[0x5cbff0,0x64bff0)` in the writable load segment at `0x58d000`. Inspection of its linked Picolibc `__malloc_sbrk_aligned` and `sbrk` instructions confirms that the ordinary allocator grows within these symbols. Growth past the upper boundary sets errno to 12 and returns failure. The original arena provided 524,288 bytes.

| Allocation in the original ELF | Source of the count and element size | Minimum payload |
| --- | --- | ---: |
| DIX input event batch | `GetMaximumEventsNum()` returns 100. `InitEventList(n)` calls `calloc(n, 3088)`. | 308,800 B |
| Machine-independent input queue | `mieqInit()` requests 512 slots. `mieqGrowQueue()` allocates 24 bytes per slot, then one 3,088-byte event for each. | 1,593,344 B |
| XTEST input batch | When enabled, the linked `XTestExtensionInit()` reserves another 100 events. | 308,800 B |

The first two allocations alone coexist and need 1,902,144 bytes, before allocation metadata or other server state. XKB, fonts, motion history, scratch pixmaps and transient event batches use the same private arena. A cursor warp can allocate another 100-event list. These figures describe requested payloads, not measured peak usage.

The JNOX executable target now requests a 2 MiB arena. Its input queue starts with 128 slots (398,336 payload bytes), enough to enqueue a 100-event DIX batch into an empty queue. The ring still grows when full. If allocation fails during growth, cleanup releases only newly allocated slots and the old queue remains valid. The graphics SDK defaults to 512 KiB for other applications. The linker enforces the existing guest image window from `0x00400000` to `0x00800000`. The production build verifies its two heap symbols before recording the executable digest. [Native graphics linker script](../../../sdk/native-aarch64/graphics/app.ld) · [Input queue patch](../server-memory.patch) · [Server link check](../server_receipt.py)

## Pixel storage

With `-geometry 320x240 -shm` and a depth-24 root visual, `xwl_shm_create_screen_resources()` selects a backing pixmap. Its 32-bit pixels occupy 1,280 bytes per row and 307,200 bytes in total. The selected upstream configuration uses `memfd_create`, `F_SEAL_SHRINK`, `ftruncate` and a whole-file `MAP_SHARED` mapping. The heap holds the pixmap descriptor. Xwayland passes a reference to the same backing to the compositor through `wl_shm`. A replacement pixmap for a frame can allocate another backing until the compositor releases a reusable buffer.

| Native resource | Limit and lifetime |
| --- | --- |
| Anonymous shared objects | Four kernel-wide. Each holds up to 75 pages of 4,096 bytes. An open descriptor, queued FD transfer or mapping keeps the object live. |
| Shared maps and descriptors | Four mappings and eight shared FDs per process. Server and compositor can map the same object with different access rights. |
| Other simultaneous consumers | Tiles has one 128×96×4-byte backing. Root, a swap buffer, Tiles and an active cursor can together occupy four objects. Another concurrent keymap or replacement then depends on releasing an object. |
| Named temporary files | The broker's temporary namespace separately holds four regular files of up to 1,024 bytes each and eight handles per client. The renderer's successful memfd path stores its pixels in anonymous objects. |

Record the resident gate's actual server startup, rendered Mosaic pixels and pointer-driven redraw for the packaged ELF. The ELF calculation establishes the old allocation failure and the selected bounds. Peak heap residency and shared-object concurrency during sustained input still need guest observation. [Guest acceptance](README.en-ko.md) · [Shared mapping and backing](../../../kernel/src/native_shared_mmu.rs) · [Descriptor lifetime](../../../kernel/src/native_shared_fd.rs)

## 한국어

기존 실행 파일은 Picolibc 힙으로 512 KiB를 예약했다. 링크된 코드에서 DIX 입력 작업 공간은 308,800바이트, 초기 입력 큐는 1,593,344바이트를 동시에 요구한다. 이 두 항목만으로 기존 힙 용량을 초과한다. XTEST가 활성화되면 308,800바이트가 더 필요하고, 커서 이동과 키보드·폰트 처리도 별도 공간을 사용한다.

JNOX 상주 Xwayland는 4 MiB 사용자 이미지 영역 안에 2 MiB 전용 힙을 지정한다. 초기 입력 큐는 128칸이며 필요할 때 확장한다. 확장 도중 할당이 실패하면 새로 소유한 메모리만 반환한다. 다른 그래픽 앱의 기본 힙은 512 KiB다.

320×240 화면 한 장은 307,200바이트이며 커널의 익명 객체 한 개에 정확히 들어간다. Xwayland와 합성기는 같은 객체를 매핑한다. 시스템 전체 객체 한도는 4개, 프로세스별 매핑 한도는 4개다. Tiles·루트·교체 버퍼·커서가 동시에 살아 있으면 네 자리가 모두 사용된다. 실제 최고 사용량과 연속 입력 중 객체 반환 시점은 게스트 실행으로 확인해야 한다.
