## Lada GUI v0.8.2

VR 조합에서 **불필요했던 탐지 임계값 완화를 제거**했습니다. / Drops the detection-threshold change from the VR combination — it was not needed.

### 🛠 변경 / Change

- v0.8.1은 VR에 `rfdetr-vr-v1` + 어안 투영 + 탐지 임계값 0.15를 함께 적용했습니다. 이후 요소별로 확인한 결과
  **어안 투영만으로 모자이크가 제거**되고, 임계값만 낮춘 경우는 그대로 남았습니다. 임계값은 기본값으로 되돌려
  오탐 여지를 없앴습니다.
- v0.8.1 applied `rfdetr-vr-v1` + fisheye projection + a 0.15 detection threshold to VR. Checked one lever at a time,
  **fisheye projection alone removed the mosaic** while the lowered threshold alone did not, so the threshold is back
  at JASNA's default and the false-positive risk that came with lowering it is gone.
- 원인 정리: JASNA 자동 VR 모드는 미등록 스튜디오를 `projection=raw`로 처리하는데, raw 프레임에선 탐지기가 모자이크를
  보지 못합니다. 어안으로 펴면 봅니다. / Root cause: JASNA's auto VR mode resolves an unregistered studio to
  `projection=raw`, and on raw frames the detector cannot see the mosaic; fisheye projection lets it.

### ℹ️ 참고 / Notes

- VR 판별과 적용은 자동입니다(로그 `JASNA-PROBE ... VR(SBS)`). / VR detection and the flags are automatic (log: `JASNA-PROBE ... VR(SBS)`).
- v0.7.0 이상은 앱 안의 배너로 받습니다. / From v0.7.0 this arrives through the in-app banner.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
