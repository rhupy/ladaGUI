## Lada GUI v0.8.0

**JASNA 엔진**을 선택할 수 있습니다. 같은 복원 모델로 훨씬 빠르고, VR을 내부에서 눈별로 처리합니다.
Adds the **JASNA engine** as a selectable option — same restoration model, much faster, with VR handled per-eye internally.

### ✨ 새로운 기능 / New feature

- **엔진 선택: Lada / JASNA** — 설정 패널 맨 위에서 고릅니다. Lada가 기본이며 기존 동작은 그대로입니다.
- **Engine selector: Lada / JASNA** — top of the settings panel. Lada stays the default; nothing changes unless you switch.
- 이 PC(RTX 5090)에서 **같은 파일로 실측**한 결과 / Measured on this machine (RTX 5090), identical files:
  - 1080p 파일당: Lada 92초 → **JASNA 35초 (2.6배)** / per file: 92s → **35s (2.6x)**
  - 동시 3개 실행: 처리량 **2.06배** 추가 / 3 concurrent jobs: **2.06x** more throughput on top
  - 60초 8K VR: **107초** (Lada 경로 대비 대략 18배) / 60s of 8K VR: **107s** (roughly 18x the Lada path)
- JASNA는 VR을 **파이프라인 안에서 눈별로 처리**하므로 좌우 분리·재합성이 없습니다. 재인코딩이 3회에서 1회로 줄어 화질 손실도 줄고, 4K 메모리 때문에 강제됐던 짧은 클립(20) 제한도 사라집니다.
- JASNA handles VR **per-eye inside its own pipeline** — no split/merge. Encode generations drop from 3 to 1, and the short clip window (20) forced by 4K memory limits no longer applies.
- **병렬 처리는 JASNA 자체 GUI에는 없는 기능**입니다. JASNA 하나로는 GPU를 다 쓰지 못하므로, 여러 파일을 동시에 돌리면 처리량이 실제로 늘어납니다.
- **Parallel processing is something JASNA's own GUI cannot do.** A single JASNA process does not saturate the GPU, so running several files at once genuinely raises throughput.

### 📦 JASNA 설치 (직접 해야 합니다) / Installing JASNA (you do this yourself)

이 앱은 JASNA를 **내려받거나 번들하지 않습니다** — 설치된 것을 찾아 실행만 합니다. (AGPL, Lada와 동일)
This app **never downloads or bundles JASNA** — it only detects and runs an existing install. (AGPL, same as Lada.)

1. https://github.com/Kruk2/jasna/releases 에서 Windows(NVIDIA) 패키지의 **모든 파트**(`.7z.001/.002/.003`)를 받아 압축 해제 / download **every part** of the Windows (NVIDIA) package and extract
2. **영문·숫자만 있는 경로**에 설치 — 예: `C:\jasna` (자동 탐색 위치) / install under an **ASCII-only path**, e.g. `C:\jasna` (auto-detected)
3. 첫 실행 시 TensorRT 엔진을 빌드합니다 (이 PC에서 약 3분, 최초 1회) / first run builds TensorRT engines (about 3 minutes here, once)
4. 요구사항: RTX 20 시리즈 이상, Windows 드라이버 610+ / requires RTX 20-series or newer, Windows driver 610+

다른 경로에 설치했다면 설정의 **JASNA 경로**에 `jasna.exe` 위치를 적으세요. / If installed elsewhere, set the `jasna.exe` location in **JASNA Path**.

### 🛠 그 외 / Also

- 크래시나 드라이브 끊김으로 남은 VR 임시 폴더(`lada-vr-tmp-*`, 옛 `.lada_vr_tmp_*`)를 다음 작업 시작 때 **자동으로 정리**하고, 정리 실패는 로그에 남깁니다.
- VR temp folders left behind by a crash or a dropped drive are now **swept automatically** at the next job, and a cleanup failure is logged instead of ignored.
- 업데이트 배너 버튼이 "Lada 업데이트" 버튼의 스타일을 덮어쓰던 문제를 고쳤습니다. / Fixed the update banner restyling the "Update Lada" button.
- JASNA 엔진에서는 **일시정지**를 지원하지 않습니다(네이티브 CUDA 프로세스를 안전하게 멈출 수 없어 버튼을 비활성화). 취소는 정상 동작합니다.
- **Pause is unavailable on JASNA** (a native CUDA process cannot be suspended safely, so the button is disabled). Cancel works.

### ℹ️ 참고 / Notes

- v0.7.0 이상에서는 앱 안의 배너로 이 업데이트를 받을 수 있습니다. / From v0.7.0 this update arrives through the in-app banner.
- 2D 작업에는 Lada 품질이 충분하다면 그대로 두어도 됩니다. 엔진은 언제든 바꿀 수 있습니다. / If Lada's 2D quality suits you, leave it. The engine can be switched at any time.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
