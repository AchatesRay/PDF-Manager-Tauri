# -*- mode: python ; coding: utf-8 -*-
"""
PyInstaller spec file for PDF Manager
"""

import sys
from pathlib import Path

# Get the project root directory
project_root = SPECPATH

a = Analysis(
    ['src/main.py'],
    pathex=[project_root],
    binaries=[],
    datas=[],
    hiddenimports=[
        # PaddlePaddle and PaddleOCR
        'paddle',
        'paddleocr',
        'paddle.dataset.image',
        'paddle.fluid',
        'paddle.framework',
        'paddle.base',
        'paddle.base.framework',
        'paddle.base.dataset.image',

        # PyMuPDF
        'fitz',
        'pymupdf',

        # Whoosh search engine
        'whoosh',
        'whoosh.fields',
        'whoosh.index',
        'whoosh.qparser',
        'whoosh.analysis',
        'whoosh.lang',

        # Jieba Chinese text segmentation
        'jieba',
        'jieba.analyse',

        # Pillow
        'PIL',
        'PIL.Image',

        # NumPy
        'numpy',

        # PyQt6
        'PyQt6',
        'PyQt6.QtCore',
        'PyQt6.QtGui',
        'PyQt6.QtWidgets',
        'PyQt6.sip',

        # Application modules - src package
        'src',
        'src.utils',
        'src.utils.config',
        'src.utils.logger',
        'src.utils.path_utils',
        'src.models',
        'src.models.schemas',
        'src.models.database',
        'src.services',
        'src.services.pdf_service',
        'src.services.ocr_service',
        'src.services.index_service',
        'src.core',
        'src.core.folder_manager',
        'src.core.pdf_manager',
        'src.core.search_service',
        'src.ui',
        'src.ui.main_window',
        'src.ui.widgets',
        'src.ui.widgets.folder_tree',
        'src.ui.widgets.pdf_list',
        'src.ui.widgets.pdf_viewer',
        'src.ui.dialogs',
        'src.ui.dialogs.settings_dialog',
        'src.ui.dialogs.import_dialog',
        'src.ui.dialogs.ocr_setup_dialog',
    ],
    excludes=[
        'tkinter',
        'matplotlib',
        'scipy',
        'pandas',
        'IPython',
        'jupyter',
        'notebook',
    ],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,  # 单目录模式：不将二进制文件打包进exe
    name='PDF Manager',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

# 单目录模式：收集所有文件到一个文件夹
coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='PDF Manager',
)