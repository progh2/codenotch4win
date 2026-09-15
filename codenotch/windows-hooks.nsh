; A running copy keeps its old build alive after an upgrade: the single-instance
; guard turns the freshly installed build away and the stale process stays on
; screen. Close it before files are copied, and NSIS can also overwrite the exe.
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'taskkill /F /IM Codenotch.exe'
  nsExec::Exec 'taskkill /F /IM codenotch.exe'
  Sleep 400
!macroend
