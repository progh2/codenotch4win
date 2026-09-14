# PRD — Codenotch for Windows (codenotch4win)

- 작성일: 2026-09-14 · 최종 갱신: 2026-09-14
- 작성자: progh2 (with Claude)
- 상태: v1.1 — M1 완료 (v0.4.0~v0.4.2 출시), M2 대기
- 관련 문서: [README](../README.md) · [README.ko](../README.ko.md)

## 진행 현황 (2026-09-14 기준)

| 릴리스 | 내용 |
|---|---|
| **v0.4.0** | 첫 바이너리 릴리스 — CI(#1)·릴리스 워크플로(#2) 완성, NSIS 인스톨러 + 포터블 zip 발행 |
| **v0.4.1** | Cursor Auth0/엔터프라이즈 로그인 수정(#16) — `stripeMembershipAuthId` 부재 시 JWT `sub` 폴백, 엔터프라이즈 used/limit 파싱, headline을 대시보드의 Auto 행과 일치 |
| **v0.4.2** | doctor에 usage-summary 응답 키 구조 진단 추가 — Grok Bot 버킷(#15) 발견용, 문자열 전부 마스킹 |
| **v0.4.3** | **Grok Bot 지원**(#15) — Cursor 호버 카드에 주간 사용률 막대. usage-summary에는 버킷이 없음을 진단으로 확인 후, Connect RPC `DashboardService/GetSandUsageStatus`(내부 코드명 "Sand", Bearer 토큰 재사용)로 구현 |

실기 검증(#3): 설치·실행·프로바이더 표시(Claude/Codex/Cursor) 확인 완료. 남은 것 — 재부팅 후 자동 실행, 포터블 zip 동작 확인. 발견된 개선점: 업그레이드 설치가 실행 중인 구버전을 종료하지 않아 단일 인스턴스 가드에 막힘(#17, M3).

## 1. 배경

[Codenotch](https://github.com/vinzdg/codenotch)는 Claude Code, Codex, Cursor, Antigravity 등
AI 코딩 도구의 사용량 한도를 화면 가장자리 "노치"에 상시 표시해주는 macOS 앱이다.
어떤 벤더도 "세션 한도를 N% 썼다"는 공식 API를 제공하지 않기 때문에, 각 벤더의 앱이
로컬에 보관하는 자격증명·캐시·DB를 읽어 그 벤더의 자체 엔드포인트에서 사용량을 가져온다.

윈도우 포트([Im-Midi/codenotch-windows](https://github.com/Im-Midi/codenotch-windows),
Rust + Tauri 2, MIT)가 이미 존재하고 원본의 `windows/` 트리로 병합되어 있으나,
**배포가 전혀 없다**:

| | macOS 원본 | 윈도우 포트 (현재) |
|---|---|---|
| 바이너리 배포 | ✅ Releases에 `.dmg` | ❌ 릴리스 0건, 소스 빌드만 |
| 자동 업데이트 | ✅ Sparkle (EdDSA 서명) | ❌ 없음 |
| 부팅 자동 실행 | ✅ | ✅ (HKCU Run 키, `--silent`) |
| CI | ✅ | ❌ 워크플로 없음 |
| 한국어 | ❌ | ❌ (i18n 레이어는 존재) |

**문제 정의:** 윈도우 사용자는 Rust(MSVC) 툴체인을 설치하고 직접 빌드해야만 쓸 수 있고,
업데이트도 매번 수동 재빌드다. 코드는 있는데 제품이 없다.

## 2. 목표

1. **다운로드 즉시 실행** — GitHub Releases에서 실행 파일을 받아 더블클릭으로 사용 시작.
2. **자동 업데이트** — 설치 후 사용자가 신경 쓰지 않아도 최신 버전 유지.
3. **부팅 자동 실행** — 기존 구현을 검증하고 첫 실행 시 켜도록 제안.
4. **공개 개발** — GitHub 이슈/마일스톤으로 단계적·투명하게 진행.
5. **한국어 사용자 경험** — 문서와 UI의 한국어화.

### 비목표 (Non-goals)

- 프로바이더 로직 재작성 — 기존 포트의 구현을 그대로 사용하고, 수정은 업스트림 추종 위주.
- macOS/Linux 지원 — macOS는 원본이 담당. 이 프로젝트는 윈도우 전용.
- 코드 서명 인증서(EV/OV) 구매 — 초기에는 무서명 배포(SmartScreen 경고 감수) + updater
  수준의 minisign 서명만. 인증서는 v1.0 이후 검토.
- 모바일 연동(Phone Link) — 원본의 기능이지만 이 포크의 범위 밖.

## 3. 대상 사용자와 시나리오

**대상:** 윈도우에서 Claude Code / Codex / Cursor / Antigravity를 쓰는 개발자.
Rust 툴체인을 깔고 싶지 않고, 사용량 한도가 얼마나 남았는지 수시로 궁금한 사람.

**시나리오:**

1. **설치** — Releases에서 `Codenotch-Setup-*.exe`를 받아 실행 → 관리자 권한 없이 설치
   → 화면 오른쪽 가장자리에 필이 나타남 → 첫 실행 온보딩에서 "Windows 시작 시 자동 실행"
   을 제안받고 켬.
2. **일상 사용** — 코딩 중 마우스를 노치에 올리면 호버 카드에 세션/주간 사용량 막대가
   펼쳐짐. Claude가 입력을 기다리면 링이 주황색으로 맥박.
3. **업데이트** — 새 버전이 릴리스되면 앱이 감지해 백그라운드로 받고, 재시작 시 적용.
   포터블 사용자는 알림만 받고 직접 새 파일을 받음.

## 4. 기능 요구사항

### FR1 — 실행 파일 배포 (M1)

- 태그 푸시(`v*`) 시 GitHub Actions `windows-latest` 러너에서 자동 빌드.
- 산출물 2종을 GitHub Release에 첨부:
  - `Codenotch-Setup-<ver>.exe` — NSIS 인스톨러 (기존 `tauri.conf.json`의 `nsis` 타겟),
    per-user 설치, Windows 10용 WebView2 부트스트랩 포함.
  - `codenotch-<ver>-portable.exe` — `target/release/codenotch.exe` 단일 파일.
- PR/푸시 시 `cargo check` + `cargo test` CI로 빌드 상시 검증.

### FR2 — 부팅 자동 실행 (M1 검증, M3 개선)

- 기존 `codenotch/src/autostart.rs` (HKCU `...\CurrentVersion\Run` + `--silent`) 유지.
- M1: 릴리스 빌드에서 동작 검증 (인스톨러 설치 경로 기준으로 Run 키가 올바른지).
- M3: 첫 실행 온보딩에서 자동 실행 활성화를 1회 제안 (기본 강제 아님 — 사용자 선택).

### FR3 — 자동 업데이트 (M2)

- `tauri-plugin-updater` 통합, `createUpdaterArtifacts: true`.
- 엔드포인트: `https://github.com/progh2/codenotch4win/releases/latest/download/latest.json`.
- minisign 키로 서명 (`TAURI_SIGNING_PRIVATE_KEY` GitHub Actions 시크릿; 공개키는
  `tauri.conf.json`에 내장). 서명 검증 실패 시 업데이트 미적용.
- NSIS 인스톨러 설치본만 자동 업데이트. 포터블 exe는 버전 확인 후 알림만
  (스스로 교체하지 않음).
- 업데이트 확인 주기: 시작 시 + 24시간 간격. 설정 창에 "지금 확인" 버튼과 끄기 옵션.

### FR4 — 프로바이더 모니터링 (기존 유지, M4 확장)

- 기존 4종(Claude, Codex, Cursor, Antigravity) 유지. 프로바이더 코드는 업스트림 추종.
- Cursor는 Auth0/엔터프라이즈 로그인 폴백 포함 (v0.4.1, 업스트림 #34 이식).
- M4 확장 후보:
  - **Grok Bot** (#15): ✅ v0.4.3에서 구현 — Cursor 셀 호버 카드에 주간 막대.
    `POST api2.cursor.sh/aiserver.v1.DashboardService/GetSandUsageStatus`
    (Bearer = state.vscdb의 액세스 토큰, `Connect-Protocol-Version: 1`, 본문 `{}`).
    풀드 엔터프라이즈/개인 할당량 없는 계정은 미표시, 실패해도 기본 Cursor 표시 유지.
  - Grok Build CLI: `~/.grok/auth.json` + `cli-chat-proxy.grok.com/v1/billing` —
    맥판(GrokUsage.swift)에 응답 포맷 문서화됨, 포팅 난이도 낮음.
  - Gemini CLI, GitHub Copilot (macOS 원본에 소스 로직 존재 → 포팅 참고).

### FR5 — 한국어 (M3)

- 기존 `i18n.rs` 레이어에 한국어 로케일 추가 (트레이 메뉴, 설정 창, 호버 카드 문자열).
- 문서는 영/한 병행 (README.md / README.ko.md).

## 5. 비기능 요구사항

- **상주 부담:** 유휴 시 CPU ~0%, 메모리는 WebView2 포함 상식적 수준. 폴링 주기는 기존
  포트의 정책(시작 시 + 호버 시 + 주기적, 프로바이더별 백오프) 유지.
- **권한:** 관리자 권한 불요 (설치·자동 실행·업데이트 모두 per-user).
- **호환성:** Windows 11 기본 지원, Windows 10은 WebView2 부트스트랩으로 지원 (M3에서 검증).
- **프라이버시:** 자격증명은 로컬 파일에서만 읽고 해당 벤더 엔드포인트로만 전송.
  텔레메트리 없음. 진단(`doctor`) 출력은 기존의 시크릿 마스킹 유지.
- **투명성:** 추측값 표시 금지 — 읽기 실패는 `stale`/`needsAuth`/`error` 상태로 표시
  (업스트림 원칙 계승).

## 6. 기술 설계 요약

- **스택:** Rust + Tauri 2 / WebView2 (기존 포트 그대로). 워크스페이스:
  `codenotch/` (앱) + `codenotch-hook/` (Claude Code 훅 헬퍼).
- **릴리스 파이프라인 (구현됨):** `.github/workflows/release.yml` —
  `on: push: tags: ['v*']` → `windows-latest` → 태그·버전 일치 검사 → hook 헬퍼 빌드 후
  사이드카 스테이징 → `cargo tauri build --config tauri.sidecar.conf.json` → 인스톨러 +
  포터블 zip(메인 exe + codenotch-hook.exe) → `gh release create`. M2에서 updater
  아티팩트(`latest.json`) 추가 예정.
- **사이드카 교훈:** `externalBin`을 `tauri.conf.json`에 두면 tauri-build가 컴파일
  시점에 파일 존재를 검사해 일반 `cargo check`가 깨짐 → 릴리스 빌드에서만
  `--config tauri.sidecar.conf.json` 오버레이로 병합. `hooks_install.rs`는
  `codenotch-hook.exe`가 메인 exe 옆에 있길 기대하므로 두 배포 형태 모두 포함.
- **CI:** `.github/workflows/ci.yml` — PR/main 푸시 시 `cargo check --workspace` +
  `cargo test --workspace` + `cargo fmt --check`.
- **버전 관리:** `codenotch/Cargo.toml` + `tauri.conf.json` 의 버전을 태그와 일치시킴
  (릴리스 워크플로에서 불일치 시 실패).
- **업스트림 동기화:** `upstream` 리모트(Im-Midi/codenotch-windows)를 주기적으로 머지.
  이 포크 고유 변경(updater, CI, 한국어)은 충돌 최소화를 위해 가능한 한 별도 파일/모듈로.
  검증된 파이프라인은 업스트림에 PR로 기여 (M4).

## 7. 마일스톤

| 마일스톤 | 버전 | 내용 | 완료 기준 |
|---|---|---|---|
| **M1 첫 배포판** ✅ | v0.4.x | CI + 릴리스 워크플로, 첫 바이너리 릴리스 | ✅ Releases에서 받은 인스톨러로 실행·프로바이더 표시 확인 (2026-09-14) |
| **M2 자동 업데이트** | v0.5.0 | updater 통합, 서명, latest.json | v0.5.0 설치본이 v0.5.1로 자동 업데이트되는 E2E 확인 |
| **M3 첫 실행 경험** | v0.6.0 | 온보딩(자동 실행 제안), 한국어 UI, Win10 검증 | 새 사용자가 문서 없이 설치→자동실행 설정 완료; UI 한국어 표시 |
| **M4 확장** | v1.0.0 | 프로바이더 추가, 업스트림 기여, 문서 정비 | 신규 프로바이더 1종 이상 동작; 업스트림 PR 제출 |

## 8. 성공 지표

- 릴리스 자산 다운로드 수 (GitHub Releases 통계).
- "빌드가 안 돼요"류 이슈 0건 (바이너리 배포로 원천 차단).
- 자동 업데이트 성공률: 릴리스 후 기존 설치본이 별도 조치 없이 갱신됨.
- 업스트림에 릴리스 파이프라인 기여 PR 병합.

## 9. 리스크와 대응

| 리스크 | 영향 | 대응 |
|---|---|---|
| 프로바이더의 비공식 엔드포인트/포맷 변경 | 사용량 표시 중단 | 업스트림이 활발히 추종 중 → `upstream` 머지로 흡수; 실패는 `stale`로 표시되어 오정보 없음 |
| 무서명 exe의 SmartScreen 경고 | 설치 이탈 | README에 경고 안내 명시; 다운로드 축적으로 평판 완화; v1.0 이후 코드 서명 인증서 검토 |
| 업스트림 대규모 리팩터링과 포크 변경 충돌 | 머지 비용 | 포크 고유 변경을 별도 모듈로 격리; 검증되는 대로 업스트림에 기여해 diff 축소 |
| 이 세션(개발 환경)이 Linux — 로컬 윈도우 실행 불가 | 수동 검증 지연 | 빌드는 GitHub Actions windows-latest에서; 실행 검증은 사용자의 윈도우 머신에서 수행 |
| Tauri updater가 포터블 exe를 지원하지 않음 | 포터블 사용자 수동 업데이트 | 설계에 반영: 포터블은 알림만 (FR3) |

## 10. 미해결 질문

- 포터블 zip의 신버전 알림 구현 방식 (updater 플러그인의 check-only 사용 vs GitHub API 직접 조회) — M2에서 결정.
- 온보딩에서 자동 실행 기본값을 "제안"으로 할지 "기본 켬 + 옵트아웃"으로 할지 — M3에서 결정.
- 프로젝트 독자 이름(codenotch4win) 유지 vs 업스트림 병합 후 통합 — M4에서 업스트림과 논의.
- ~~Grok Bot 주간 버킷이 실려 오는 엔드포인트~~ — 해결(v0.4.3): usage-summary에 없음을
  확인 후 `GetSandUsageStatus` Connect RPC로 구현.
