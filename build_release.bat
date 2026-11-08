@echo off
cd /d "%~dp0"
call cargo_env.bat build --release --features desktop,discord %*
