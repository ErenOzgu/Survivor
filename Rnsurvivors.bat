@echo off
rem Rnsurvivors: asks for the WoW Data folder once, then starts the game.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0launcher\Rnsurvivors.ps1" %*
if errorlevel 1 pause
