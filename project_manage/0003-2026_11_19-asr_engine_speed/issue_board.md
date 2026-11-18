# 받아쓰기 엔진 interface 를 세우고 기기별 속도를 잰다
(이 파일: 기준 브랜치 main 의 `project_manage/0003-2026_11_19-asr_engine_speed/issue_board.md`, 같은 폴더에 `issue_decision.md`, `issue_log.md`)
- 이슈: https://github.com/guny524/seamone/issues/3, 유형: impl(범위가 정해진 구현)
- 운영 상태(상태, 브랜치, worktree, worker, 세션, 선행과 후속): 프로젝트 `project_manage/project_board.md` 4절(이슈) #3

## 1. 사용자 지시 원문
- 2026-11-19 "그리고 이제 아까 최종적으로 보고 있는 모델 크기는 CPU에서 돌아가는 모델 크기였으면 좋겠어. 한 3빌리언 정도면 그래도 CPU에서 빨리 돌지 않을까?"
- 2026-11-19 "폰에서 폰 AP 가속기 써가지고 못해? 최대한 일단 할 수 있는 거에서 모델 크기가 컸으면 좋겠어. 아니면은 폰과 노트북 모델은 따로 가는 걸로 하자. 그 노트북이나 그런 데서도 맥 같은 거는 GPU 그 AP 가속기를 쓸 수가 있잖아. M1 이런 것들은 그런 거 좀 감안을 하고 우리 CPU에서 계산할 때도 심드 같은 걸로 최대한 가속해서 계산할 수 있게끔 하고"
- 2026-11-19 "그리고 최대한 고나 로스트 같은 걸로 그냥 빌드가 됐으면 좋겠고,"

## 2. 배경
### 2-1. 배경
- 기기마다 쓸 수 있는 가속기가 다르다(Apple GPU, 폰 GPU 와 NPU, 가속기 없는 CPU), 엔진을 기기마다 갈아 끼우려면 받아쓰기 interface 가 먼저 있어야 한다
- 시작 모델은 "쓸 수 있는 속도 안에서 가장 큰 모델" 로 고른다(프로젝트 PD7), 공개된 속도 수치는 기기와 엔진이 제각각이라 앱에 실제로 들어갈 엔진으로 직접 잰다
### 2-2. 목표
- `Transcriber` interface 뒤에서 엔진 둘 이상이 같은 음성을 받아쓰고, 후보 모델마다 이 노트북의 CPU 와 가속기 속도가 나온다, #2 의 오류율과 합쳐 계층마다 시작 모델을 추천한다
### 2-3. 구현 방향
(이미 정해진 것, 출처 프로젝트 `project_manage/project_decision.md`)
- PD5: Rust, PD7: 가속기를 쓰고 CPU 는 SIMD 로 가속한다, 속도 기준은 데스크톱 계층이 가속기로 실시간의 3배 이상과 CPU 만으로 실시간 이상, 폰 계층이 가속기로 실시간 이상
- interface 의 계약: 입력은 16kHz mono 음성 표본과 언어 힌트(없을 수 있다), 출력은 전사 조각의 목록(시작 시각, 끝 시각, 글, 화자 칸은 비워 둔다), 조각의 타입은 `seamone-core` 에 한 번만 정의한다
- 엔진은 cargo feature 로 켜고 끈다, 기본 feature 만으로 다섯 플랫폼의 빌드와 CI 가 그대로 통과해야 한다(C++ 를 빌드하는 엔진은 기본으로 켜지 않는다)
- 엔진 후보: whisper.cpp 를 감싼 crate(CPU SIMD 와 Apple GPU), 순수 Rust 엔진(Candle 의 Whisper, `qwen-asr` crate), 지금 판과 유지 상태는 조회해 확인한다
### 2-4. 해치지 말아야 할 것, 기존 구현 의도
- #1 이 만든 workspace, Makefile, CI 를 깨지 않는다
- 모델 파일은 레포에 넣지 않는다, `/Users/min-jo/Documents/Developments/seamone/.tmp/` 아래에 받고 측정이 끝나면 지운다(PD12)

