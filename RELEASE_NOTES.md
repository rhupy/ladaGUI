## Lada GUI v0.4.0

모자이크 복원 품질을 직접 제어할 수 있는 옵션과 UI 개선이 추가된 릴리스입니다.
This release adds direct control over restoration quality plus several UI fixes.

### ✨ 새로운 기능 / New features

- **복원 모델 선택 (Restoration model selector)**
  - 모자이크를 실제로 복원하는 모델을 직접 고를 수 있습니다: `basicvsrpp-v1.2`(최신·권장) / `v1.1` / `v1.0`.
  - Pick the model that actually restores the mosaic — the core of output quality.
- **FP16 (반정밀도) 옵션 / FP16 (half-precision) option**
  - `자동 / 켜기 / 끄기` 선택. **자동**은 GPU 지원 여부를 Lada가 판단합니다(권장). VRAM 절약과 최신 GPU에서의 속도 향상에 도움이 됩니다.
  - `Auto / On / Off`. **Auto** lets Lada decide based on your GPU (recommended). Saves VRAM and can speed up modern GPUs with negligible quality difference.

### 🛠 개선 / Improvements

- **타이틀에 버전 표시 / Version shown in title**
  - 헤더와 OS 창 제목에 현재 버전(`v0.4.0`)이 표시됩니다.
  - The header and OS window title now show the current version.
- **진행률 하단 레이아웃 정리 / Progress footer layout fix**
  - 각 작업 카드 하단을 한 줄로 고정: 좌측 `처리량(processed)`, 우측 `남은시간 · 속도`. 텍스트 유무와 상관없이 카드 크기가 변하지 않습니다.
  - Each job card's footer is now a single fixed line: processed on the left, remaining time · speed on the right. Card height no longer shifts.
  - 속도/남은시간이 작업별로 정확히 표시됩니다(이전에는 전역값 공유 문제가 있었습니다).
  - Speed/ETA are now per-file (previously shared a global value).

### ℹ️ 참고 / Notes

- 원천 기술(Lada)의 복원 모델은 현재 `basicvsrpp-v1.2`가 최신이며, 이 버전에 이미 포함되어 있습니다.
- The underlying Lada restoration model is currently `basicvsrpp-v1.2`, already bundled here.
- 복원 모델 드롭다운의 항목은 Docker 이미지에 포함된 모델에 한해 동작합니다.
- Restoration-model options only work for models bundled in the Docker image.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
