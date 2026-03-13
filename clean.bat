@echo off
chcp 65001 >nul
echo ========================================
echo   清理打包文件
echo ========================================
echo.

if exist "build" (
    echo [清理] build/
    rmdir /s /q build
)

if exist "dist" (
    echo [清理] dist/
    rmdir /s /q dist
)

if exist "__pycache__" (
    echo [清理] __pycache__/
    rmdir /s /q __pycache__
)

if exist "*.spec" (
    echo [清理] *.spec
    del /q *.spec
)

echo.
echo [完成] 清理完成！
pause