# PDF Manager

本地PDF管理工具，支持扫描版PDF的OCR识别和全文搜索。

## 功能特性

- **文件夹管理** - 创建文件夹分类管理PDF文件，支持多级嵌套
- **智能识别** - 自动检测PDF类型（文字型/扫描型），选择最优处理方式
- **OCR识别** - 使用PaddleOCR识别扫描版PDF中的中文内容
- **OCR自动检测** - 启动时自动检测OCR模型，支持一键下载或手动安装
- **全文搜索** - 基于Whoosh和jieba的中文全文搜索，支持关键词高亮
- **PDF预览** - 内置PDF预览器，支持翻页、缩放
- **批量导入** - 支持单个文件、多文件、整个文件夹批量导入
- **完全离线** - 所有功能本地运行，无需联网
- **便携部署** - 数据、日志、OCR模型统一存储在程序目录，方便迁移

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10/11, macOS 10.15+, Linux |
| Python | 3.10 或更高版本 |
| 内存 | 最低 4GB，推荐 8GB+ |
| 磁盘 | 至少 1GB 可用空间（含OCR模型） |

## 安装指南

### Windows

1. **安装 Python**
   - 从 [python.org](https://www.python.org/downloads/) 下载 Python 3.10+
   - 安装时务必勾选 **"Add Python to PATH"**

2. **下载项目**
   ```powershell
   git clone https://github.com/yourusername/pdf-manager.git
   cd pdf-manager
   ```

3. **创建虚拟环境**
   ```powershell
   python -m venv venv
   .\venv\Scripts\activate
   ```

4. **安装依赖**
   ```powershell
   pip install -r requirements.txt
   ```

### macOS / Linux

```bash
# 克隆项目
git clone https://github.com/yourusername/pdf-manager.git
cd pdf-manager

# 创建虚拟环境
python3 -m venv venv
source venv/bin/activate

# 安装依赖
pip install -r requirements.txt
```

### 国内用户加速

如果下载依赖较慢，可以使用国内镜像：

```bash
pip install -r requirements.txt -i https://mirror.baidu.com/pypi/simple
```

## 运行应用

### 启动应用

```bash
# 激活虚拟环境
# Windows:
.\venv\Scripts\activate
# macOS/Linux:
source venv/bin/activate

# 运行应用
python -m src.main
```

### 首次运行

首次运行时，程序会自动检测 OCR 模型是否已安装。如果未安装，会弹出设置对话框：

- **自动下载**：点击按钮自动下载 OCR 模型（约 50MB）
- **手动下载**：显示下载链接，可手动下载后放到程序目录
- **以后再说**：跳过安装，但 OCR 功能将不可用

## 数据存储

所有数据统一存储在程序所在目录，方便备份和迁移：

```
程序目录/
├── PDF Manager.exe          # 主程序
├── logs/                    # 日志目录
│   └── pdf_manager.log      # 运行日志
├── data/                    # 数据目录
│   ├── config.json          # 配置文件
│   ├── pdf_manager.db       # 数据库
│   ├── pdfs/                # PDF 文件
│   ├── thumbnails/          # 缩略图缓存
│   └── index/               # 搜索索引
└── ocr_models/              # OCR 模型文件
```

## 使用指南

### 界面说明

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  PDF Manager                                                                 │
├────────────┬─────────────────────────────┬───────────────────────────────────┤
│            │ 🔍 [内容搜索框____________] │                                   │
│   📁 文件夹 ├─────────────────────────────┤        📖 PDF预览                │
│   ├─工作文档│ 📄 PDF列表                  │   ┌─────────────────────────────┐│
│   │ ├─合同  │ ┌─────────────────────────┐│   │                             ││
│   │ └─报告  │ │📑 报告.pdf   50页  ✓完成││   │    [PDF页面渲染区域]        ││
│   ├─个人资料│ │📑 合同.pdf   12页 处理中││   │                             ││
│   └─归档文件│ └─────────────────────────┘│   └─────────────────────────────┘│
│            │ 🔍 [文件名搜索...] [+]      │   [◀] 1/50 [▶] [打开] [显示]    │
│ [+新建文件夹]│                             │                                   │
└────────────┴─────────────────────────────┴───────────────────────────────────┘
```

界面采用三栏布局：
- **左栏**：文件夹树，管理PDF分类
- **中栏**：内容搜索框 + PDF列表（支持文件名过滤）
- **右栏**：PDF预览区域

### 基本操作

#### 1. 创建文件夹

- 点击左下角 **"+ 新建文件夹"** 按钮
- 或在文件夹上右键选择"新建子文件夹"

#### 2. 添加PDF文件

- 点击 **"添加PDF"** 按钮选择文件
- 或拖拽PDF文件到窗口
- 支持批量选择和文件夹导入

#### 3. 搜索内容

程序提供两种搜索方式：

- **内容搜索**（顶部搜索框）：搜索PDF文件内容
  - 输入关键词后按回车执行全文搜索
  - 结果显示匹配的页面列表和内容摘要
  - 点击结果直接跳转到对应PDF页面

- **文件名搜索**（PDF列表上方）：快速过滤文件名
  - 实时过滤当前文件夹中的PDF文件

#### 4. 查看PDF

- 单击PDF文件查看预览
- 使用翻页按钮导航
- 点击"外部打开"使用系统PDF阅读器打开

#### 5. 删除PDF

- 选中PDF后点击"删除"按钮
- 确认后将删除文件、索引和所有相关数据

### 设置

通过 **文件 → 设置** 可配置：

| 设置项 | 说明 | 默认值 |
|--------|------|--------|
| 存储路径 | 数据文件存储位置 | `./data` |
| OCR并发数 | 同时处理的PDF数量 | 2 |

## 项目结构

```
pdf-manager/
├── src/
│   ├── main.py                 # 应用入口
│   ├── ui/                     # 界面层
│   │   ├── main_window.py      # 主窗口
│   │   ├── widgets/            # UI组件
│   │   │   ├── folder_tree.py  # 文件夹树
│   │   │   ├── pdf_list.py     # PDF列表
│   │   │   ├── pdf_viewer.py   # PDF预览
│   │   │   └── search_results.py # 搜索结果
│   │   └── dialogs/            # 对话框
│   │       ├── settings_dialog.py
│   │       ├── import_dialog.py
│   │       └── ocr_setup_dialog.py
│   ├── core/                   # 业务逻辑层
│   │   ├── folder_manager.py   # 文件夹管理
│   │   ├── pdf_manager.py      # PDF管理
│   │   └── search_service.py   # 搜索服务
│   ├── services/               # 服务层
│   │   ├── pdf_service.py      # PDF处理
│   │   ├── ocr_service.py      # OCR识别
│   │   └── index_service.py    # 索引服务
│   ├── models/                 # 数据模型
│   │   ├── database.py         # 数据库操作
│   │   └── schemas.py          # 数据结构
│   └── utils/                  # 工具函数
│       ├── config.py           # 配置管理
│       ├── logger.py           # 日志工具
│       └── path_utils.py       # 路径工具
├── tests/                      # 测试文件
├── docs/                       # 文档
├── pdf_manager.spec            # PyInstaller 打包配置
├── requirements.txt            # 依赖列表
└── README.md                   # 项目说明
```

## 技术架构

### 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| GUI框架 | PyQt6 | 跨平台图形界面 |
| OCR引擎 | PaddleOCR | 百度开源，中文识别效果最好 |
| 搜索引擎 | Whoosh + jieba | 纯Python，支持中文分词 |
| PDF处理 | PyMuPDF | 高效的PDF渲染和文本提取 |
| 数据库 | SQLite | 轻量级本地数据库 |
| 图像处理 | Pillow | 缩略图生成 |

### 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                      PDF Manager GUI                        │
│                        (PyQt6)                              │
├─────────────┬─────────────┬─────────────┬─────────────────┤
│  文件管理模块  │  搜索模块    │  预览模块    │   设置模块      │
├─────────────┴─────────────┴─────────────┴─────────────────┤
│                      业务逻辑层                             │
│  - 文件夹管理  - PDF导入  - OCR处理  - 搜索服务            │
├───────────────────────────────────────────────────────────┤
│                      服务层                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────┐   │
│  │ OCR Service │  │Search Engine│  │  PDF Service     │   │
│  │ (PaddleOCR) │  │(Whoosh+jieba)│  │  (PyMuPDF)       │   │
│  └─────────────┘  └─────────────┘  └──────────────────┘   │
├───────────────────────────────────────────────────────────┤
│                      数据存储层                             │
│  ┌─────────────────┐  ┌───────────────────────────────┐   │
│  │ SQLite Database │  │ 文件存储 (PDF + 缩略图缓存)    │   │
│  └─────────────────┘  └───────────────────────────────┘   │
└───────────────────────────────────────────────────────────┘
```

### 数据流程

```
PDF导入 → 类型检测 → 文字型？ → 直接提取文字
                         ↓ 否
                    扫描型 → OCR识别图像
                         ↓
                    分词处理 → 写入索引
                         ↓
                      处理完成
```

## 开发指南

### 开发环境搭建

```bash
# 克隆项目
git clone https://github.com/yourusername/pdf-manager.git
cd pdf-manager

# 创建虚拟环境
python -m venv venv
source venv/bin/activate  # Linux/macOS
# 或 .\venv\Scripts\activate  # Windows

# 安装开发依赖
pip install -r requirements.txt
pip install pytest pytest-qt
```

### 运行测试

```bash
# 运行所有测试
python -m pytest tests/ -v

# 运行特定测试
python -m pytest tests/test_pdf_service.py -v
```

### 代码规范

- 使用 Python 3.10+ 语法
- 遵循 PEP 8 编码规范
- 使用类型注解
- 编写单元测试

### 打包发布

使用 PyInstaller 打包成独立可执行文件：

```bash
# 安装 PyInstaller
pip install pyinstaller

# 使用 spec 文件打包（推荐）
pyinstaller pdf_manager.spec --clean

# 可执行文件在 dist/ 目录
```

打包后程序会在程序目录下创建 `data`、`logs`、`ocr_models` 目录存储所有数据。

## 常见问题

### Q: OCR识别速度很慢怎么办？

A:
- 降低OCR并发数（在设置中调整）
- 确保没有其他程序占用CPU
- 如有NVIDIA显卡，可修改代码启用GPU加速

### Q: 导入PDF时提示"无效的PDF文件"？

A:
- 确认文件是有效的PDF格式
- 检查文件是否被其他程序锁定
- 尝试用其他PDF阅读器打开确认文件正常

### Q: 搜索不到内容？

A:
- 确认PDF已完成OCR处理（状态显示"已完成"）
- 尝试使用不同的关键词搜索
- 检查搜索词是否正确

### Q: 如何备份数据？

A:
- 复制程序目录下的 `data/` 文件夹即可备份所有数据
- 包括：数据库、PDF文件、索引、缩略图

### Q: 如何迁移到新电脑？

A:
- **打包版本**：直接复制整个程序目录到新电脑即可
- **开发版本**：复制项目文件夹，安装Python和依赖后运行

### Q: OCR模型下载失败怎么办？

A:
- 点击"手动下载"获取下载链接
- 下载后将文件放到程序目录下的 `ocr_models/` 文件夹
- 重启程序点击"重新检查"

### Q: 日志文件在哪里？

A:
- 日志文件位于程序目录下的 `logs/pdf_manager.log`
- 记录所有操作和错误信息，便于排查问题

## 更新日志

### v0.3.0 (2026-03-15)

- **UI重构**：改为三栏布局（左：文件夹树，中：搜索+列表，右：预览）
- **内容搜索**：修复全文搜索功能，支持搜索PDF内容
- **搜索结果**：显示匹配页面列表和内容摘要
- **页面跳转**：点击搜索结果直接跳转到对应PDF页面
- 新增 SearchResultsWidget 组件

### v0.2.1 (2025-03-15)

- 修复 OCR 模型路径配置问题，确保模型正确加载
- 修复添加文件夹功能不生效的问题
- 修复 PDF 预览中"外部打开"和"在文件夹中显示"功能
- 修复 PDF 删除功能，添加确认对话框
- 改进信号连接，完善 UI 交互

### v0.2.0 (2025-03-14)

- 新增 OCR 引擎自动检测和下载功能
- 新增启动时 OCR 模型状态检查
- 新增手动下载 OCR 模型支持
- 新增运行日志功能，记录程序运行状态
- 改进数据存储位置，统一存储在程序目录
- 改进打包配置，使用 .spec 文件管理
- 修复打包后功能异常的问题

### v0.1.0 (2024-03-13)

- 初始版本发布
- 支持PDF导入和OCR识别
- 支持全文搜索
- 支持文件夹管理
- 内置PDF预览

## 许可证

本项目采用 MIT 许可证。

## 致谢

- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) - OCR引擎
- [PyMuPDF](https://github.com/pymupdf/PyMuPDF) - PDF处理
- [Whoosh](https://github.com/mchaput/whoosh) - 全文搜索
- [PyQt6](https://www.riverbankcomputing.com/software/pyqt/) - GUI框架