"""OCR处理服务"""

import os
from pathlib import Path
from typing import Optional, Dict, List, Any, TYPE_CHECKING

from PIL import Image

from src.utils.logger import get_logger

if TYPE_CHECKING:
    from src.services.pdf_service import PDFService

logger = get_logger("ocr_service")


class OCRService:
    """OCR处理服务类，使用PaddleOCR进行文字识别"""

    def __init__(self, lang: str = "ch", use_gpu: bool = False):
        """
        初始化OCR服务

        Args:
            lang: OCR语言，默认为中文("ch")
            use_gpu: 是否使用GPU加速
        """
        self._lang = lang
        self._use_gpu = use_gpu
        self._ocr_engine = None
        self._available = None

    @property
    def ocr(self):
        """
        延迟加载OCR引擎

        Returns:
            PaddleOCR实例，如果加载失败返回None
        """
        if self._ocr_engine is None:
            try:
                import paddleocr
                from paddleocr import PaddleOCR
                from src.utils.path_utils import get_ocr_models_path

                # 设置 OCR 模型存储路径到程序目录
                ocr_models_path = get_ocr_models_path()
                ocr_models_path.mkdir(parents=True, exist_ok=True)
                os.environ['PADDLEOCR_HOME'] = str(ocr_models_path)
                logger.info(f"OCR模型目录: {ocr_models_path}")

                # 检查版本
                version = paddleocr.__version__
                logger.info(f"PaddleOCR 版本: {version}")

                # 设置环境变量，禁用 oneDNN 以避免某些 CPU 兼容性问题
                os.environ['FLAGS_enable_onednn_backend'] = '0'

                logger.info("正在初始化PaddleOCR引擎...")

                # 使用最简配置初始化，避免不同版本API兼容性问题
                self._ocr_engine = PaddleOCR(
                    use_angle_cls=True,
                    lang=self._lang,
                )

                self._available = True
                logger.info("PaddleOCR引擎初始化成功")

            except ImportError as e:
                logger.error(f"PaddleOCR未安装: {e}")
                self._available = False
                return None
            except Exception as e:
                logger.error(f"PaddleOCR初始化失败: {e}")
                import traceback
                traceback.print_exc()
                self._available = False
                return None

        return self._ocr_engine

    def is_available(self) -> bool:
        """
        检查OCR服务是否可用

        Returns:
            bool: OCR服务是否可用
        """
        if self._available is None:
            # 尝试加载OCR引擎来检查可用性
            _ = self.ocr
        return self._available is True

    def recognize_image(self, image: Image.Image) -> str:
        """
        识别图像中的文字

        Args:
            image: PIL Image对象

        Returns:
            str: 识别出的文字，失败返回空字符串
        """
        if image is None:
            logger.warning("recognize_image: 图像为空")
            return ""

        try:
            ocr_engine = self.ocr
            if ocr_engine is None:
                logger.warning("recognize_image: OCR引擎不可用")
                return ""

            # 将PIL Image转换为numpy数组
            import numpy as np
            img_array = np.array(image)

            logger.debug(f"开始OCR识别，图像尺寸: {img_array.shape}")

            # 执行OCR识别
            result = ocr_engine.ocr(img_array)

            # 解析结果
            if result is None or len(result) == 0:
                logger.debug("OCR识别结果为空")
                return ""

            # 提取所有识别的文字
            texts = []
            for line in result:
                if line:
                    for item in line:
                        if item and len(item) >= 2:
                            text = item[1][0]  # 获取识别的文字
                            texts.append(text)

            result_text = "\n".join(texts)
            logger.debug(f"OCR识别完成，文本长度: {len(result_text)}")
            return result_text

        except Exception as e:
            logger.error(f"OCR识别失败: {e}")
            return ""

    def recognize_image_file(self, image_path: str) -> str:
        """
        识别图像文件中的文字

        Args:
            image_path: 图像文件路径

        Returns:
            str: 识别出的文字，失败返回空字符串
        """
        if not os.path.exists(image_path):
            return ""

        if not os.path.isfile(image_path):
            return ""

        try:
            image = Image.open(image_path)

            # 转换为RGB模式（如果需要）
            if image.mode != "RGB":
                image = image.convert("RGB")

            return self.recognize_image(image)

        except Exception:
            return ""

    def recognize_pdf_page(
        self,
        pdf_service: "PDFService",
        pdf_path: str,
        page_number: int,
        dpi: int = 200
    ) -> str:
        """
        识别PDF页面的文字

        Args:
            pdf_service: PDF服务实例，用于渲染PDF页面
            pdf_path: PDF文件路径
            page_number: 页码（从0开始）
            dpi: 渲染DPI，默认200（更高的DPI可以获得更好的OCR效果）

        Returns:
            str: 识别出的文字，失败返回空字符串
        """
        if pdf_service is None:
            logger.warning("recognize_pdf_page: PDF服务为空")
            return ""

        logger.debug(f"渲染PDF页面: {pdf_path}, 页码: {page_number}, DPI: {dpi}")

        # 使用PDF服务渲染页面为图像
        image = pdf_service.render_page_to_image(pdf_path, page_number, dpi=dpi)

        if image is None:
            logger.warning(f"PDF页面渲染失败: {pdf_path}, 页码: {page_number}")
            return ""

        logger.debug(f"PDF页面渲染成功，尺寸: {image.size}")
        return self.recognize_image(image)

    def check_model_status(self) -> Dict[str, Any]:
        """
        检查 OCR 模型是否已安装

        Returns:
            dict: 包含以下键:
                - installed: bool, 模型是否已完整安装
                - model_path: str|None, 模型目录路径
                - missing_models: list, 缺失的模型名称列表
                - installed_models: list, 已安装的模型名称列表
        """
        # 新版本 PaddleOCR 自动管理模型，只需检查包是否安装
        try:
            import importlib.util
            spec = importlib.util.find_spec("paddleocr")
            if spec is not None:
                return {
                    "installed": True,
                    "model_path": "自动管理",
                    "missing_models": [],
                    "installed_models": ["PaddleOCR"],
                }
        except Exception:
            pass

        return {
            "installed": False,
            "model_path": None,
            "missing_models": ["paddleocr"],
            "installed_models": [],
        }

    def get_manual_download_info(self) -> Dict[str, Any]:
        """
        获取模型手动下载信息

        Returns:
            dict: 包含以下键:
                - models: list, 模型信息列表 (包含 name 和 url)
                - model_dir: str, 模型应放置的目录
        """
        return {
            "models": [{
                "name": "PaddleOCR",
                "url": "pip install paddlepaddle paddleocr",
                "type": "pip package",
            }],
            "model_dir": "PaddleOCR 会自动下载模型到用户目录",
        }

    def download_models(self) -> Dict[str, Any]:
        """
        自动安装 PaddleOCR

        Returns:
            dict: 包含以下键:
                - success: bool, 是否成功
                - error: str|None, 错误信息 (如果失败)
                - downloaded: list, 已下载的模型名称列表
        """
        import subprocess
        import sys

        try:
            # 使用百度镜像安装 paddlepaddle 和 paddleocr
            result = subprocess.run(
                [
                    sys.executable, "-m", "pip", "install",
                    "paddlepaddle", "paddleocr",
                    "-i", "https://mirror.baidu.com/pypi/simple",
                    "--trusted-host", "mirror.baidu.com",
                    "-q"
                ],
                capture_output=True,
                text=True,
                timeout=600
            )

            if result.returncode == 0:
                return {
                    "success": True,
                    "error": None,
                    "downloaded": ["paddlepaddle", "paddleocr"],
                }
            else:
                return {
                    "success": False,
                    "error": f"安装失败: {result.stderr or '未知错误'}",
                    "downloaded": [],
                }
        except subprocess.TimeoutExpired:
            return {
                "success": False,
                "error": "安装超时，请检查网络连接",
                "downloaded": [],
            }
        except Exception as e:
            return {
                "success": False,
                "error": f"安装出错: {str(e)}",
                "downloaded": [],
            }