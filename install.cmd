@echo off
rem agenttrace Windows installer for CMD. Downloads install.ps1 and runs it with -File so
rem arguments such as -Version v0.9.1 reach the script as parameters, not as code.
rem Usage: curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.cmd -o install.cmd && install.cmd && del install.cmd
setlocal
set "AGENTTRACE_PS1=%TEMP%\agenttrace-install-%RANDOM%%RANDOM%.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -UseBasicParsing -Uri 'https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1' -OutFile $env:AGENTTRACE_PS1"
if errorlevel 1 (
  echo ERROR: Could not download install.ps1
  exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%AGENTTRACE_PS1%" %*
set "AGENTTRACE_STATUS=%ERRORLEVEL%"
del "%AGENTTRACE_PS1%" >nul 2>&1
exit /b %AGENTTRACE_STATUS%
