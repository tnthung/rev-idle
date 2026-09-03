@echo off
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0generate-state-reference.ps1" %*
exit /b %ERRORLEVEL%
