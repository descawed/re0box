@echo off
setlocal EnableDelayedExpansion

echo ========================================
echo re0box uninstaller
echo ========================================
echo.
echo This will delete the following files and folders:
echo - scripts\re0box.asi
echo - dinput8.dll (only if scripts folder contains no other ASI files)
echo - re0box.ini
echo - re0box.log
echo - re0box_readme.txt
echo - nativePC\arc\message\msg_chS_box.arc
echo - nativePC\arc\message\msg_chT_box.arc
echo - nativePC\arc\message\msg_eng_box.arc
echo - nativePC\arc\message\msg_fre_box.arc
echo - nativePC\arc\message\msg_ger_box.arc
echo - nativePC\arc\message\msg_ita_box.arc
echo - nativePC\arc\message\msg_jpn_box.arc
echo - nativePC\arc\message\msg_spa_box.arc
echo.
echo WARNING: If you have saved with the mod installed, uninstalling
echo will cause all item boxes to be deleted from all saves.
echo.
choice /C YN /N /M "Are you sure you want to uninstall? (Y/N): "
if errorlevel 2 (
    echo Uninstall cancelled.
    pause
    exit /b 0
)

echo.
echo Starting uninstall...
echo.

REM Delete the main ASI file
if exist "scripts\re0box.asi" (
    del "scripts\re0box.asi"
    if !errorlevel! equ 0 (
        echo Deleted: scripts\re0box.asi
    ) else (
        echo ERROR: Failed to delete scripts\re0box.asi
    )
) else (
    echo Not found: scripts\re0box.asi
)

REM Check if dinput8.dll should be deleted
set "delete_dinput8=1"
if exist "scripts\" (
    for /f "delims=" %%f in ('dir /b /a-d "scripts\*.asi" 2^>nul') do (
        echo Found other ASI file: scripts\%%f
        set "delete_dinput8=0"
    )
)

REM Check if scripts folder is empty and delete it if so
if exist "scripts\" (
    dir /b "scripts\" | findstr "^" >nul
    if !errorlevel! neq 0 (
        rmdir "scripts"
        if exist "scripts\" (
            echo ERROR: Failed to delete scripts folder
        ) else (
            echo Deleted empty folder: scripts
        )
    ) else (
        echo Keeping scripts folder ^(not empty^)
    )
)

if "%delete_dinput8%"=="1" (
    if exist "dinput8.dll" (
        del "dinput8.dll"
        if !errorlevel! equ 0 (
            echo Deleted: dinput8.dll
        ) else (
            echo ERROR: Failed to delete dinput8.dll
        )
    ) else (
        echo Not found: dinput8.dll
    )
) else (
    echo Skipping dinput8.dll ^(other ASI files detected in scripts folder^)
)

REM Delete config and log files
if exist "re0box.ini" (
    del "re0box.ini"
    if !errorlevel! equ 0 (
        echo Deleted: re0box.ini
    ) else (
        echo ERROR: Failed to delete re0box.ini
    )
) else (
    echo Not found: re0box.ini
)

if exist "re0box.log" (
    del "re0box.log"
    if !errorlevel! equ 0 (
        echo Deleted: re0box.log
    ) else (
        echo ERROR: Failed to delete re0box.log
    )
) else (
    echo Not found: re0box.log
)

if exist "re0box_readme.txt" (
    del "re0box_readme.txt"
    if !errorlevel! equ 0 (
        echo Deleted: re0box_readme.txt
    ) else (
        echo ERROR: Failed to delete re0box_readme.txt
    )
) else (
    echo Not found: re0box_readme.txt
)

REM Delete message arc files
set "msg_files=msg_chS_box.arc msg_chT_box.arc msg_eng_box.arc msg_fre_box.arc msg_ger_box.arc msg_ita_box.arc msg_jpn_box.arc msg_spa_box.arc"
for %%m in (%msg_files%) do (
    if exist "nativePC\arc\message\%%m" (
        del "nativePC\arc\message\%%m"
        if !errorlevel! equ 0 (
            echo Deleted: nativePC\arc\message\%%m
        ) else (
            echo ERROR: Failed to delete nativePC\arc\message\%%m
        )
    ) else (
        echo Not found: nativePC\arc\message\%%m
    )
)

echo.
echo Uninstall complete!
echo This uninstaller will now delete itself.
echo.
pause

REM Self-delete using a temporary batch file
(goto) 2>nul & del "%~f0"
