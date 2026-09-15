## Lada GUI v0.7.0

사양을 감지해 **설정을 자동으로 맞춰주고**, 앱 안에서 **새 버전으로 바로 업데이트**할 수 있게 되었습니다.
This release tunes itself to your hardware and can update itself in place.

### ✨ 새로운 기능 / New features

- **사양 자동 감지 + 자동 설정 / Hardware detection and auto settings**
  - GPU·VRAM·CPU 코어·RAM·Docker 메모리 상한·임시폴더 여유를 감지해, **병렬 작업 수 / 클립 길이 / 컨테이너 메모리 / FP16 / 인코더**를 계산해 적용합니다. 설정에서 `자동 ↔ 수동`을 고를 수 있습니다.
  - Detects GPU/VRAM, CPU cores, RAM, Docker's memory ceiling and free temp space, then sets parallel jobs, clip length, container memory, FP16 and encoder. Switch between `Auto` and `Manual` in settings.
  - **숫자만 보여주지 않고 근거를 함께 표시합니다** — 예: `4 concurrent jobs — limited by CPU cores (VRAM allows 8, CPU 4, Docker RAM 4)`.
  - It shows *why*, not just *what* — e.g. `4 concurrent jobs — limited by CPU cores (VRAM allows 8, CPU 4, Docker RAM 4)`.
  - 추천값은 추측이 아니라 **실측 기반**입니다. RTX 5090에서 클립 길이별 VRAM/처리시간을 직접 측정해 표로 넣었습니다.
  - The recommendations come from measurements, not guesses: VRAM and processing time were measured per clip length on an RTX 5090.
  - ⚠️ **기존 사용자의 설정은 그대로 유지됩니다.** 직접 맞춰둔 값을 덮어쓰지 않도록, 자동 모드는 신규 설치에서만 기본으로 켜집니다. 원하면 설정에서 켜세요.
  - Existing installs keep their settings — auto mode is only the default on a fresh install, so hand-tuned values are never overwritten. Turn it on in settings if you want it.
- **앱 내 자동 업데이트 / In-app auto-update**
  - 새 버전이 나오면 상단에 배너가 뜨고, 클릭 한 번으로 내려받아 설치·재시작합니다. 모든 업데이트는 서명으로 검증됩니다.
  - A banner appears when a new version is out; one click downloads, verifies and installs it. Every update is signature-verified.
  - **작업 중에는 업데이트할 수 없습니다** — Windows는 설치 과정에서 앱을 종료시키므로, 진행 중인 작업이 사라지지 않도록 막아둡니다.
  - Updates are blocked while jobs are running: Windows exits the app to install, which would kill work in progress.

### 🐞 버그 수정 / Bug fixes

- **무한 재시도 중단 / Retries are now bounded**
  - 외장하드가 빠지는 등으로 실패하면 30초마다 **영원히 재시도**하며 작업이 끝나지 않던 문제를 고쳤습니다. 이제 복구 불가능한 원인(드라이브 유실 등)은 즉시 실패로 처리하고, 그 외에는 최대 10회까지만 간격을 늘려가며 재시도한 뒤 정리합니다.
  - A failure used to retry every 30s forever — a disconnected drive left a job spinning indefinitely. Unrecoverable causes now fail immediately, and anything else retries at most 10 times with growing backoff.
  - 포기한 작업이 "처리 중"에 멈춰 있지 않고 **실패로 명확히 표시**됩니다.
  - A job that gives up is now clearly marked failed instead of sitting on "processing".
- **VR 동시 처리 시 메모리 초과 / VR jobs could overcommit memory**
  - VR 작업이 메모리 무제한으로 실행되어, 병렬 작업 수가 2 이상이면 4K 컨테이너 여러 개가 상한 없이 경쟁할 수 있었습니다. 이제 각 패스가 적정 한도를 받습니다.
  - VR passes ran without a memory limit, so with parallel jobs above 1 several 4K containers could compete unbounded. Each pass now gets a proper limit.
- 최대 클립 길이 툴팁이 단위를 "초"라고 잘못 안내하던 것을 **"프레임"**으로 수정했습니다.
- Fixed the max clip length tooltip, which described the unit as seconds when it is frames.

### ℹ️ 참고 / Notes

- **이번 버전은 수동으로 설치해야 합니다.** v0.6.0에는 업데이트 기능이 없어 자동으로 받을 수 없습니다. v0.7.0부터는 앱 안에서 업데이트됩니다.
- **This one must be installed by hand.** v0.6.0 has no updater, so it cannot fetch this automatically. From v0.7.0 onward updates happen in-app.
- 설정을 자동으로 바꾸고 싶다면 설정 패널에서 `설정 방식`을 **자동**으로 바꾸세요.
- To let the app tune itself, set `Settings Mode` to **Auto** in the settings panel.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
