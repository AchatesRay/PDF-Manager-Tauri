# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller 打包配置文件"""

import sys
from pathlib import Path

block_cipher = None

# 项目根目录
project_root = Path(SPECPATH)

a = Analysis(
    ['src/main.py'],
    pathex=[str(project_root)],
    binaries=[],
    datas=[
        # 如果有资源文件，在这里添加
        # ('assets/icons', 'assets/icons'),
    ],
    hiddenimports=[
        # PyQt6 相关
        'PyQt6',
        'PyQt6.QtCore',
        'PyQt6.QtGui',
        'PyQt6.QtWidgets',
        'PyQt6.sip',

        # PaddleOCR 相关
        'paddle',
        'paddleocr',
        'paddle.dataset',
        'paddle.dataset.image',
        'paddle.fluid',
        'paddle.fluid.core',
        'paddle.fluid.dygraph',

        # PDF 处理
        'fitz',

        # 搜索相关
        'whoosh',
        'whoosh.fields',
        'whoosh.index',
        'whoosh.qparser',
        'whoosh.analysis',
        'jieba',
        'jieba.analyse',

        # 图像处理
        'PIL',
        'PIL.Image',

        # 数据库
        'sqlite3',

        # 其他
        'numpy',
        'json',
        'logging',
        'dataclasses',
        'enum',
        'pathlib',
        'typing',
        'datetime',
        'tempfile',
        'shutil',
        'threading',
        'concurrent.futures',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        # 排除不需要的模块以减小体积
        'tkinter',
        'matplotlib',
        'scipy',
        'pandas',
        'IPython',
        'notebook',
        'selenium',
        'requests',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='PDF Manager',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,  # 不显示控制台窗口
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=None,  # 如果有图标文件，在这里指定路径
)