; Akkhor PDF Editor - OCR language pack (Bangla + English)
;
; A separate, offline installer for the Tesseract language data (SRS
; FR-10.3). The app never downloads anything: users who want OCR install
; this pack, published on GitHub Releases.
;
; Installs for the current user only (no admin rights) into
;   %LOCALAPPDATA%\Akkhor PDF Editor OCR\tessdata
; which is where the app looks for it.
;
; Build with scripts\build-ocr-pack.ps1 (it puts the data in ocr-pack\data).

Unicode true
!define PACK_NAME "Akkhor PDF Editor OCR Pack"
!ifndef PACK_VERSION
  !define PACK_VERSION "1.0.0"
!endif
!define PACK_DIR "Akkhor PDF Editor OCR"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\AkkhorOcrPack"

Name "${PACK_NAME} ${PACK_VERSION}"
OutFile "dist\Akkhor-OCR-Pack-ben-eng-${PACK_VERSION}.exe"
InstallDir "$LOCALAPPDATA\${PACK_DIR}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUnInstDetails show

!include "MUI2.nsh"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Akkhor PDF Editor - OCR (Bangla + English)"
!define MUI_WELCOMEPAGE_TEXT "This adds text recognition (OCR) to Akkhor PDF Editor for scanned pages, in Bangla and English.$\r$\n$\r$\nIt installs only language data (about 26 MB) for your Windows account. Nothing is downloaded and nothing is sent anywhere.$\r$\n$\r$\nএটি অক্ষর PDF Editor-এ স্ক্যান করা পাতার লেখা চেনার (OCR) সুবিধা যোগ করে — বাংলা ও ইংরেজি।"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "data\LICENSE-tessdata.txt"
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "OCR is ready. Restart Akkhor PDF Editor if it is open, then use More > Make searchable (OCR).$\r$\n$\r$\nOCR প্রস্তুত। অক্ষর খোলা থাকলে বন্ধ করে আবার খুলুন।"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Language data" SecData
  SectionIn RO
  SetOutPath "$INSTDIR\tessdata"
  File "data\ben.traineddata"
  File "data\eng.traineddata"
  SetOutPath "$INSTDIR"
  File "data\LICENSE-tessdata.txt"
  File "README.txt"
  WriteUninstaller "$INSTDIR\Uninstall OCR Pack.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PACK_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${PACK_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Akkhor PDF Editor contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall OCR Pack.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" 26000
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\tessdata\ben.traineddata"
  Delete "$INSTDIR\tessdata\eng.traineddata"
  RMDir "$INSTDIR\tessdata"
  Delete "$INSTDIR\LICENSE-tessdata.txt"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\Uninstall OCR Pack.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
