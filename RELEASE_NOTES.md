## Lada GUI v0.5.0

좌우 분할(SBS) VR 영상을 **자동으로 감지해 처리**하는 기능이 추가되었습니다.
This release adds **automatic detection and processing of side-by-side (SBS) VR videos**.

### ✨ 새로운 기능 / New feature

- **VR(SBS) 영상 자동 지원 / Automatic VR (SBS) support**
  - 지금까지는 좌우로 나뉜 VR 영상을 그대로 넣으면 탐지기가 "찌그러진 두 화면"을 보게 돼 결과물이 나빴습니다. 이제 **옵션 설정 없이** VR 영상을 자동으로 알아서 처리합니다.
  - Previously, feeding a split VR video confused the detector (it saw one squished double-image) and results were poor. Now VR videos are handled automatically — **no option to toggle, nothing to configure.**
  - 처리 방식: 화면을 좌/우 눈으로 **분리 → 각 눈을 개별 복원(2패스) → 다시 합성**하고 원본 오디오를 유지합니다. 각 눈이 정상 비율의 화면으로 들어가므로 탐지 정확도가 크게 올라갑니다.
  - How it works: the frame is **split into left/right eyes → each eye is restored independently → recombined**, with the original audio preserved. Each eye enters as a correctly-proportioned image, so detection accuracy improves significantly.

### 🔍 자동 감지 방식 / How detection works

- 영상 비율이 **2:1**이고, 좌/우 절반이 서로 **스테레오 쌍**(유사도 검사)일 때만 VR로 판정합니다.
- A video is treated as VR only if its aspect ratio is **2:1** *and* the left/right halves are a **stereo pair** (similarity check).
- **일반 2D 영상은 영향 없음** — 2:1이 아니면 즉시 기존 2D 경로로 처리되어 속도 변화가 없습니다.
- **Normal 2D videos are unaffected** — anything not 2:1 skips straight to the existing 2D path with no speed change.

### ℹ️ 참고 / Notes

- VR 영상은 8K 등 고해상도가 많아 2패스 처리로 시간이 더 걸릴 수 있습니다(각 눈은 절반 픽셀이라 총량은 원본 1회와 비슷).
- VR videos are often high-resolution (8K), so two-pass processing can take longer (each eye is half the pixels, so total work is similar to one full-frame pass).
- 어안(fisheye) 왜곡 자체는 보정하지 않으므로 화면 가장자리는 여전히 한계가 있을 수 있습니다. 좌/우안을 독립 복원하므로 미세한 스테레오 차이가 생길 수 있습니다.
- Fisheye distortion itself is not corrected, so frame edges may still be imperfect. Eyes are restored independently, so minor stereo differences can occur.
- 모든 VR 처리는 기존과 동일하게 Docker 이미지(`ladaapp/lada:latest`) 안에서 실행됩니다 — 별도 설치가 필요 없습니다.
- All VR processing runs inside the existing Docker image (`ladaapp/lada:latest`) — no extra installation required.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
