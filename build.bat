@echo off
chcp 65001 >nul
echo ========================================
echo   PDF Manager 打包脚本
echo ========================================
echo.

:: 检查Python
python --version >nul 2>&1
if errorlevel 1 (
    echo [错误] 未找到 Python，请先安装 Python 3.10+
    pause
    exit /b 1
)

:: 检查虚拟环境
if not exist "venv\Scripts\activate.bat" (
    echo [信息] 创建虚拟环境...
    python -m venv venv
)

:: 激活虚拟环境
echo [信息] 激活虚拟环境...
call venv\Scripts\activate.bat

:: 安装依赖
echo [信息] 安装依赖...
pip install -r requirements.txt -i https://mirror.baidu.com/pypi/simple
pip install pyinstaller -i https://mirror.baidu.com/pypi/simple

:: 清理旧的打包文件
echo [信息] 清理旧的打包文件...
if exist "build" rmdir /s /q build
if exist "dist" rmdir /s /q dist

:: 打包
echo [信息] 开始打包（这可能需要几分钟）...
pyinstaller build.spec --clean

:: 检查打包结果
if exist "dist\PDF Manager.exe" (
    echo.
    echo ========================================
    echo   打包成功！
    echo ========================================
    echo.
    echo   输出文件: dist\PDF Manager.exe
    echo   文件大小:
    for %%I in ("dist\PDF Manager.exe") do echo   %%~zI 字节 ^(约 %%~nI%%~xI^)
    echo.
    echo   使用方法:
    echo   1. 将 "PDF Manager.exe" 复制到任意位置
    echo   2. 双击运行即可
    echo   3. 数据文件会在程序同目录下的 data\ 文件夹
    echo.
) else (
    echo.
    echo ========================================
    echo   打包失败！
    echo ========================================
    echo   请检查错误信息
)

pause