## Lada GUI v0.5.2

VR 처리가 5%에서 멈춘 채 무한 반복되던 문제를 해결한 **중요 수정** 릴리스입니다.
Important fix: VR jobs that got stuck looping at 5% now complete.

### 🐞 버그 수정 / Bug fix

- **VR 복원 OOM 무한 재시도 해결 / Fixed VR restoration OOM crash-loop**
  - v0.5.0/0.5.1에서 VR의 좌안 복원이 시작되자마자 컨테이너가 **메모리 부족(OOM)으로 강제 종료(exit 137)** 되고, 30초마다 재시도만 반복해 **5%에서 영원히 멈춰** 있었습니다.
  - In v0.5.0/0.5.1 the left-eye restoration container was **killed for out-of-memory (exit 137)** the moment it started, and the app just retried every 30s — stuck at 5% forever.
  - 원인: 분할된 각 눈이 **4K(4096×4096)** 라, 일반 2D용 설정(clip 길이 최대 180프레임 + 메모리 제한 10GB)으로는 메모리가 폭증해 죽었습니다.
  - Cause: each split eye is **4K (4096×4096)**, so the normal 2D settings (clip length up to 180 + 10 GB memory cap) blew past memory and died.
  - 수정: **VR 패스는 clip 길이를 20으로 낮추고 메모리 제한을 해제**합니다. 실측 결과 4K 복원이 약 7GB만 쓰고 정상 완료됩니다.
  - Fix: **VR passes now use a clip length of 20 and run without the memory cap.** Measured: 4K restoration completes using only ~7 GB.

### ℹ️ 참고 / Notes

- clip 길이를 20으로 낮춘 만큼 시간적 안정성이 약간 줄 수 있습니다(미세한 깜빡임 가능). 4K 메모리 한계상 불가피한 절충이며, 결과에 깜빡임이 보이면 알려주세요.
- The lower clip length (20) slightly reduces temporal stability (possible faint flicker). It's a necessary trade-off for 4K memory limits — let me know if you see flicker.
- v0.5.1의 개선(분리/합성 진행률 표시, 중간 파일을 출력 드라이브에 저장)도 포함됩니다.
- Includes the v0.5.1 improvements (progress shown during split/merge, VR temp files on the output drive).
- VR 자동 감지 방식은 v0.5.0과 동일하며, 일반 2D 영상은 영향 없습니다.
- VR auto-detection is unchanged from v0.5.0; normal 2D videos are unaffected.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
