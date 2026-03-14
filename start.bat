@echo off
:loop
.\maimai-touch-rs.exe
if %errorlevel% equ 42 goto loop
