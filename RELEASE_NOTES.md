## Lada GUI v0.5.3

VR 중간 파일을 **빠르고 안전한 드라이브에 저장**하도록 개선한 릴리스입니다.
VR intermediates are now placed on a fast, safe drive.

### 🛠 개선 / Improvements

- **VR 임시 파일 위치 자동 선택 / Smarter VR temp-file location**
  - v0.5.1/0.5.2는 VR 중간 파일(수십 GB)을 무조건 **출력 드라이브**에 썼습니다. 출력이 외장 HDD면 **느리고**, 장시간 대용량 쓰기 중 **연결이 끊기면 작업 전체가 실패**했습니다.
  - v0.5.1/0.5.2 always wrote the VR intermediates (tens of GB) to the **output drive**. If that's an external HDD it is **slow**, and a disconnect during the long write **kills the whole job**.
  - 이제 **시스템 임시 폴더(보통 내장 SSD)의 여유 공간을 먼저 확인해**, 충분하면 그쪽을 쓰고 부족할 때만 출력 드라이브로 넘어갑니다. (필요 공간은 원본 크기의 약 5배로 계산)
  - Now it **checks free space on the system temp dir (usually a fast internal SSD)** and uses it when there's room, falling back to the output drive only if not. (Budget: ~5× the source file size.)
- **드라이브 연결 끊김 오류 안내 / Clearer message when a drive disappears**
  - Docker 마운트 실패(exit 125) 시 **"드라이브 연결 확인 후 Docker Desktop 재시작"** 안내가 함께 표시됩니다. 원인 모를 재시도 반복을 줄여줍니다.
  - A Docker mount failure (exit 125) now shows a **"drive unavailable — check the drive, then restart Docker Desktop"** hint instead of silently retrying.

### ℹ️ 참고 / Notes

- **VR 작업은 원본을 내장 SSD에 두고 돌리는 것을 권장합니다.** 8K VR은 몇 시간짜리 작업이라 외장 USB HDD는 도중에 끊길 위험이 큽니다.
- **For VR jobs, keep the source on an internal SSD.** 8K VR runs for hours; external USB drives are prone to dropping mid-job.
- v0.5.2의 OOM 수정(VR clip 길이 20 + 메모리 제한 해제)과 v0.5.1의 진행률 표시가 모두 포함됩니다.
- Includes the v0.5.2 OOM fix (VR clip length 20 + no memory cap) and the v0.5.1 progress display.
- VR 자동 감지 방식은 v0.5.0과 동일하며, 일반 2D 영상은 영향 없습니다.
- VR auto-detection is unchanged from v0.5.0; normal 2D videos are unaffected.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
