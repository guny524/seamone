# seamone 작업 규칙

반복된 오류는 지시를 늘리기 전에 코드 구조나 타입으로 막는다. 그게 맞지 않으면 검사나
회귀 시험으로 막고, 실패 메시지에는 고칠 위치를 적는다. 수정 전 실패와 수정 후 성공을
확인한다. 앱·설치 파일·실기기 검증은 서로 구분하고 실행한 검증만 보고한다.
출처: [pstack /correct](https://github.com/cursor/plugins/blob/main/pstack/skills/correct/SKILL.md).

| 규칙 | 강제하는 곳 |
| --- | --- |
| UI·ASR·도메인·앱의 의존 방향은 우회하지 않는다. | `make check-architecture`; CI `domain` |
| 연구 가설 JSONL에서 개행으로 완료된 깨진 행은 읽기·재개 어느 쪽에서도 버리지 않는다. | `research/seamone_research/hypotheses.py`의 공용 파서와 `research/tests/test_hypotheses.py`; CI `research-test` |
