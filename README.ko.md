# Codenotch for Windows (codenotch4win)

[English README](README.md) · [PRD](docs/PRD.md)

[Codenotch](https://github.com/vinzdg/codenotch)의 윈도우 버전입니다 — 화면 가장자리에 붙어서
두 가지 질문에 한눈에 답해주는 "사용량 노치": **AI 사용량이 얼마나 남았나**, 그리고
**Claude가 아직 작업 중인가**.

이 저장소는 [Im-Midi/codenotch-windows](https://github.com/Im-Midi/codenotch-windows)
(원본 저장소의 `windows/` 트리로 병합된 윈도우 포트)의 포크입니다. 포트 자체는 완성도가
높지만 **배포가 없었습니다** — 이 포크는 그 빈 부분을 채우기 위해 존재합니다:

- **빌드된 실행 파일 배포** — 다운로드해서 바로 실행, Rust 툴체인 불필요
- **자동 업데이트** — GitHub Releases에서 앱이 스스로 최신 버전 유지
- **부팅 시 자동 실행** — 설정에서 체크 한 번 (포트에 이미 구현됨; 첫 실행 시 제안)
- **한국어 UI** — 기존 i18n 레이어 위에 한국어 번역 추가

각 항목의 진행 상황은 [로드맵](#로드맵)을 참조하세요.

## 무엇을 보여주나

macOS 원본과 같은 디자인 언어(역방향 라운드 필, 색상 그라데이션 링, 창별 막대가 있는
호버 카드)를 Rust + Tauri 2 / WebView2로 재구현했습니다. Swift 코드를 복사하지 않고,
각 프로바이더의 문서화된 동작과 와이어 포맷을 기반으로 재작성한 것입니다.

| 셀 | 소스 | 읽는 방식 |
|---|---|---|
| **Claude** | Claude Code가 `~/.claude/.credentials.json`에 보관하는 토큰으로 `GET https://api.anthropic.com/api/oauth/usage` 호출 | 세션/주간 윈도우, 429 백오프, 오래된 값은 흐리게 표시. Claude 세션이 작업 중이면 링 안에 얇은 호가 회전하고, 입력을 기다리면 주황색으로 맥박. |
| **Codex** | `~/.codex/auth.json`의 로컬 로그인(읽기 전용), 없으면 최신 세션 스냅샷 | 유료 플랜은 5시간 + 주간, 무료는 월간 윈도우를 실시간 표시. |
| **Cursor** | 에디터 자체 세션(`state.vscdb`) → `cursor.com/api/usage-summary` | 포함 사용량 / API 사용량 / 온디맨드, 결제 주기 말 리셋. |
| **Antigravity** | 공식 `agy` CLI의 `/usage` 출력(설치 시), 없으면 로컬 `language_server` 브리지 | IDE를 켜지 않고도 공식 쿼터(Gemini & Claude/GPT, 5시간/주간) 표시. |

설치되지 않은 프로바이더는 셀 자체가 나타나지 않습니다.

## 설치

> **현재 상태:** 첫 바이너리 릴리스는 마일스톤 [M1](../../milestones)에서 진행 중입니다.
> 그 전까지는 아래의 소스 빌드를 이용하세요.

[Releases](../../releases) 페이지에 두 가지 형태로 배포됩니다:

- **`Codenotch-Setup-x.y.z.exe`** (NSIS 인스톨러) — 권장. 사용자별 설치(관리자 권한 불필요)이며
  **자동 업데이트 채널**입니다: 앱이 GitHub Releases를 확인해 스스로 업데이트합니다.
- **`codenotch-x.y.z-portable.zip`** — 포터블 빌드. 압축을 풀고 `Codenotch.exe`를 실행하면
  됩니다 (Claude Code 세션 감지용 `codenotch-hook.exe`가 함께 들어 있음).
  자동 업데이트는 없고, 새 버전이 나오면 알림만 표시합니다.

요구사항: Windows 10/11 + WebView2 런타임 (Windows 11은 기본 내장, Windows 10은
인스톨러가 자동 설치).

## 소스 빌드

준비물: Rust (MSVC 툴체인), WebView2 런타임.

```powershell
cargo build --release
.\target\release\codenotch.exe          # 주 모니터 오른쪽 가장자리에 필이 나타남
.\target\release\codenotch.exe doctor   # 자가진단: 자격증명, 데이터 소스, 아이콘, 훅
```

트레이 메뉴: **Settings…**, **Refresh usage now**, **Quit**. 나머지는 모두 설정 창에
있습니다: 작업표시줄 아이콘, 노치에 표시할 링, 크기, **Windows 시작 시 자동 실행**, 언어,
Claude Code 훅, 위치 초기화, 데이터 폴더(`%APPDATA%\codenotch` — 로그, 저장된 값,
아이콘 오버라이드).

## 개인정보 & 보안

- 자격증명은 각 벤더의 앱이 이미 내 컴퓨터에 보관 중인 파일에서만 읽으며, 해당 벤더의
  자체 엔드포인트로만 전송됩니다. 다른 곳으로는 아무것도 보내지 않습니다.
- 텔레메트리, 분석 수집 없음.
- 자동 업데이트 아티팩트는 서명되며, 업데이터가 서명 검증 후에만 적용합니다.
- 전부 사용자 권한으로 동작; 관리자 권한 불필요.

## 로드맵

[이슈](../../issues)와 [마일스톤](../../milestones)으로 진행합니다:

| 마일스톤 | 목표 |
|---|---|
| **v0.4.0 — 첫 바이너리 릴리스** | GitHub Actions 릴리스 파이프라인; 인스톨러 + 포터블 exe를 Releases에서 다운로드 가능하게 |
| **v0.5.0 — 자동 업데이트** | `tauri-plugin-updater` + 서명된 `latest.json`을 GitHub Releases에 |
| **v0.6.0 — 첫 실행 경험** | 온보딩(자동 실행 켜기 제안), 한국어 번역, Windows 10 검증 |
| **v1.0.0 — 확장** | 프로바이더 추가(Gemini CLI, GitHub Copilot 등), 업스트림 동기화·기여 |

## 업스트림과의 관계

이 포트는 업스트림의 디자인과 프로바이더 동작을 따릅니다. 개발은
[Im-Midi/codenotch-windows](https://github.com/Im-Midi/codenotch-windows)에서 이루어지며
[vinzdg/codenotch](https://github.com/vinzdg/codenotch)의 `windows/` 트리로 제공됩니다.
이 포크는 그 작업을 추적하고(`upstream` 리모트), 릴리스·자동 업데이트 파이프라인이
검증되면 업스트림에 기여하는 것을 목표로 합니다.

### 아이콘

프로바이더 마크는 [`@lobehub/icons-static-svg`](https://github.com/lobehub/lobe-icons)(MIT)의
SVG를 수정 없이 내장한 것입니다 — `codenotch/glyphs/NOTICE.md` 참조.
`%APPDATA%\codenotch\glyphs\`에 `claude|codex|cursor|gemini.svg`(또는 `.png`)를 넣으면
오버라이드됩니다. 각 마크는 해당 소유자의 상표입니다.

## 라이선스

MIT — [`LICENSE`](LICENSE) 참조. Codenotch의 디자인과 이름은
[업스트림 저자](https://github.com/vinzdg)에게, 윈도우 포트는
[Im-Midi](https://github.com/Im-Midi)에게 귀속되며, 이 포크는 그 위에 배포를 더합니다.
