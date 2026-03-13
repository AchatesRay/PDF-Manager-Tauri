@echo off
chcp 65001 >nul
echo ========================================
echo   PDF Manager 快速打包（目录模式）
echo ========================================
echo.
echo 此脚本会生成一个包含exe和依赖文件的目录
echo 启动速度比单文件模式更快
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
if exist "dist\PDF Manager" rmdir /s /q "dist\PDF Manager"

:: 打包（目录模式）
echo [信息] 开始打包...
pyinstaller ^
    --name "PDF Manager" ^
    --noconsole ^
    --onedir ^
    --clean ^
    --noconfirm ^
    --hidden-import paddle ^
    --hidden-import paddleocr ^
    --hidden-import fitz ^
    --hidden-import whoosh ^
    --hidden-import jieba ^
    --hidden-import PIL ^
    --hidden-import numpy ^
    --exclude-module tkinter ^
    --exclude-module matplotlib ^
    --exclude-module scipy ^
    --exclude-module pandas ^
    src/main.py

:: 检查打包结果
if exist "dist\PDF Manager\PDF Manager.exe" (
    echo.
    echo ========================================
    echo   打包成功！
    echo ========================================
    echo.
    echo   输出目录: dist\PDF Manager\
    echo   主程序:   dist\PDF Manager\PDF Manager.exe
    echo.
    echo   使用方法:
    echo   1. 将整个 "dist\PDF Manager" 文件夹复制到目标位置
    echo   2. 双击 "PDF Manager.exe" 运行
    echo.
    explorer "dist\PDF Manager"
) else (
    echo.
    echo [错误] 打包失败，请检查错误信息
)

pause