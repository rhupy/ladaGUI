## Lada GUI v0.6.0

외장하드처럼 **연결이 끊길 수 있는 드라이브에서도 VR 작업이 안정적으로 완주**하도록 구조를 바꾼 릴리스입니다.
VR jobs now survive a drive that briefly disconnects — the whole point of this release.

### ✨ 핵심 변경 / Key change

- **VR 작업 중 Docker가 원본/출력 드라이브를 아예 건드리지 않습니다**
- **Docker no longer touches the source/output drive during a VR job**
  - 이전에는 몇 시간짜리 VR 작업 내내 해당 드라이브를 Docker에 마운트했습니다. 외장하드가 **한 번만 깜빡여도 그 마운트가 죽고**, 윈도우에서 드라이브가 다시 붙어도 **Docker Desktop을 재시작하기 전까지는 복구되지 않아** 작업 전체가 무산됐습니다(`exit 125`, 무한 재시도).
  - Previously the drive stayed mounted into Docker for the entire multi-hour job. One blink and that mount dies — and it stays dead until Docker Desktop is restarted, even after Windows reattaches the drive. The job was stranded (`exit 125`, retrying forever).
  - 이제 흐름: **원본을 작업 드라이브로 복사 → 분리·좌안·우안·합성을 전부 작업 드라이브에서 처리 → 완성본만 출력 폴더로 복사.**
  - New flow: **copy the source to the work drive → split / left eye / right eye / merge entirely there → copy only the finished file back out.**
  - 두 번의 복사는 Docker가 아니라 **앱이 직접 수행**하므로, 도중에 드라이브가 빠져도 **다시 붙는 즉시 재시도가 성공**합니다. 사용자가 할 일은 없습니다.
  - Those two copies are plain host file I/O (not Docker), so if the drive drops mid-copy it simply **retries and succeeds as soon as it returns** — no manual steps.
  - 외장하드 노출 구간이 **"몇 시간" → "복사 두 번"** 으로 줄어듭니다. 추가 시간은 9GB 원본 기준 약 3분(전체 작업의 1~2%)입니다.
  - Drive exposure shrinks from **hours to two copies**. Overhead is ~3 minutes for a 9 GB source (1–2% of the job).

### 🛠 그 외 / Also

- 복사 구간에도 진행률이 표시됩니다. 진행률 배분: 복사 0–5% → 분리 5–12% → 좌안 12–52% → 우안 52–92% → 합성 92–97% → 결과 복사 97–100%.
- The copy stages report progress too: copy-in 0–5% → split 5–12% → left 12–52% → right 52–92% → merge 92–97% → copy-out 97–100%.
- 중간 파일을 단계마다 즉시 삭제해 작업 드라이브 사용량을 원본의 약 4~5배로 억제합니다. 여유 공간이 부족하면 출력 드라이브로 자동 폴백합니다.
- Intermediates are deleted as soon as they're consumed, capping work-drive use at ~4–5× the source; if space is short it falls back to the output drive.
- 원본을 단순한 이름으로 복사해 처리하므로, 파일명의 공백·괄호·한글이 Docker 인자를 거치며 문제를 일으킬 여지가 없습니다.
- The staged copy uses a plain ASCII name, so spaces/brackets/non-Latin characters in filenames never pass through docker arguments.
- v0.5.1~0.5.3의 개선(분리·합성 진행률, VR OOM 수정, 드라이브 끊김 안내)이 모두 포함됩니다. 일반 2D 영상 처리 경로는 변경되지 않았습니다.
- Includes everything from v0.5.1–0.5.3 (split/merge progress, the VR OOM fix, the drive-disconnect hint). The normal 2D path is unchanged.

---

**테스트 / Testing:** Windows 설치 파일(`.msi` / `.exe`)을 아래 Assets에서 받아 설치 후 사용하세요.
Download the Windows installer (`.msi` / `.exe`) from the Assets below.
