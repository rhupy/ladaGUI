## Lada GUI v0.8.1

**JASNA 엔진에서 VR 모자이크가 실제로 제거됩니다.**
**VR mosaics are now actually removed on the JASNA engine.**

### 🐞 수정 / Fix

- v0.8.0에서 JASNA로 VR 파일을 돌리면 모자이크가 그대로 남았습니다. 원인은 JASNA의 자동 VR 모드가 눈 분리는
  하지만 **범용 탐지기(`rfdetr-v6`)와 raw 투영, 기본 임계값**을 그대로 써서, 어안 프레임의 모자이크를 하나도
  찾지 못했기 때문입니다. VR 전용 탐지기만 바꿔도, 어안 투영만 켜도, 임계값만 낮춰도 안 됐고 — **세 가지를
  함께** 적용했을 때 육안으로 모자이크가 확실히 사라졌습니다.
- On v0.8.0, VR files through JASNA came out with the mosaic intact. JASNA's auto VR mode splits the eyes but
  keeps the **generic detector (`rfdetr-v6`), raw projection and default threshold**, which found nothing on
  fisheye frames. The VR detector alone, fisheye alone and a lower threshold alone all failed — **all three
  together** removed the mosaic, confirmed by eye.
- 앱이 이제 VR(2:1, 1080p 초과 — JASNA와 같은 규칙)을 JASNA 번들 `ffprobe`로 판별해 **자동으로** 그 조합을
  적용합니다: `rfdetr-vr-v1` + `sbs-fisheye` + 탐지 임계값 0.15. 설정할 것 없음. 2D 파일은 그대로(임계값을
  낮추면 오탐 위험이 있어 VR에만 적용).
- The app now identifies VR (2:1 and taller than 1080p — JASNA's own rule) using the `ffprobe` JASNA ships,
  and **automatically** applies that combination: `rfdetr-vr-v1` + `sbs-fisheye` + detection threshold 0.15.
  Nothing to configure. 2D files are unchanged — the lower threshold could cause false positives there.
- 같은 60초 8K 클립 기준 처리 시간 111초(복원 포함). / 111 s for the same 60 s 8K clip, restoration included.

### ℹ️ 참고 / Notes

- 로그(`lada-gui.log`)에 `JASNA-PROBE ... VR(SBS)` 로 판별 결과가 남습니다. / The probe result is logged as `JASNA-PROBE ... VR(SBS)`.
- v0.7.0 이상에서는 앱 안의 배너로 이 업데이트를 받을 수 있습니다. / From v0.7.0 this arrives through the in-app banner.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