## 3. 목표와 범위
### 3-1. 요구
- R1. `seamone-asr` crate 에 `Transcriber` interface 와 엔진 구현 둘 이상이 있다, 출처 2-3
- R2. 속도 측정 CLI 가 엔진, 모델, 장치(CPU 만, 가속기), 스레드 수를 받아 음성 길이, 처리 시간, 실시간 배수(음성 길이 나누기 처리 시간), 메모리 최고치를 낸다, 출처 PD7
- R3. 후보 모델(Whisper base, small, large-v3-turbo, large-v3, Qwen3-ASR 0.6B 와 1.7B 가운데 엔진이 받는 것)을 같은 한국어 음성 10개로 재고, 첫 실행은 빼고 세 번 잰 가운데값을 쓴다, 출처 PD7
- R4. interface 와 실시간 배수 계산에 단위 시험이 있다(엔진 없이 가짜 엔진으로), 출처 rule `change.md` 6절
- R5. #2 의 오류율과 합쳐 계층마다 시작 모델을 추천한다, 폰 계층은 실측이 없으므로 어림임을 밝힌다, 출처 PD9
### 3-2. 범위
- `crates/seamone-asr`, `crates/seamone-core` 의 조각 타입, 속도 측정 CLI, 결과 문서(자리는 skill `project-structure` 로 정한다), `README.md` 2-2 의 받아쓰기 엔진 줄
### 3-3. 제외 범위
- 화면과 녹음, 이유: #5
- 학습, 이유: 뒤 이슈
- 폰 실측, 이유: 기기와 빌드 환경이 없다
### 3-4. 완료 기준
- [ ] `make lint`, `make test` 통과, 기본 feature 로 CI 의 다섯 플랫폼 빌드가 그대로 통과
- [ ] 엔진 둘이 같은 한국어 음성 하나를 받아써 글이 나온다
- [ ] 후보 모델마다 CPU 만 쓸 때와 Apple GPU 를 쓸 때의 실시간 배수와 메모리가 결과 문서에 있다, 재지 못한 조합은 까닭이 있다
- [ ] 계층마다 시작 모델 추천이 PD7 의 기준과 #2 의 오류율에 근거해 적혀 있다
### 3-5. 검증 방법
- `make lint`, `make test`, 측정은 make target 하나(`make bench-asr` 같은 이름)로 다시 돌릴 수 있게 한다
### 3-6. 의존성과 관련 링크
- 선행: #1 앱 뼈대, #2 오류율 측정(결과와 한국어 음성 표본), 후속: #5 녹음과 받아쓰기와 수정 화면, 프로젝트 PD9
### 3-7. 확인 필요
- 사실: 이 노트북에 cmake 가 있다, Xcode 는 명령줄 도구만 있다
- 가정: 명령줄 도구만으로 Apple GPU(Metal)를 쓰는 엔진이 빌드된다, 틀리면 CPU 측정만 하고 가속기 측정은 "재지 못함" 과 까닭을 적는다
- 질문: 없음
### 3-8. 우선 참조할 파일
- `/Users/min-jo/Documents/Developments/seamone/project_manage/project_decision.md` PD5, PD7, PD9, PD12
- `research/results/2026_11_19_start_model_cer.md`(#2 가 merge 된 뒤)
- skill `contract-spec`(interface 와 DTO), `project-structure`, `design-test`

---
(아래는 AI 가 쓰는 절)

## 4. 조사
### 4-1. 기존 코드와 구현의 핵심, 의도
- TODO
### 4-2. 바뀌어야 할 곳
- TODO

## 5. 방안과 판정
### 5-9. 최종 방안
- TODO(엔진 crate 마다 지금 판, 유지 상태, 가속기 지원, 다섯 플랫폼 빌드 가능 여부를 견줘 적는다)

## 6. 사용자 승인
- 해당 없음: 방향은 PD5, PD7 로 정해졌다, 시작 모델을 고르는 일은 프로젝트 PD9 에서 한다

## 7. 할 일과 검증
(순서대로 한다, 시간 한도: 모델 하나의 측정이 15분을 넘으면 음성을 5개로 줄이고 줄인 수를 적는다)
- [ ] 7-1. 조각 타입과 interface: skill `contract-spec` 을 읽고 `crates/seamone-core` 에 전사 조각 타입을, `crates/seamone-asr` 에 `Transcriber` interface 와 시험용 가짜 엔진을 만든다 / 검증: 가짜 엔진으로 interface 계약의 단위 시험
- [ ] 7-2. 엔진 고르기: 2-3 의 엔진 후보를 crates.io 와 저장소에서 조회해 5-9 에 견줘 적고 둘 이상을 cargo feature 뒤에 구현한다(가속기를 쓰는 것 하나, 순수 Rust 하나) / 검증: 엔진마다 한국어 음성 하나를 받아써 글이 나온다
- [ ] 7-3. 측정 CLI 와 make target: 실시간 배수 계산은 순수 함수로 두고 단위 시험을 단다, 음성은 #2 의 표본에서 한국어 10개를 wav 로 꺼내 `.tmp/` 에 둔다 / 검증: `make test`, 가짜 엔진으로 CLI 가 끝까지 돈다
- [ ] 7-4. 측정: 모델 파일을 `.tmp/` 에 받아 작은 것부터 재고(CPU 만, Apple GPU), 한 모델이 끝나면 그 파일을 지운다 / 검증: 결과가 `.tmp/bench/` 에 남고 결과 문서로 옮겨진다
- [ ] 7-5. 결과 문서와 추천: 측정 조건(기기, 스레드 수, 양자화, 엔진 판), 모델마다 결과, 한계, #2 의 오류율과 합친 계층별 시작 모델 추천을 쓴다, 표를 쓰지 않고 목록으로 쓴다
- [ ] 7-6. `README.md` 2-2 의 받아쓰기 엔진 줄을 "있음" 으로 고치고 interface 와 조각 타입을 한 줄씩 적는다, 8절 수정 요약과 보고를 쓴다

## 8. 수정 요약
- TODO

## 9. 지금 상태
- 메인이 직접 확인한 것: 없음
- worker 주장(아직 확인 전): 없음
- 남은 것: 7절 전부, #1 이 merge 된 뒤 시작한다
- 판단거리: 없음

## 10. Closing
### 10-1. 검토
- TODO
### 10-2. 커밋 직전 증거 대조
- TODO
### 10-3. merge 직전 재확인
- TODO
### 10-4. 사람 시험
- 해당 없음: 측정과 시험은 명령으로 확인한다
### 10-5. 자원
- TODO
### 10-6. 이슈 마무리 코멘트
- 상태: TODO
### 10-7. 끝내기 승인
- TODO
### 10-8. 커밋과 merge
- TODO

## 11. 참고
- whisper.cpp https://github.com/ggml-org/whisper.cpp, CPU SIMD 와 Apple GPU 를 쓰는 Whisper 엔진
- Candle https://github.com/huggingface/candle, Rust 로 짠 ML 엔진과 Whisper 구현
- `qwen-asr` crate https://lib.rs/crates/qwen-asr, 순수 Rust 로 짠 Qwen3-ASR CPU 엔진
