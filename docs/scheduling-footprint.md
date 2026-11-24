# 스케줄링 릴리스 ELF 크기 측정 (#66, #21)

최소 구성과 확장 구성의 실제 링크 결과를 비교한다. 의도한 기능 외에는 타깃, 도구 체인, 프로필, 링커 스크립트와 빌드 설정을 맞춘다. JSON과 함께 빌드 명령, 리비전, 활성 기능을 기록한다. 이름과 해시만으로 빌드 설정을 검증할 수 없다.

```sh
python tools/scheduling_footprint.py --baseline out/minimal.elf --feature extended=out/extended.elf > footprint.json
```

추가 구성마다 --feature NAME=PATH를 반복한다. 성공하면 기준값, 구성별 측정값, 기준값을 뺀 부호 있는 바이트 차이를 단일 JSON 객체로 출력한다. 잘못된 입력은 표준 오류에 사유를 쓰고 코드 2로 종료한다. 일반 프로그램 헤더 표와 이름이 있는 섹션 표를 갖춘 ELF64 리틀엔디언 AArch64 또는 x86-64 실행 파일을 같은 아키텍처끼리 비교한다.

| 바이트 필드 | 산출 기준 |
|---|---|
| file | 헤더, 메타데이터, 끝에 붙은 바이트를 포함한 실제 파일 길이. SHA-256도 파일 전체를 대상으로 한다. |
| load_file | 각 PT_LOAD의 파일 오프셋부터 p_filesz만큼의 구간을 합친 길이. |
| load_memory | 각 PT_LOAD의 가상 주소부터 p_memsz만큼의 구간을 합친 길이. |
| load_backed | 각 PT_LOAD의 가상 주소부터 p_filesz만큼의 구간을 합친 길이. |
| zero_fill | load_memory에서 load_backed를 뺀 값. 겹치는 주소는 파일에 대응하는 매핑을 우선한다. |
| alloc | 비어 있지 않은 SHF_ALLOC 섹션의 가상 주소 구간을 합친 길이. 각 구간은 로드 메모리 범위 안에 있어야 한다. |
| padding | load_memory에서 alloc을 뺀 값. 매핑된 헤더와 섹션 사이 빈 공간도 포함한다. |
| bss | .bss 또는 .bss.* 이름의 할당된 SHT_NOBITS 섹션 구간을 합친 길이. |
| linked_stack | NOLOAD 예약 공간을 포함한 .stack 또는 .stack.* 이름의 할당 섹션 구간을 합친 길이. |

모든 구간은 시작 주소를 포함하고 끝 주소를 제외한다. 지표 안에서는 중복 바이트를 한 번만 센다. 주소 공간이 다르거나 범주가 겹치는 지표끼리 합산하면 안 된다. 길이 0인 구간은 기여하지 않는다. 이름 기반 집계는 링커 명명 규칙에 의존한다. linked_stack은 링크 시 예약량이다. 실행 중 최대 스택 사용량, 힙 사용량, 물리 배치, 스케줄링 비용과 실제 로더 동작은 별도 근거가 필요하다.
