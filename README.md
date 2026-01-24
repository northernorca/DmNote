<meta name="google-site-verification" content="tw5pjIDYKCrq1QKYBrD5iyV7DXIM4rsHN9d11WlJFe4" />

**한국어** | [English](docs/readme_en.md)

<div align="center">
  <img src="src-tauri/icons/icon.ico" alt="dmnote Logo" width="120" height="120">

  <h1>DM Note</h1>
  
  <p>
    <strong>다양한 커스터마이징을 지원하는 키뷰어 프로그램</strong>
  </p>
  <p>
    <strong>사용자 정의 키 매핑과 스타일링, 손쉽게 전환 가능한 프리셋, 모던하고 직관적인 인터페이스를 제공합니다.</strong>
  </p>
  
  [![GitHub release](https://img.shields.io/github/release/lee-sihun/DmNote.svg?logo=github)](https://github.com/lee-sihun/DmNote/releases)
  [![GitHub downloads](https://img.shields.io/github/downloads/lee-sihun/DmNote/total.svg?logo=github)](https://github.com/lee-sihun/DmNote/releases/download/1.5.0/DM.NOTE.v.1.5.0.zip)
  [![GitHub license](https://img.shields.io/github/license/lee-sihun/DmNote.svg?logo=github)](https://github.com/lee-sihun/DmNote/blob/master/LICENSE)
</div>

https://github.com/user-attachments/assets/20fb118d-3982-4925-9004-9ce0936590c2

## 🌟 개요

**DM Note**는 DJMAX RESPECT V에서 사용하기 위해 만들어진 키뷰어 프로그램입니다. Tauri와 React로 구축 되었으며 간편한 설정으로 스트리밍이나 플레이 영상 제작 시 키 입력을 시각적으로 보여줄 수 있습니다. 현재는 Windows/macOS 환경을 지원하며, 이외의 다른 게임에서도 사용이 가능합니다.

본 레포지토리는 DM Note를 **Linux**에도 사용할 수 있도록 구현하는 프로젝트입니다.

[DM NOTE for Linux 다운로드](https://github.com/northernorca/DmNote/releases)

## 설치 및 실행

### Ubuntu, Debian 등 APT 기반 배포판

Release 탭에서 `.deb` 파일을 다운받아 실행합니다.

### Fedora, RHEL 등 RPM 기반 배포판

Release 탭에서 `.rpm` 파일을 다운받아 실행합니다.

### Arch, Cachy, Endeavour 등 pacman 기반 배포판

DM NOTE for Linux는 [AUR에서 배포중](https://aur.archlinux.org/packages/dm-note-bin)입니다. AUR의 사용법은 ArchWiki의 [Arch User Repository](https://wiki.archlinux.org/title/Arch_User_Repository) 페이지를 참조하세요.

```bash
sudo pacman -U <filename>.pkg.tar.zst
```

### 그 외

추후에 Appimage로도 번들링하여 distro-agnostic하게 실행 가능하게 할 계획입니다.

### 직접 빌드 및 실행

```bash
git clone https://github.com/northernorca/DmNote.git --depth 1
cd DmNote
npm install
npm run tauri:dev
```

Nvidia 그래픽 카드를 사용중이라면 마지막 줄을 아래와 같이 바꾸어 explicit sync를 해제해야 합니다.

```bash
__NV_DISABLE_EXPLICIT_SYNC=1 npm run tauri:dev
```
>>>>>>> 3d08f2b (chore: add linux packaging and versioning)

## ✨ 주요 기능

### ⌨️ 키보드 입력 및 매핑

- 실시간 키보드 입력 감지 및 시각화
- 커스텀 키 매핑 설정

### 🎨 키 스타일 커스터마이징

- 그리드 기반 키 편집 
- 이미지 할당 지원
- 커스텀 CSS 지원

### 💾 프리셋 및 설정 관리

- 사용자 설정 자동 저장
- 프리셋 저장/불러오기

### 🖼️ 오버레이 및 창 관리

- 창 위치 고정
- 항상 위에 표시
- 리사이즈 기준점 선택

### 🌧️ 노트 효과 (Raining Effect) 커스터마이징

- 노트 효과 색상, 투명도, 라운딩, 속도, 높이 조절
- 리버스 기능

### 🔢 키 카운터

- 키별 입력 횟수 실시간 표시
- 카운터 위치, 색상 및 스타일 커스터마이징
- 커스텀 CSS 지원

### ⚙️ 그래픽 및 설정

- 다국어 지원 (한글, 영어)
- 그래픽 렌더링 옵션 (Direct3D 11/9, OpenGL)
- 설정 초기화

## 🚀 개발

### 기술 스택

- **프론트엔드**: React 19 + Typescript + Vite 7
- **백엔드**: Tauri
- **스타일링**: Tailwind CSS 3
- **입력 감지**: Raw Input API (Windows), 전역 입력 이벤트 (macOS) 
- **패키지 매니저**: npm

### 폴더 구조

```
DmNote/
├─ src/                          # 프론트엔드
│  ├─ renderer/                  # React 렌더러
│  │  ├─ components/             # UI 컴포넌트
│  │  ├─ hooks/                  # 상태/동기화 훅
│  │  ├─ stores/                 # Zustand 스토어
│  │  ├─ windows/                # 렌더러 윈도우 (main/overlay)
│  │  ├─ styles/                 # 전역/공통 스타일
│  │  └─ assets/                 # 정적 리소스
│  └─ types/                     # 공유 타입/스키마
├─ src-tauri/                    # Tauri 백엔드
│  └─ src/                       # 커맨드, 서비스
├─ package.json                  # 프로젝트 의존성 및 실행 스크립트
├─ tsconfig.json                 # TypeScript 설정
└─ vite.config.ts                # Vite 설정
```

### 빌드 및 실행

터미널에서 다음 명령어를 순서대로 입력하세요.

```bash
git clone https://github.com/lee-sihun/DmNote.git
cd DmNote
npm install
npm run tauri:dev
```

Linux의 Nvidia, Wayland 환경에서는 마지막 라인을 다음으로 고쳐서 explicit sync를 비활성화 시켜주세요.

```bash
__NV_DISABLE_EXPLICIT_SYNC=1 npm run tauri:dev
```

### Linux 번들링

`src-tauri/tauri.linux.conf.json`의 버전을 알맞게 수정하고 아래 스크립트를 실행합니다.

```bash
./scripts/bundle/bundle.sh
```

그러면 `scripts/bundle/{version}/` 디렉토리 하에 번들링된 패키지가 놓입니다.

## 🖼️ 스크린샷

<!--img src="docs/assets/2025-08-29_12-07-12.webp" alt="Note Effect" width="700"-->

<img src="docs/assets/IMG_1005.gif" alt="Note Effect" width="700">

<!--img src="docs/assets/1.webp" alt="키뷰어 데모 1" width="700"-->

<img src="docs/assets/2025-09-20_11-55-17.gif" alt="키뷰어 데모 2" width="700">

<!--img src="docs/assets/IMG_1008.gif" alt="키뷰어 데모 3" width="700"-->

<img src="docs/assets/2025-09-20_11-57-38.gif" alt="키뷰어 데모 4" width="700">

## 📝 참고사항

- 일부 게임의 전체화면 모드에서는 정상 동작하지 않습니다. 이 경우 테두리 없는 창 모드를 사용해주세요.
- 그래픽 문제 발생 시 설정에서 렌더링 옵션을 변경해주세요.
- OBS 윈도우 캡쳐로 크로마키 없이 배경을 투명하게 불러올 수 있습니다.
- 게임 화면 위에 표시할 경우, **항상 위에 표시**로 배치한 뒤 **오버레이 창 고정**을 활성화해주세요.
- 커스텀 CSS 예제 파일은 `assets` 폴더에 있습니다.
- 클래스명 할당 시 선택자는 제외하고 이름만 입력해주세요.(`blue` -> o, `.blue` -> x)
- 프로그램 기본 설정은 `%appdata%/com.dmnote.desktop` 폴더의 `store.json`에 저장됩니다.

## 🤝 기여하기

여러분의 참여를 환영합니다! 자세한 내용은 [기여 가이드](CONTRIBUTING.md)를 확인해주세요.

## 📄 라이선스

[GPL-3.0 License Copyright (C) 2024 lee-sihun](https://github.com/lee-sihun/DmNote/blob/master/LICENSE)

## ❤️ Special Thanks!

- [tauri-apps/tauri](https://github.com/tauri-apps/tauri)

<!--
## 🔜 업데이트 예정

- 키 입력 카운트, 입력 속도 시각화
- 동시 입력 간격 밀리초(ms) 표시
- 입력 통계 분석 기능
 -->




