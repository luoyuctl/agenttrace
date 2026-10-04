@echo off
rem agenttrace Windows installer for CMD. Runs install.ps1 with the same options.
rem Usage: curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.cmd -o install.cmd && install.cmd && del install.cmd
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -Command "& ([scriptblock]::Create((Invoke-RestMethod -UseBasicParsing 'https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1'))) %*"
exit /b %ERRORLEVEL%
