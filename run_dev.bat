@echo off
cd /d "%~dp0"
call cargo_env.bat run --features desktop,discord %*
