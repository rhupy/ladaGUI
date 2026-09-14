## Lada GUI v0.5.1

v0.5.0의 VR 처리 UX·안정성 개선 릴리스입니다.
UX and reliability fixes for the VR processing added in v0.5.0.

### 🛠 개선 / Improvements

- **VR 분리/합성 단계 진행률 표시 / Progress shown during VR split & merge**
  - v0.5.0에서는 VR 영상의 좌우 분리·합성 중 진행률이 안 나와 **멈춘 것처럼 보였습니다.** 이제 이 단계에도 실시간 진행률이 표시됩니다.
  - In v0.5.0 the split/merge stages showed no progress and **looked frozen.** These stages now report live progress.
  - 카드 진행률 배분: 분리 2–10% → 좌안 복원 10–52% → 우안 복원 52–94% → 합성 94–100%.
  - Progress mapping: split 2–10% → left eye 10–52% → right eye 52–94% → merge 94–100%.
- **VR 중간 파일을 출력 드라이브에 저장 / VR temp files kept on the output drive**
  - 8K VR은 중간 파일이 수십 GB에 달할 수 있어, 시스템 임시폴더(C:) 대신 **출력 폴더 옆**에 임시 작업 폴더를 만들어 처리 후 자동 삭제합니다. C: 용량 부족으로 인한 실패를 방지합니다.
  - 8K VR intermediates can total tens of GB, so they are now written next to the output folder (not system temp / C:) and auto-deleted afterward, avoiding out-of-space failures.

### ℹ️ 참고 / Notes

- VR 처리는 8K 등 고해상도라 **분리 단계만 대략 영상 길이만큼(실시간) 걸린 뒤** 좌/우안 복원이 이어집니다. 전체적으로 시간이 꽤 걸릴 수 있습니다.
- VR processing is high-resolution (8K); the **split stage alone takes roughly the video's own length (~realtime)** before the two eye-restoration passes begin, so the full job can take a while.
- 일반 2D 영상은 영향 없음(2:1 아니면 즉시 기존 경로). VR 자동 감지·처리 방식은 v0.5.0과 동일합니다.
- Normal 2D videos are unaffected. VR auto-detection/handling is unchanged from v0.5.0.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
