# First independent catalog review / 증거 목록 최초 독립 검토

REJECT public #206 577b49fad60aff8bbf95d6da7811730de021df11.
Catalog before: exit 1, four findings. Catalog after: exit 1, one new provenance discrepancy. Exact Korean prose attaches grammatical particles directly to both identifiers; the unchanged checker extracts zero Korean references vs two English references. A disposable Markdown delimiter-only control for both names gives exit 0, zero findings.
Both independent invocations of the repository requirement graph exited 0. Thirteen typed edges checked independently. One real removed edge and four real file removals each yielded the specific expected findings. Archive CRC and six published digest pins checked; five source revisions and their relevant code blobs resolved. Raw ELF extents rederived. Numeric and scope descriptions in Korean and English agree on the historical product limits, cohorts, original failures and controls. No guests, builds or releases ran.
The pre-outcome freeze mistakenly calls the second language Russian. Actual non-English language is Korean. Both actual languages for both records were checked. Replay with python audit.py /path/to/git/clone containing the five original commits and public #206.

## 독립 검토 결과 한국어판

최초 후보를 거절했다. 본선에서 발견한 네 가지 오류를 재현했다. 후보에서는 오류 한 건이 남았다. 한국어 문서에서 `IMP-058`과 `CHG-076` 바로 뒤에 조사가 붙어 기존 검사기가 두 식별자를 읽지 못했다. 영어 문서에서는 두 식별자를 모두 읽었다. 일회용 복사본에서 두 식별자를 백틱으로 감싸자 오류가 0건이 됐다.

요구사항 그래프는 두 버전 모두 통과했다. 독립 검증자는 유형이 명시된 연결 13개를 확인했다. 연결 하나를 삭제하는 대조 시험과 한영 파일 네 개를 각각 삭제하는 대조 시험에서 예상 오류가 모두 검출됐다. 보관 파일의 CRC, 공개 해시 6개, 소스 버전 5개와 해당 코드 객체를 확인하고 ELF 적재 범위를 다시 계산했다. 두 언어가 설명하는 과거 시험 결과, 제품 적용 범위 및 남은 한계는 일치했다. 이 검토에서 게스트 실행, 빌드 및 제품 출시는 0건이다.

사전 기준 파일은 두 번째 언어를 러시아어로 잘못 적었다. 실제 검토 대상은 두 기록의 영어판과 한국어판이었다. 원본 기준과 독립 결과는 아래 ZIP에 보존했다.

[Frozen cases, first findings, source hashes and controls / 사전 기준과 최초 결함 및 대조 자료](scheduling-catalog-repair-first.zip) · SHA-256 310110d9c05b827febbb6f32809daa908672eb5c6325afe3901fa75b6bb4103d.
