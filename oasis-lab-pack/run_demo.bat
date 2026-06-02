@echo off
REM OASIS Grid Demo launcher (Windows) — runs v2 then v3 in sequence.
REM v2 = each layer in isolation (8 scenarios)
REM v3 = layers coordinated (4 chained scenarios)

cd /d "%~dp0"
echo.
echo OASIS Grid Demo launcher (Windows)
echo.

echo Running Demo v2 (5 layers in isolation, 8 scenarios)
echo -----------------------------------------------------------------
binaries\oasis_grid_demo.exe
set EX2=%ERRORLEVEL%
echo -----------------------------------------------------------------
echo v2 exit: %EX2%
echo.

echo Running Demo v3 (chained interactions, 4 scenarios)
echo -----------------------------------------------------------------
binaries\oasis_grid_demo_v3.exe
set EX3=%ERRORLEVEL%
echo -----------------------------------------------------------------
echo v3 exit: %EX3%
echo.

echo Running Demo v4 (all 11 mechanisms, PROVEN/EXP tagged)
echo -----------------------------------------------------------------
binaries\oasis_grid_demo_v4.exe
set EX4=%ERRORLEVEL%
echo -----------------------------------------------------------------
echo v4 exit: %EX4%
echo.

if "%EX2%"=="0" if "%EX3%"=="0" if "%EX4%"=="0" (
    echo [DONE] All 3 demos PASSED. See docs\ for level-2 evaluation.
    exit /b 0
)
echo [FAIL] v2=%EX2% v3=%EX3% v4=%EX4% — one or more unexpected.
echo Send this output to souhaybrharrab@gmail.com.
exit /b 1
