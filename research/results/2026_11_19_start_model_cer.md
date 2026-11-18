# 시작 모델 후보의 한국어와 영어 글자 오류율 측정
- 이슈: 시작 모델 후보의 한국어 오류율을 같은 잣대로 잰다 (https://github.com/guny524/seamone/issues/2)
- 측정일: 2026-11-19, 코드: `research/`, 이슈 기록: `project_manage/0002-2026_11_19-start_model_cer/`
- 상태: 2026-11-19 07:10 기준(받아쓰기 측정은 07:05 에, LoRA 시험은 07:09 에 끝냈다, 못 한 것은 4절과 5절)
- 이 문서가 답하는 것: 후보 여섯 모델의 한국어와 영어 글자 오류율(CER, character error rate), 후보를 PyTorch 에서 LoRA(작은 덧붙임 가중치)로 학습할 수 있는지와 그 비용

## 1. 결론과 추천
- 한국어 글자 오류율(CER)은 Qwen3-ASR-1.7B 가 가장 낮고, Whisper large-v3-turbo 가 그다음이고, Whisper small 은 그보다 높고, Whisper base 는 훨씬 높다, Qwen3-ASR-0.6B 는 large-v3-turbo 와 구별되지 않는다
  - 네 모델이 모두 푼 같은 한국어 발화 64개(FLEURS 56개와 Zeroth-Korean 8개)의 두 세트 합산 CER: Qwen3-ASR-1.7B 4.19%(±1.08%p), Whisper large-v3-turbo 5.44%(±1.20%p), Whisper small 7.47%(±1.21%p), Whisper base 13.89%(±1.67%p), ±는 표준오차다
  - 같은 발화에서 모델 두 개의 CER 차이(앞 모델에서 뒤 모델을 뺀 값, 음수면 앞 모델의 오류가 적다): Qwen3-ASR-1.7B 와 large-v3-turbo 는 -1.25%p(±0.46%p, 표준오차의 약 2.7배), Qwen3-ASR-1.7B 와 small 은 -3.28%p(±0.69%p, 약 4.7배), large-v3-turbo 와 small 은 -2.30%p(±0.78%p, 약 3.0배), small 과 base 는 -5.71%p(±0.34%p, 같은 발화 400개)
  - Qwen3-ASR-0.6B 는 잰 발화가 한국어 27개뿐이라 따로 견준다: large-v3-turbo 와 구별되지 않고(+0.08%p, ±0.66%p), Qwen3-ASR-1.7B 보다 높고(Qwen3-ASR-1.7B 에서 뺀 값 -1.23%p, ±0.37%p, 약 3.3배), small 보다는 낮은 쪽으로 약하게 기울지만 표준오차의 약 1.8배라 확정하지 못한다(-1.77%p, ±0.98%p)
- 영어는 같은 FLEURS 영어 40개에서 Qwen3-ASR-1.7B 가 1.78%(±0.48%p), large-v3-turbo 가 2.42%(±0.52%p), small 이 3.68%(±0.63%p), base 가 5.30%(±0.87%p) 이고, Qwen3-ASR-1.7B 와 large-v3-turbo 의 차이는 -0.64%p(±0.23%p, 약 2.8배)다
  - 여러 언어를 지원한다는 조건은 이 이슈가 한국어와 영어만 재서 다른 언어는 확인하지 못했다, 지원 언어는 모델 카드 기준으로 Qwen3-ASR 이 52개 언어와 방언(이슈 board 2-3절이 인용한 모델 카드), Whisper 는 99개가 넘는다
- 이슈 board 2-2절의 가설과 맞춰 본 결과: "Qwen3-ASR-1.7B 가 가장 낮다" 는 맞는 쪽으로 기운다(한국어와 영어 모두 large-v3-turbo 와의 차이가 표준오차의 약 2.7배와 2.8배라 확정은 아니다), "Whisper large-v3-turbo 가 그다음" 은 맞다(Qwen3-ASR-0.6B 와는 구별되지 않는다), "Whisper small 과 base 는 크게 높다" 는 small 이 large-v3-turbo 보다 2.3%p 높고 base 가 small 보다 5.7%p 높아 맞다
- 앞쪽 몇 개 발화로 잰 값은 낙관적이었다: FLEURS 한국어 CER 이 Qwen3-ASR-1.7B 는 앞 12개에서 2.34%, 앞 56개에서 4.53% 이고 large-v3-turbo 는 앞 20개에서 4.67%, 앞 56개에서 5.83% 다, 그래서 잰 수가 다른 값을 바로 견주지 않는다(2-5절)
- 표본은 아직 작다: Zeroth-Korean 은 Qwen3-ASR-1.7B 가 8개, large-v3-turbo 가 16개뿐이고, Qwen3-ASR-0.6B 는 한국어 27개와 영어 8개, whisper-large-v3 는 재지 못했다(2-5절과 5절에 까닭)
- 속도와 크기는 이 이슈가 정하지 않는다: 3-5절의 시간은 노트북이 다른 빌드와 겹친 상태의 참고값이고, 속도는 이슈 "받아쓰기 엔진 interface 를 세우고 기기별 속도를 잰다" 가 앱에 들어갈 Rust 엔진으로 잰다
- 추천(6절): 데스크톱 계층은 Qwen3-ASR-1.7B 를 첫 후보로, Whisper large-v3-turbo 를 속도 기준을 못 넘을 때의 대안으로 두고, 폰 계층은 Qwen3-ASR-0.6B 와 Whisper small 을 속도와 메모리를 잰 뒤에 가른다, 시작 모델은 이 문서가 정하지 않는다(프로젝트 결정 "시작 모델" 은 속도 측정 뒤 메인이 정한다)
- LoRA 역전파 한 번(4절): 글 디코더만 붙이는 범위로 Apple GPU 에서 whisper-small 과 whisper-large-v3-turbo 는 묶음 4, Qwen3-ASR-1.7B 는 묶음 1 이 돈다(두 번째 걸음 2.4초, 9.9초, 4.4초, GPU 메모리 2379MiB, 3147MiB, 6460MiB), Qwen3-ASR-1.7B 의 묶음 4 는 GPU 메모리 한도를 넘어 실패했고, 음성 인코더까지 붙이는 범위 all 은 whisper-small 묶음 4 가 32분 넘게 끝나지 않았다(묶음 1 은 3.6초), CPU 는 whisper-small 만 쟀다

## 2. 조건
### 2-1. 후보 여섯 모델
- 모델, 내려받은 version(Hugging Face 커밋 hash), 가중치 파일 크기, 모델 카드의 license 태그(2026-11-19 조회)
  - `openai/whisper-base`: e37978b90ca9030d5170a5c07aadb050351a65bb, 290MB, apache-2.0
  - `openai/whisper-small`: 973afd24965f72e36ca33b3055d56a652f456b4d, 967MB, apache-2.0
  - `openai/whisper-large-v3-turbo`: 41f01f3fe87f28c78e2fbf8b568835947dd65ed9, 1.62GB, mit
  - `openai/whisper-large-v3`: 06f233fe06e710322aca913c1bc4249a0d71fce1, 3.09GB, apache-2.0
  - `Qwen/Qwen3-ASR-0.6B`: 5eb144179a02acc5e5ba31e748d22b0cf3e303b0, 1.88GB, apache-2.0
  - `Qwen/Qwen3-ASR-1.7B`: 7278e1e70fe206f11671096ffdd38061171dd6e5, 4.70GB, apache-2.0
- 남긴 가중치 파일의 헤더에서 읽은 저장 dtype 과 매개변수 수: whisper-small 은 fp32 에 2억 4170만 개, whisper-large-v3-turbo 는 fp16 에 8억 890만 개, Qwen3-ASR-1.7B 는 bf16 에 23억 4920만 개(두 파일의 합, 이름의 1.7B 보다 크다)
  - 지운 whisper-base 는 파일 크기 290MB 로 보아 fp32 로 어림하고(확인하지 못했다), Qwen3-ASR-0.6B 도 지운 뒤라 헤더를 읽지 못했다
  - 풀어 쓸 때는 모든 모델을 fp16 으로 올렸다(Qwen3-ASR 은 bf16 에서 fp16 으로 바꿔 올렸고 출력 글이 정상이었다)
- 이슈 board 2-3절은 "Whisper 는 MIT" 라고 적었다, 모델 카드의 license 태그는 whisper-large-v3-turbo 만 mit 이고 나머지 Whisper 셋은 apache-2.0 이다, 둘 다 누구나 쓰고 고칠 수 있는 license 라 모델 고르기에는 영향이 없다

### 2-2. 시험 세트 셋
- 세 세트 모두 사람이 문장을 읽은 낭독 음성이고, 회의처럼 여럿이 말하거나 소음이 낀 음성이 아니다(5절 한계)
- FLEURS 한국어(`google/fleurs` 의 `ko_kr` test, CC BY 4.0, version 70bb2e84b976b7e960aa89f1c648e09c59f894dd)
  - 후보 270문장(test 녹음 382개에서 같은 문장을 여러 사람이 읽은 녹음을 문장마다 하나로 줄인 수), 그중 200개를 뽑았다
  - 음성 합계 2569.0초, 평균 12.8초, 가장 긴 녹음 25.8초
  - 정답 200개 가운데 아라비아 숫자가 든 정답 46개, 영문자가 든 정답 38개
- FLEURS 영어(`en_us` test, 같은 version)
  - 후보 350문장(녹음 647개), 그중 100개를 뽑았다, 음성 합계 993.7초, 평균 9.9초, 가장 긴 녹음 29.3초
  - 정답 100개 가운데 숫자가 든 정답 21개
- Zeroth-Korean(`kresnik/zeroth_korean` test, CC BY 4.0, version 1fe937899f828af822293d05e086200946088bdf)
  - 후보 457개(test 전부), 그중 200개를 뽑았다, 음성 합계 1934.7초, 평균 9.7초, 가장 긴 녹음 20.5초
  - 정답 200개 모두에 아라비아 숫자와 영문자가 없다(숫자를 한글로 풀어 쓴 꼴, 예: "이천 구 년")
- 뽑는 방법: 후보에서 30초를 넘는 녹음(Whisper 의 입력 창이 30초라서)과 정규화한 정답이 빈 녹음을 빼고, 같은 문장을 여러 사람이 읽은 FLEURS 녹음은 file_name 이 가장 앞선 하나만 두고, seed 20261119 와 녹음 id 를 이은 글의 SHA-256 값 순서로 줄 세워 앞에서 필요한 수만큼 뽑는다
  - 이 순서가 `research/data/eval_ids.json` 의 순서다, 모든 모델이 이 순서의 앞에서부터 같은 발화를 잰다
  - 30초를 넘어 빠진 녹음은 FLEURS 와 Zeroth 에 없었다

### 2-3. 같은 잣대
- 정답 글: FLEURS 는 `raw_transcription`(문장부호와 숫자가 있는 원문), Zeroth-Korean 은 `text`
- 글 정규화(정답과 모델이 쓴 글 모두에 같게 적용): 유니코드 NFC 로 합치고, 소문자로 바꾸고, 문장부호(유니코드 범주 P*)와 공백(Z*)과 제어와 서식 문자(Cc, Cf)를 지운다, 숫자와 기호(통화 기호 `$` 와 수학 기호 `+` 같은 S*)는 그대로 둔다
  - 숫자를 글자로 바꾸지 않는다: 정답이 "오" 이고 모델이 "5" 로 쓰면 오류로 센다(5절 한계)
- CER: 세트 전체의 편집 거리(대체, 삭제, 삽입의 합)를 정답 글자 수의 합으로 나눈다, 발화마다 CER 의 평균이 아니다
- 표준오차(CER 추정값이 표본에 따라 흔들리는 폭): 발화 하나를 한 덩어리로 보는 비율 추정(ratio estimator)으로 계산한 값이다, 이 문서의 "±" 는 표준오차 한 개 폭이다
- 풀어 쓰기: 모든 모델이 같은 16kHz 단일 채널 음성 배열을 받고, 세트의 언어를 지정하고(Whisper 는 `language`, Qwen3-ASR 은 "Korean" 이나 "English"), 글 맥락 없이, greedy(빔 1, 표집 없음)로 푼다
  - 풀어 쓰기 최대 길이: Whisper 440 토큰, Qwen3-ASR 512 토큰
- 채점 확인: jiwer 와 따로 쓴 편집 거리 계산(동적 계획법 한 함수)으로 네 곳을 다시 계산해 같은 값을 얻었다(whisper-small Zeroth 10.99%, whisper-small FLEURS 한국어 8.10%, whisper-base FLEURS 영어 5.23%, whisper-large-v3-turbo FLEURS 한국어 앞 20개 4.67%), 이 확인은 한 번 돌린 것이라 `make` 목표로 만들지 않았다
- 채점은 모델을 돌리는 단계와 따로 한다: `research/seamone_research/measure.py` 가 가설 글을 `.tmp/research/hyp/<모델>/<세트>.jsonl` 에 쌓고, `score.py` 가 그 파일에서 CER 을 다시 계산한다

### 2-4. 실행 환경
- 노트북: MacBook Air(M1, 메모리 16GB, 모델 식별자 MacBookAir10,1), macOS 26.5.2
- 소프트웨어(`research/uv.lock` 의 version): Python 3.12.6, torch 2.15.0, transformers 4.57.6, qwen-asr 0.0.6(transformers 4.57.6 과 accelerate 1.12.0 을 고정한다), peft 0.21.3, jiwer 4.0.0, huggingface-hub 0.36.2
- 모든 모델을 Apple GPU(MPS)에서 fp16 으로 풀어 썼다, bf16 은 쓰지 않았다
- 묶음 크기: whisper-base 와 whisper-small 은 16, 그 밖은 4(Qwen3-ASR-0.6B 의 한국어 앞 19개만 5절에 적은 대로 다르다)(메인이 노트북의 메모리 부족을 이유로 묶음 크기를 1에서 4 로 정하라고 지시했다, 05:30)
- 측정 도중 이 노트북에서 다른 worker 가 Rust 를 빌드했고 load average 가 25에서 144 사이였다, swap 은 5GB 에서 15GB 까지 찼다, 그래서 3-5절의 시간은 참고값이다(속도는 이슈 "받아쓰기 엔진 interface 를 세우고 기기별 속도를 잰다" 가 앱에 들어갈 Rust 엔진으로 잰다)

### 2-5. 모델마다 잰 발화 수가 다른 까닭과 읽는 법
- 모델마다 잰 발화 수(세트마다 eval_ids.json 순서의 앞에서부터, 순서는 FLEURS 한국어, FLEURS 영어, Zeroth-Korean)
  - whisper-base 와 whisper-small: 200개, 100개, 200개(세 세트 전부)
  - whisper-large-v3-turbo: 56개, 40개, 16개
  - Qwen3-ASR-1.7B: 56개, 40개, 8개
  - Qwen3-ASR-0.6B: 19개, 8개, 8개(가중치를 지운 뒤라 더 재지 못했다)
  - whisper-large-v3: 하나도 재지 못했다(5절)
- 줄인 까닭: 측정하는 동안 이 노트북에서 다른 worker 가 Rust 를 빌드해 load average 가 25에서 144 였고 swap 이 5GB 에서 15GB 까지 찼다, 풀어 쓰기가 한 걸음마다 느려 묶음 하나가 길게는 5분 걸렸다(예: Qwen3-ASR-1.7B 의 묶음 4 하나가 24초에서 332초), 이슈 board 7절 머리의 "모델 하나의 측정이 25분을 넘을 것 같으면 표본을 줄이고 줄인 수를 적는다" 를 따랐고, 메인이 메모리와 디스크 부족을 이유로 묶음 크기를 4 이하로 줄이게 했고(05:30) 새 무거운 실행을 06:30 까지 쉬게 했다(05:58)
- 늘린 순서: 05:28 까지 whisper-large-v3-turbo 와 Qwen3-ASR 두 모델을 한국어 12개에서 20개, 영어 8개에서 12개, Zeroth 8개에서 16개 안팎으로 쟀고, 06:32 에 재개해 Qwen3-ASR-1.7B 와 whisper-large-v3-turbo 를 영어 40개로 늘린 뒤 한국어를 60개로 늘리려 했다(메인이 정한 값, 두 모델이 데스크톱 계층 후보라서)
  - 06:59 에 load average 가 50 을 넘고 디스크 여유가 16GiB 로 내려 Qwen3-ASR-1.7B 한국어 측정을 56개에서 멈췄다(마지막 묶음 4개에 332초), 그래서 whisper-large-v3-turbo 한국어를 같은 56개까지만 늘렸고(07:05) Zeroth-Korean 은 늘리지 못했다
- 중단한 실행 한 번: Qwen3-ASR-1.7B 를 묶음 32 로 05:13 에 시작했는데 10분이 지나도 첫 묶음이 끝나지 않고 메모리를 약 10GB 차지해 05:28 에 멈췄다, 이 실행이 쌓은 가설 글은 없다, 이 실행이 메인이 알린 메모리 부족과 시간이 겹친다
- 읽는 법: 잰 발화 수가 다른 CER 을 바로 견주지 않는다, 앞쪽 발화가 쉬운 구간일 수 있어서다(예: Zeroth-Korean 앞 16개에서 whisper-base 는 10.85%, 전체 200개에서는 16.86%), 견줄 때는 모든 모델이 같이 가진 앞부분만 쓰고(3-2절과 3-3절) 표본이 작아 흔들리는 폭은 ±표준오차로 읽는다

## 3. 결과
### 3-1. 모델마다 잰 전부
- 모델마다 잰 발화 수(n)가 다르다, 이 값끼리 모델을 직접 견주지 않는다(2-5절)
- 다시 계산하는 명령은 `make -C research score MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-0.6b qwen3-asr-1.7b"` 이다(이 명령이 같이 내는 "공통 앞부분" 값은 MODELS 에 든 모델에 따라 달라진다)
- whisper-base
  - FLEURS 한국어: n=200, CER 13.66%(±0.88%p), 대체 9.22%, 삭제 3.79%, 삽입 0.65%, 정답 글자 9553, 두 배 넘게 긴 가설 0개
  - FLEURS 영어: n=100, CER 5.23%(±0.55%p), 대체 2.17%, 삭제 1.76%, 삽입 1.29%, 정답 글자 11002, 두 배 넘게 긴 가설 0개
  - Zeroth-Korean: n=200, CER 16.86%(±1.00%p), 대체 13.42%, 삭제 2.61%, 삽입 0.82%, 정답 글자 8745, 두 배 넘게 긴 가설 0개
- whisper-small
  - FLEURS 한국어: n=200, CER 8.10%(±0.77%p), 대체 4.34%, 삭제 3.49%, 삽입 0.27%, 정답 글자 9553, 두 배 넘게 긴 가설 0개
  - FLEURS 영어: n=100, CER 2.99%(±0.38%p), 대체 0.87%, 삭제 1.07%, 삽입 1.05%, 정답 글자 11002, 두 배 넘게 긴 가설 0개
  - Zeroth-Korean: n=200, CER 10.99%(±0.95%p), 대체 8.48%, 삭제 1.75%, 삽입 0.75%, 정답 글자 8745, 두 배 넘게 긴 가설 0개
- whisper-large-v3-turbo
  - FLEURS 한국어: n=56, CER 5.83%(±1.34%p), 대체 2.75%, 삭제 2.97%, 삽입 0.11%, 정답 글자 2694, 두 배 넘게 긴 가설 0개
  - FLEURS 영어: n=40, CER 2.42%(±0.52%p), 대체 0.59%, 삭제 1.31%, 삽입 0.52%, 정답 글자 4210, 두 배 넘게 긴 가설 0개
  - Zeroth-Korean: n=16, CER 2.17%(±0.90%p), 대체 1.45%, 삭제 0.14%, 삽입 0.58%, 정답 글자 691, 두 배 넘게 긴 가설 0개
- qwen3-asr-0.6b
  - FLEURS 한국어: n=19, CER 4.77%(±1.85%p), 대체 2.22%, 삭제 2.44%, 삽입 0.11%, 정답 글자 944, 두 배 넘게 긴 가설 0개
  - FLEURS 영어: n=8, CER 3.15%(±1.65%p), 대체 1.18%, 삭제 1.84%, 삽입 0.13%, 정답 글자 761, 두 배 넘게 긴 가설 0개
  - Zeroth-Korean: n=8, CER 2.79%(±1.08%p), 대체 2.79%, 삭제 0.00%, 삽입 0.00%, 정답 글자 358, 두 배 넘게 긴 가설 0개
- qwen3-asr-1.7b
  - FLEURS 한국어: n=56, CER 4.53%(±1.22%p), 대체 1.45%, 삭제 2.86%, 삽입 0.22%, 정답 글자 2694, 두 배 넘게 긴 가설 0개
  - FLEURS 영어: n=40, CER 1.78%(±0.48%p), 대체 0.40%, 삭제 1.07%, 삽입 0.31%, 정답 글자 4210, 두 배 넘게 긴 가설 0개
  - Zeroth-Korean: n=8, CER 1.68%(±1.03%p), 대체 1.40%, 삭제 0.00%, 삽입 0.28%, 정답 글자 358, 두 배 넘게 긴 가설 0개

### 3-2. 같은 발화로 견준 값(모든 모델이 같이 가진 앞부분)
- 네 모델(whisper-base, whisper-small, whisper-large-v3-turbo, Qwen3-ASR-1.7B)이 모두 가진 앞부분의 CER 이다, 이 문서의 기본 비교다, 다시 계산하는 명령은 `make -C research score MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-1.7b"` 가 내는 "공통 앞부분" 값이다, ±는 표준오차다
- FLEURS 한국어, 공통 앞부분 56개
  - qwen3-asr-1.7b: CER 4.53%(±1.22%p)
  - whisper-large-v3-turbo: CER 5.83%(±1.34%p)
  - whisper-small: CER 7.65%(±1.35%p)
  - whisper-base: CER 14.25%(±1.85%p)
- FLEURS 영어, 공통 앞부분 40개
  - qwen3-asr-1.7b: CER 1.78%(±0.48%p)
  - whisper-large-v3-turbo: CER 2.42%(±0.52%p)
  - whisper-small: CER 3.68%(±0.63%p)
  - whisper-base: CER 5.30%(±0.87%p)
- Zeroth-Korean, 공통 앞부분 8개
  - qwen3-asr-1.7b: CER 1.68%(±1.03%p)
  - whisper-large-v3-turbo: CER 2.51%(±1.56%p)
  - whisper-small: CER 6.15%(±2.08%p)
  - whisper-base: CER 11.17%(±3.04%p)
- Qwen3-ASR-0.6B 를 더한 다섯 모델이 모두 가진 앞부분의 CER 이다(Qwen3-ASR-0.6B 는 잰 발화가 적어 앞부분이 짧다), 다시 계산하는 명령은 `make -C research score MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-0.6b qwen3-asr-1.7b"` 가 내는 "공통 앞부분" 값이다
- FLEURS 한국어, 공통 앞부분 19개
  - qwen3-asr-1.7b: CER 3.50%(±1.67%p)
  - whisper-large-v3-turbo: CER 4.77%(±2.00%p)
  - qwen3-asr-0.6b: CER 4.77%(±1.85%p)
  - whisper-small: CER 5.93%(±1.61%p)
  - whisper-base: CER 13.35%(±2.10%p)
- FLEURS 영어, 공통 앞부분 8개
  - qwen3-asr-1.7b: CER 2.37%(±1.12%p)
  - whisper-small: CER 3.15%(±1.41%p)
  - whisper-large-v3-turbo: CER 3.15%(±1.50%p)
  - qwen3-asr-0.6b: CER 3.15%(±1.65%p)
  - whisper-base: CER 5.39%(±1.82%p)
- Zeroth-Korean, 공통 앞부분 8개
  - qwen3-asr-1.7b: CER 1.68%(±1.03%p)
  - whisper-large-v3-turbo: CER 2.51%(±1.56%p)
  - qwen3-asr-0.6b: CER 2.79%(±1.08%p)
  - whisper-small: CER 6.15%(±2.08%p)
  - whisper-base: CER 11.17%(±3.04%p)

### 3-3. 세트를 합친 값과 같은 발화에서 모델 둘의 차이
- 네 모델의 한국어 두 세트 합산 CER(같은 발화 64개: FLEURS 한국어 56개, Zeroth-Korean 8개), 다시 계산하는 명령은 `make -C research compare MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-1.7b" SETS="fleurs_ko zeroth_ko"`
  - qwen3-asr-1.7b: 4.19%(±1.08%p)
  - whisper-large-v3-turbo: 5.44%(±1.20%p)
  - whisper-small: 7.47%(±1.21%p)
  - whisper-base: 13.89%(±1.67%p)
- 같은 발화에서 모델 두 개의 CER 차이(앞 모델에서 뒤 모델을 뺀 값, 두 모델이 같이 가진 앞부분만 쓴다), 다시 계산하는 명령은 `make -C research compare MODELS="a b" SETS="fleurs_ko zeroth_ko" PAIR="a b"`
  - qwen3-asr-1.7b 와 whisper-large-v3-turbo: -1.25%p(±0.46%p, 표준오차의 약 2.7배), 같은 발화 64개
  - qwen3-asr-1.7b 와 whisper-small: -3.28%p(±0.69%p, 표준오차의 약 4.7배), 같은 발화 64개
  - whisper-large-v3-turbo 와 whisper-small: -2.30%p(±0.78%p, 표준오차의 약 3.0배), 같은 발화 72개
  - whisper-small 와 whisper-base: -5.71%p(±0.34%p, 표준오차의 약 16.8배), 같은 발화 400개
- 차이가 표준오차의 약 2배 이상이면 한 방향으로 기운다고 읽는다(정규분포를 가정한 어림이다), 표본이 작으면 확정이 아니다
- Qwen3-ASR-0.6B 를 더한 다섯 모델의 한국어 두 세트 합산 CER(같은 발화 27개: FLEURS 한국어 19개, Zeroth-Korean 8개), 다시 계산하는 명령은 `make -C research compare MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-0.6b qwen3-asr-1.7b" SETS="fleurs_ko zeroth_ko"`
  - qwen3-asr-1.7b: 3.00%(±1.24%p)
  - whisper-large-v3-turbo: 4.15%(±1.50%p)
  - qwen3-asr-0.6b: 4.22%(±1.37%p)
  - whisper-small: 5.99%(±1.28%p)
  - whisper-base: 12.75%(±1.71%p)
- Qwen3-ASR-0.6B 와 견준 CER 차이(앞 모델에서 뒤 모델을 뺀 값, 두 모델이 같이 가진 앞부분만 쓴다), 다시 계산하는 명령은 `make -C research compare MODELS="a b" SETS="fleurs_ko zeroth_ko" PAIR="a b"`
  - qwen3-asr-1.7b 와 qwen3-asr-0.6b: -1.23%p(±0.37%p, 표준오차의 약 3.3배), 같은 발화 27개
  - qwen3-asr-0.6b 와 whisper-large-v3-turbo: +0.08%p(±0.66%p, 표준오차의 약 0.1배), 같은 발화 27개
  - qwen3-asr-0.6b 와 whisper-small: -1.77%p(±0.98%p, 표준오차의 약 1.8배), 같은 발화 27개
- 네 모델의 영어 CER(같은 발화 40개: FLEURS 영어 40개), 다시 계산하는 명령은 `make -C research compare MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-1.7b" SETS="fleurs_en"`
  - qwen3-asr-1.7b: 1.78%(±0.48%p)
  - whisper-large-v3-turbo: 2.42%(±0.52%p)
  - whisper-small: 3.68%(±0.63%p)
  - whisper-base: 5.30%(±0.87%p)
- 영어에서 모델 두 개의 CER 차이(앞 모델에서 뒤 모델을 뺀 값, 두 모델이 같이 가진 앞부분만 쓴다), 다시 계산하는 명령은 `make -C research compare MODELS="a b" SETS="fleurs_en" PAIR="a b"`
  - qwen3-asr-1.7b 와 whisper-large-v3-turbo: -0.64%p(±0.23%p, 표준오차의 약 2.8배), 같은 발화 40개
- 세 세트를 모두 합친 네 모델의 CER, 글자 수가 많은 세트가 비중을 키운다(같은 발화 104개: FLEURS 한국어 56개, FLEURS 영어 40개, Zeroth-Korean 8개), 다시 계산하는 명령은 `make -C research compare MODELS="whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-1.7b" SETS="fleurs_ko fleurs_en zeroth_ko"`
  - qwen3-asr-1.7b: 2.80%(±0.54%p)
  - whisper-large-v3-turbo: 3.69%(±0.61%p)
  - whisper-small: 5.27%(±0.65%p)
  - whisper-base: 8.91%(±0.98%p)

### 3-4. 눈에 띈 것
- 숫자 표기가 CER 을 올린다(보조 계산, 저장된 가설 글에서 센 값이고 `make` 목표로 만들지 않았다)
  - Zeroth-Korean 정답 200개에는 아라비아 숫자가 한 번도 없다(숫자를 한글로 풀어 썼다), 모델이 숫자로 쓰면 오류로 센다: whisper-small 가설 200개 가운데 61개, whisper-base 가설 200개 가운데 65개가 정답에 없는 숫자를 썼다
  - 정답과 가설 모두에 숫자가 없는 발화만 보면 whisper-small 은 8.07%(139개, 전체 10.99%), whisper-base 는 12.67%(135개, 전체 16.86%) 다
  - FLEURS 한국어는 정답 46개에 숫자가 있고, 숫자 없는 146개만 보면 whisper-small 7.92%(전체 8.10%), 숫자 없는 145개만 보면 whisper-base 12.28%(전체 13.66%) 다
- FLEURS 한국어 정답의 영문 이름과 괄호 속 표기는 모델이 한글로 쓰거나 빼서 오류가 된다: whisper-large-v3-turbo 가 정답 "ASUS Eee PC" 를 "아수스 EPC" 로, "TogiNet Radio" 를 "토지넷 라디오" 로 쓰고, 정답 "(75.6 cm x 62.2 cm)" 를 쓰지 않았다(큰 오류가 난 FLEURS 한국어 발화 3개)
- 가설이 정답보다 두 배 넘게 긴 발화는 다섯 모델 모두 0개라 같은 말을 되풀이하는 환각은 이 표본에서 없었다
- Qwen3-ASR-1.7B 는 정답에 없는 말을 거의 덧붙이지 않고 오류의 대부분이 삭제다(FLEURS 한국어 56개: 대체 1.45%, 삭제 2.86%, 삽입 0.22%), Qwen3-ASR-0.6B 는 대체 2.22%, 삭제 2.44%, 삽입 0.11%(FLEURS 한국어 19개)로 대체가 많다
- 다섯 모델 비교(3-2절)의 FLEURS 영어 앞 8개에서 whisper-small, whisper-large-v3-turbo, Qwen3-ASR-0.6B 의 CER 이 모두 3.15% 로 같은 것은 우연이다: 세 모델 모두 정답 761글자 가운데 24글자가 틀렸고, 발화별 오류 수는 모델마다 다르다(whisper-small 0, 9, 3, 1, 0, 3, 0, 8 개, whisper-large-v3-turbo 0, 9, 2, 1, 0, 3, 0, 9 개, Qwen3-ASR-0.6B 0, 8, 2, 0, 0, 3, 0, 11 개), 오류가 대서양 허리케인 문장과 인도 이름이 든 문장 두 발화에 몰려 있다

### 3-5. 걸린 시간과 메모리(참고값)
- 같은 모델이라도 세트마다 처음 한 묶음에 GPU 준비 시간이 들어 있고, 모델마다 묶음 크기와 노트북 부하가 달라 모델끼리 속도를 견주는 근거가 아니다(5절), 원본은 `.tmp/research/hyp/<모델>/timing.json`
- whisper-base
  - FLEURS 한국어 n=5: 음성 83.5초를 48.2초에(실시간의 1.7배), 묶음 16, 불러오기 48.3초, GPU 메모리 1121MiB, process RSS 최고 704MiB, mps float16
  - FLEURS 영어 n=5: 음성 39.3초를 1.8초에(실시간의 21.8배), 묶음 16, 불러오기 48.3초, GPU 메모리 1121MiB, process RSS 최고 704MiB, mps float16
  - Zeroth-Korean n=5: 음성 51.1초를 5.9초에(실시간의 8.7배), 묶음 16, 불러오기 48.3초, GPU 메모리 1121MiB, process RSS 최고 704MiB, mps float16
  - FLEURS 한국어 n=195: 음성 2485.4초를 198.3초에(실시간의 12.5배), 묶음 16, 불러오기 10.5초, GPU 메모리 1137MiB, process RSS 최고 474MiB, mps float16
  - FLEURS 영어 n=95: 음성 954.5초를 100.2초에(실시간의 9.5배), 묶음 16, 불러오기 10.5초, GPU 메모리 1137MiB, process RSS 최고 474MiB, mps float16
  - Zeroth-Korean n=195: 음성 1883.6초를 209.5초에(실시간의 9.0배), 묶음 16, 불러오기 10.5초, GPU 메모리 1137MiB, process RSS 최고 474MiB, mps float16
- whisper-small
  - FLEURS 한국어 n=200: 음성 2569.0초를 631.1초에(실시간의 4.1배), 묶음 16, 불러오기 18.3초, GPU 메모리 2313MiB, process RSS 최고 748MiB, mps float16
  - FLEURS 영어 n=100: 음성 993.7초를 227.3초에(실시간의 4.4배), 묶음 16, 불러오기 18.3초, GPU 메모리 2313MiB, process RSS 최고 748MiB, mps float16
  - Zeroth-Korean n=200: 음성 1934.7초를 385.9초에(실시간의 5.0배), 묶음 16, 불러오기 18.3초, GPU 메모리 2313MiB, process RSS 최고 748MiB, mps float16
- whisper-large-v3-turbo
  - FLEURS 한국어 n=20: 음성 287.5초를 105.7초에(실시간의 2.7배), 묶음 4, 불러오기 17.1초, GPU 메모리 2113MiB, process RSS 최고 281MiB, mps float16
  - FLEURS 영어 n=12: 음성 112.3초를 37.6초에(실시간의 3.0배), 묶음 4, 불러오기 17.1초, GPU 메모리 2113MiB, process RSS 최고 281MiB, mps float16
  - Zeroth-Korean n=16: 음성 149.0초를 56.8초에(실시간의 2.6배), 묶음 4, 불러오기 17.1초, GPU 메모리 2113MiB, process RSS 최고 281MiB, mps float16
  - FLEURS 영어 n=28: 음성 277.7초를 90.9초에(실시간의 3.1배), 묶음 4, 불러오기 7.0초, GPU 메모리 2113MiB, process RSS 최고 444MiB, mps float16
  - FLEURS 한국어 n=36: 음성 441.2초를 278.2초에(실시간의 1.6배), 묶음 4, 불러오기 22.7초, GPU 메모리 2121MiB, process RSS 최고 272MiB, mps float16
- qwen3-asr-0.6b
  - FLEURS 한국어 n=3: 음성 50.6초를 150.5초에(실시간의 0.3배), 묶음 8, 불러오기 122.3초, GPU 메모리 2860MiB, process RSS 최고 794MiB, mps float16
  - FLEURS 한국어 n=16: 음성 226.4초를 286.7초에(실시간의 0.8배), 묶음 8, 불러오기 162.0초, GPU 메모리 3156MiB, process RSS 최고 559MiB, mps float16
  - FLEURS 영어 n=8: 음성 69.3초를 68.0초에(실시간의 1.0배), 묶음 4, 불러오기 33.7초, GPU 메모리 2860MiB, process RSS 최고 1291MiB, mps float16
  - Zeroth-Korean n=8: 음성 75.2초를 38.2초에(실시간의 2.0배), 묶음 4, 불러오기 33.7초, GPU 메모리 2868MiB, process RSS 최고 1291MiB, mps float16
- qwen3-asr-1.7b
  - FLEURS 한국어 n=12: 음성 179.3초를 189.7초에(실시간의 0.9배), 묶음 4, 불러오기 48.5초, GPU 메모리 5396MiB, process RSS 최고 1647MiB, mps float16
  - FLEURS 영어 n=8: 음성 69.3초를 132.1초에(실시간의 0.5배), 묶음 4, 불러오기 48.5초, GPU 메모리 5396MiB, process RSS 최고 1647MiB, mps float16
  - Zeroth-Korean n=8: 음성 75.2초를 133.8초에(실시간의 0.6배), 묶음 4, 불러오기 48.5초, GPU 메모리 5396MiB, process RSS 최고 1647MiB, mps float16
  - FLEURS 영어 n=32: 음성 320.7초를 147.3초에(실시간의 2.2배), 묶음 4, 불러오기 27.5초, GPU 메모리 6420MiB, process RSS 최고 2022MiB, mps float16

## 4. LoRA 역전파 한 번
- 시험 방법: 모델에 LoRA(rank 8, alpha 16, q_proj 와 v_proj)를 붙이고 FLEURS 한국어 앞 발화 묶음의 정답 글에 손실을 걸어 순전파와 역전파를 두 걸음 돌린다, 코드는 `research/seamone_research/lora_probe.py`, 결과 파일은 `.tmp/research/lora/<모델>-<device>-b<묶음>[-decoder].json`
  - 범위 all: 음성 인코더와 글 디코더의 q_proj, v_proj 모두에 붙인다
  - 범위 decoder: 글 디코더만 붙인다, 음성 인코더에는 학습할 가중치가 없어 역전파 그래프를 만들지 않는다(프로젝트 결정 "라벨 없이 모델을 키우는 방향" 의 "몸통은 얼려 추론만 하고 그 위의 작은 부분만 학습한다" 에 가깝다)
  - Qwen3-ASR 은 학습용 입력을 만드는 API 가 없어 추론과 같은 프롬프트 뒤에 정답 글을 붙여 만들고 프롬프트 토큰은 손실에서 가렸다, 이 시험은 역전파가 도는지와 비용을 보는 것이라 이 정도로 충분하다
- 성공 기준: 손실이 숫자로 나오고 LoRA 가중치의 기울기가 모두 유한하다
- 메모리: Apple GPU 는 `torch.mps.driver_allocated_memory`, CPU 는 process 의 최고 상주 메모리를 쟀다, Apple GPU 시험은 PyTorch 의 GPU 메모리 한도를 이 노트북의 추천 최대 작업 메모리(11.84GiB, `torch.mps.recommended_max_memory()` 조회)의 1.0배로 걸었다
- 결과
  - qwen3-asr-1.7b, Apple GPU(MPS), float16, 묶음 1, 범위 decoder: 성공, 손실 0.0970, 순전파 4.33초와 1.53초, 역전파 3.54초와 2.82초(두 걸음, 앞이 첫 걸음), LoRA 가중치 1,605,632개(전체 2,039,658,112개의 0.08%), 기울기 모두 유한, GPU 메모리 6460MiB, process RSS 최고 1295MiB
  - qwen3-asr-1.7b, Apple GPU(MPS), float16, 묶음 4, 범위 decoder: 실패, 오류 RuntimeError: MPS backend out of memory (MPS allocated: 11.80 GiB, other allocations: 640.00 KiB, max allowed: 11.84 GiB). Tried to allocate 768.00 MiB on shared pool. Use PYTORCH_MPS_HIGH_WATERMARK_RATIO=0.0 to disable upper limit for memory allocations (may cause system failure).
  - whisper-large-v3-turbo, Apple GPU(MPS), float16, 묶음 4, 범위 decoder: 성공, 손실 0.3679, 순전파 10.41초와 9.39초, 역전파 2.87초와 0.54초(두 걸음, 앞이 첫 걸음), LoRA 가중치 327,680개(전체 809,205,760개의 0.04%), 기울기 모두 유한, GPU 메모리 3147MiB, process RSS 최고 439MiB
  - whisper-small, CPU, float32, 묶음 4, 범위 decoder: 성공, 손실 0.4319, 순전파 25.57초와 27.97초, 역전파 9.66초와 3.41초(두 걸음, 앞이 첫 걸음), LoRA 가중치 589,824개(전체 242,324,736개의 0.24%), 기울기 모두 유한, process RSS 최고 1016MiB
  - whisper-small, CPU, float32, 묶음 4, 범위 all: 성공, 손실 0.4319, 순전파 142.18초와 71.56초, 역전파 60.43초와 146.9초(두 걸음, 앞이 첫 걸음), LoRA 가중치 884,736개(전체 242,619,648개의 0.36%), 기울기 모두 유한, process RSS 최고 1315MiB
  - whisper-small, Apple GPU(MPS), float16, 묶음 1, 범위 all: 성공, 손실 0.3159, 순전파 11.5초와 1.13초, 역전파 22.44초와 2.51초(두 걸음, 앞이 첫 걸음), LoRA 가중치 884,736개(전체 242,619,648개의 0.36%), 기울기 모두 유한, GPU 메모리 3364MiB, process RSS 최고 764MiB
  - whisper-small, Apple GPU(MPS), float16, 묶음 4, 범위 decoder: 성공, 손실 0.4324, 순전파 2.52초와 1.88초, 역전파 1.63초와 0.49초(두 걸음, 앞이 첫 걸음), LoRA 가중치 589,824개(전체 242,324,736개의 0.24%), 기울기 모두 유한, GPU 메모리 2379MiB, process RSS 최고 1039MiB
- 결과 파일이 없는 실행 한 번: whisper-small, Apple GPU, fp16, 묶음 4, 범위 all 은 05:48 에 시작해 06:20 에 멈출 때까지 32분 동안 두 걸음을 마치지 못했다
  - process 가 쓴 CPU 시간은 약 3분이었고 process 메모리가 11GB 로 GPU 메모리 한도(11.84GiB, `torch.mps.recommended_max_memory()` 조회)에 가까웠다
  - 호출 스택 표본(`sample`)에서 주 thread 가 `torch.mps.synchronize` 안의 GPU 작업 완료 대기였다, Python 이 멈춘 것이 아니라 GPU 작업이 느렸다
  - 원인은 추측이다: 음성 인코더 전체로 흐르는 역전파의 활성값이 한도에 닿았다, 같은 모델이 묶음 1 에서는 GPU 메모리 3364MiB 로 돈다
- 읽는 법
  - 글 디코더만 학습하는 범위 decoder 는 Apple GPU 에서 세 모델 모두 돈다, 두 번째 걸음(커널 준비가 끝난 뒤) 기준으로 whisper-small 묶음 4 는 2.4초(순전파 1.88초와 역전파 0.49초)에 GPU 메모리 2379MiB, whisper-large-v3-turbo 묶음 4 는 9.9초(9.39초와 0.54초)에 3147MiB, Qwen3-ASR-1.7B 묶음 1 은 4.4초(1.53초와 2.82초)에 6460MiB 다
  - Qwen3-ASR-1.7B 는 묶음 4 가 GPU 메모리 한도를 넘어 실패했다(오류 문구: MPS allocated 11.80 GiB, max allowed 11.84 GiB), 어휘가 15만 개라 글 위치마다 나오는 점수 행렬이 커서 그렇다는 것은 추측이다
  - whisper-large-v3-turbo 는 시간의 대부분이 순전파다(9.39초): 기울기가 필요 없는 32층 음성 인코더가 도는 시간이 걸음마다 든다, 앞부분을 얼려 두는 구조(프로젝트 결정 "라벨 없이 모델을 키우는 방향")에서는 인코더 출력을 저장해 다시 쓰는 설계를 검토할 만하다(추측이고 이 이슈가 확인하지 않았다)
  - 음성 인코더까지 학습하는 범위 all 은 whisper-small 묶음 1 이 Apple GPU 에서 3.6초(순전파 1.13초와 역전파 2.51초)에 3364MiB 로 돌지만, 묶음 4 는 끝나지 않았다(묶음 1 의 GPU 메모리 3364MiB 의 3.5배가 한도 11.84GiB 에 가깝다)
  - CPU 에서는 whisper-small 이 범위 decoder 묶음 4 로 한 걸음 약 31초(27.97초와 3.41초), 범위 all 묶음 4 로 약 210초다, 둘 다 load average 가 45 에서 100 인 때의 값이라 부하가 없으면 더 빠를 것이다(확인하지 못했다)
  - LoRA 를 붙인 직후에는 새 가중치(B)가 0 이라 손실이 원래 모델의 손실과 같다: 같은 앞 4개 발화에서 whisper-small 의 CPU(fp32)가 0.4319, Apple GPU(fp16)가 0.4324 로 맞아 두 장치와 dtype 의 순전파가 같다는 확인이 된다
- 아직 하지 못한 것: whisper-large-v3-turbo 와 Qwen3-ASR-1.7B 의 범위 all 시험, whisper-large-v3-turbo 와 Qwen3-ASR-1.7B 의 CPU 시험, 이유는 부하(load average 40 에서 140)와 디스크 여유 17GiB 안팎에서 이 시험이 부하를 키울 것 같아 대기열에서 뺐다(07:00)

## 5. 한계
- 표본이 작다: Qwen3-ASR-1.7B 와 large-v3-turbo 의 차이가 표준오차의 약 2.7배라 확정이 아니고, Zeroth-Korean 은 Qwen3-ASR-1.7B 가 8개뿐이다, 같은 발화 수를 늘리면 표준오차가 발화 수의 제곱근에 반비례해 준다(예: 64개에서 128개로 늘리면 약 1/1.4)
- 낭독 음성이다: 세 세트 모두 사람이 문장을 읽은 음성이다, 이 앱이 받아쓸 여럿이 말하는 회의 음성(소음, 겹친 말, 끊긴 말)에서의 오류율을 대표하지 않는다
- 숫자와 표기: 정규화가 숫자를 글자로 바꾸지 않아서 모델이 숫자로 쓰는 버릇이 Zeroth-Korean CER 에 섞인다(3-4절), 영문 이름과 괄호 속 표기도 오류로 센다
- 풀어 쓰기 하나만 썼다: greedy 한 가지로 한 번 돌렸다, Whisper 의 온도 되돌리기나 빔 탐색 같은 권장 디코딩은 쓰지 않았다
- 모델 version 은 2026-11-19 에 받은 Hugging Face 의 커밋이다(2-1절), 나중에 모델 저장소가 바뀌면 값이 달라질 수 있다
- 묶음 크기가 모델마다 다르다: whisper-base 와 whisper-small 은 16, whisper-large-v3-turbo 와 Qwen3-ASR-1.7B 는 4, Qwen3-ASR-0.6B 는 한국어 앞 19개를 시험 삼아 돌린 두 번의 실행에서 묶음 3 과 8 로 쟀고 영어와 Zeroth-Korean 은 4 로 쟀다, 묶음 크기는 greedy 결과를 바꾸지 않아야 하지만 fp16 계산 순서가 달라져 드물게 글자 하나가 바뀔 수 있다(확인하지 못했다)
- 시간 기록(3-5절)에는 모델을 처음 쓸 때의 GPU 준비 시간이 들어 있고 다른 빌드와 겹쳤다, 모델끼리 속도를 견주는 근거로 쓰지 않는다
- 모델 둘이 겹쳐 돈 구간이 있다: 이슈 board 7절 머리의 "모델은 한 번에 하나만 올린다" 를 어기고 whisper-base 측정 중에 Qwen3-ASR-0.6B 시험 실행이, whisper-small 측정 중에 Qwen3-ASR-0.6B 시험 실행과 whisper-small 의 CPU LoRA 시험(05:07 에서 05:16)이 같이 돌았고, 그 CPU LoRA 시험이 Qwen3-ASR-1.7B 묶음 32 실행(05:13 부터)과도 겹쳤다(메인이 05:30 에 그 규칙을 다시 짚었다), 이 구간의 시간 기록이 느리고 글 결과는 영향이 없다(실행마다 가설 글이 따로 쌓인다)
- whisper-large-v3 를 재지 못했다
  - 해 본 것: 가중치 3.09GB 를 `.tmp/hf` 에 받았고 측정을 대기열에 넣었다, 메인의 메모리와 디스크 부족 지시(05:30, 05:58)로 측정을 시작하지 못했고 디스크 여유가 15GB 선이라 가중치를 지웠다(06:03)
  - 무엇이 되면 풀리나: 디스크 여유 18GB 이상과 메모리 여유가 있을 때 `make -C research prefetch MODELS=whisper-large-v3` 다음에 `make -C research cer MODELS=whisper-large-v3 N="fleurs_ko=8 fleurs_en=4 zeroth_ko=4"`(내려받기 5분, 측정 5분 안팎으로 어림했고 확인하지 못했다)

## 6. 추천
- 이 문서의 몫은 시작 모델 결정(프로젝트 결정 "시작 모델", 미결정)에 쓸 오류율 근거와 후보 순서다, 결정은 속도를 잰 뒤 메인이 한다
- 한국어 오류율이 낮은 순서(3-3절): Qwen3-ASR-1.7B, Whisper large-v3-turbo(와 구별되지 않는 Qwen3-ASR-0.6B), Whisper small, Whisper base
- 데스크톱 계층(기준 기기 M1 노트북, 프로젝트 결정 "가속기를 쓰고 쓸 수 있는 속도 안에서 가장 큰 모델을 고른다" 의 기준: 가속기로 실시간의 3배 이상, CPU 만으로 실시간 이상)
  - 첫 후보 Qwen3-ASR-1.7B: 오류율이 가장 낮다(large-v3-turbo 보다 한국어 -1.25%p, 영어 -0.64%p, 표준오차의 약 2.7배와 2.8배라 확정은 아니다), 가중치 파일 4.70GB(bf16, 매개변수 23억 4920만 개)
  - 대안 Whisper large-v3-turbo: 오류율이 Qwen3-ASR-1.7B 보다 한국어 1.3%p, 영어 0.6%p 높고, 가중치 파일 1.62GB(fp16, 8억 890만 개)로 약 1/3 크기다
  - 가르는 법: 속도 측정 이슈가 Rust 엔진으로 잰 Qwen3-ASR-1.7B 속도가 기준을 넘으면 Qwen3-ASR-1.7B, 못 넘으면 Whisper large-v3-turbo
  - 이 측정의 Python 과 MPS 경로 속도는 참고값이다(3-5절): Qwen3-ASR-1.7B 는 실시간의 0.5배에서 2.2배, large-v3-turbo 는 1.6배에서 3.1배이고 부하에 따라 크게 달라진다, 비슷한 부하의 06:36 과 06:38 영어 실행은 2.2배 대 3.1배였다
  - 추측: Qwen3-ASR-1.7B 는 한 번도 3배를 넘지 못했고 large-v3-turbo 는 3배 안팎이라, Rust 엔진으로도 Qwen3-ASR-1.7B 가 기준을 못 넘고 large-v3-turbo 가 데스크톱 계층이 될 가능성이 있다, 속도 측정 이슈가 확인한다
- 폰 계층(폰 가속기로 실시간 이상)
  - Qwen3-ASR-0.6B: large-v3-turbo 와 같은 수준의 오류율이다(+0.08%p, 구별되지 않는다, 같은 한국어 발화 27개), 가중치 파일 1.88GB 라 폰 메모리에 올릴 수 있는지는 속도 측정 이슈가 확인한다(양자화는 이 이슈가 재지 않았다)
  - Whisper small: 가중치 파일 967MB(fp32 로 저장, fp16 으로 올리면 약 0.5GB), 같은 발화 27개에서 Qwen3-ASR-0.6B 에서 Whisper small 을 뺀 CER 차이가 -1.77%p(±0.98%p, 표준오차의 약 1.8배)라 Whisper small 의 오류율이 약하게 높은 쪽이고 이 표본으로는 확정하지 못한다
  - 가르는 법: 속도 측정 이슈가 폰에서 잰 속도와 메모리로 Qwen3-ASR-0.6B 가 들어가면 Qwen3-ASR-0.6B, 아니면 Whisper small
- 라벨 없는 FL 모의실험 이슈에 넘기는 것: 남긴 가중치 whisper-small 0.97GB, whisper-large-v3-turbo 1.5GB, Qwen3-ASR-1.7B 4.4GB(`.tmp/hf`), 지운 것은 whisper-base, Qwen3-ASR-0.6B, whisper-large-v3, LoRA 는 글 디코더만 붙이는 범위가 Apple GPU 에서 세 모델 모두 돈다(whisper-small 묶음 4, whisper-large-v3-turbo 묶음 4, Qwen3-ASR-1.7B 묶음 1, 4절), 평가 코드는 `research/seamone_research/score.py` 와 `compare.py`
- 판단거리(메인이 정한다)
  1. Zeroth-Korean 을 늘려 Qwen3-ASR-1.7B 와 large-v3-turbo 를 더 견줄지, 추천은 늘린다: 지금 Qwen3-ASR-1.7B 는 8개뿐이다, 부하가 낮을 때 `make -C research cer MODELS=qwen3-asr-1.7b SETS=zeroth_ko N=60` 로 52개를 더 재고(약 8분에서 15분) large-v3-turbo 는 44개를 더 잰다(약 3분)
  2. 폰 계층 후보를 재려면 Qwen3-ASR-0.6B 를 다시 받아야 한다(1.9GB, 디스크 여유 18GB 이상일 때), 추천은 속도 측정 이슈가 폰 계층 후보를 좁힌 뒤에 받는다
  3. whisper-large-v3 를 재지는 선택이다(오류율 상한 참고용이고 속도가 느려 계층 후보가 되기 어렵다), 추천은 재지 않는다

## 7. 다시 돌리는 법과 자리
- 환경: `make -C research install`(uv 가 Python 3.12 와 `research/uv.lock` 의 version 으로 `research/.venv` 를 만든다)
- 검사: `make -C research lint`, `make -C research test`
- 표본 id 목록과 시험 음성: `make -C research eval-ids`(`research/data/eval_ids.json` 을 만들고 음성을 메인 레포의 `.tmp/hf/seamone_eval/` 에 둔다)
- 측정: `make -C research cer`(여섯 모델 전부, 오래 걸린다), 빠른 확인은 `make -C research cer MODELS=whisper-small N=5`, 세트마다 다르게는 `N="fleurs_ko=16 fleurs_en=8 zeroth_ko=12"`
- 저장된 가설 글로 CER 다시 계산: `make -C research score MODELS="whisper-small whisper-large-v3-turbo"`
- 같은 발화로 모델 견주기: `make -C research compare MODELS="a b" SETS="fleurs_ko zeroth_ko" PAIR="a b"`
- LoRA 시험: `make -C research lora LORA_MODELS=whisper-small LORA_DEVICES=mps LORA_BATCH=1`
- 가설 글 자리: `<레포>/.tmp/research/hyp/<모델>/<세트>.jsonl`(한 줄에 id, 정답, 가설, 음성 길이), 이 폴더는 작업 컴퓨터를 옮길 때 버려도 되는 것이다
