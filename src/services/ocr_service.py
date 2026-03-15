"""OCR处理服务"""

import os
import tarfile
import tempfile
from pathlib import Path
from typing import Optional, Dict, List, Any, TYPE_CHECKING

from PIL import Image

from src.utils.path_utils import get_ocr_models_path

if TYPE_CHECKING:
    from src.services.pdf_service import PDFService


# 中文模型的模型名称和下载URL
MODEL_NAMES = {
    "det": "ch_ppocr_mobile_v2.0_det_infer",
    "cls": "ch_ppocr_mobile_v2.0_cls_infer",
    "rec": "ch_ppocr_mobile_v2.0_rec_infer",
}

MODEL_DOWNLOAD_URL = "https://paddleocr.bj.bcebos.com/dygraph_v2.0/ch/{model_name}.tar"


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
                from paddleocr import PaddleOCR

                # 获取模型路径
                model_dir = get_ocr_models_path()

                self._ocr_engine = PaddleOCR(
                    use_angle_cls=True,
                    lang=self._lang,
                    use_gpu=self._use_gpu,
                    show_log=False,
                    model_dir=str(model_dir) if model_dir.exists() else None
                )
                self._available = True
            except ImportError:
                self._available = False
                return None
            except Exception:
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
            return ""

        try:
            ocr_engine = self.ocr
            if ocr_engine is None:
                return ""

            # 将PIL Image转换为numpy数组
            import numpy as np
            img_array = np.array(image)

            # 执行OCR识别
            result = ocr_engine.ocr(img_array, cls=True)

            # 解析结果
            if result is None or len(result) == 0:
                return ""

            # 提取所有识别的文字
            texts = []
            for line in result:
                if line:
                    for item in line:
                        if item and len(item) >= 2:
                            text = item[1][0]  # 获取识别的文字
                            texts.append(text)

            return "\n".join(texts)

        except Exception:
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
            return ""

        # 使用PDF服务渲染页面为图像
        image = pdf_service.render_page_to_image(pdf_path, page_number, dpi=dpi)

        if image is None:
            return ""

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
        model_dir = get_ocr_models_path()
        missing_models = []
        installed_models = []

        # 检查每个模型是否存在
        for model_type, model_name in MODEL_NAMES.items():
            model_path = model_dir / model_name
            if model_path.exists():
                installed_models.append(model_name)
            else:
                missing_models.append(model_name)

        return {
            "installed": len(missing_models) == 0,
            "model_path": str(model_dir) if model_dir.exists() else None,
            "missing_models": missing_models,
            "installed_models": installed_models,
        }

    def get_manual_download_info(self) -> Dict[str, Any]:
        """
        获取模型手动下载信息

        Returns:
            dict: 包含以下键:
                - models: list, 模型信息列表 (包含 name 和 url)
                - model_dir: str, 模型应放置的目录
        """
        model_dir = get_ocr_models_path()
        models = []

        for model_type, model_name in MODEL_NAMES.items():
            models.append({
                "name": model_name,
                "url": MODEL_DOWNLOAD_URL.format(model_name=model_name),
                "type": model_type,
            })

        return {
            "models": models,
            "model_dir": str(model_dir),
        }

    def download_models(self) -> Dict[str, Any]:
        """
        自动下载 OCR 模型

        Returns:
            dict: 包含以下键:
                - success: bool, 是否成功
                - error: str|None, 错误信息 (如果失败)
                - downloaded: list, 已下载的模型名称列表
        """
        import requests

        model_dir = get_ocr_models_path()
        downloaded = []

        try:
            # 确保模型目录存在
            model_dir.mkdir(parents=True, exist_ok=True)

            for model_type, model_name in MODEL_NAMES.items():
                model_path = model_dir / model_name

                # 如果模型已存在，跳过
                if model_path.exists():
                    downloaded.append(model_name)
                    continue

                # 下载模型
                url = MODEL_DOWNLOAD_URL.format(model_name=model_name)

                try:
                    response = requests.get(url, timeout=300)
                    response.raise_for_status()

                    # 保存并解压 tar 文件
                    with tempfile.NamedTemporaryFile(suffix=".tar", delete=False) as tmp_file:
                        tmp_file.write(response.content)
                        tmp_file_path = tmp_file.name

                    try:
                        # 解压到模型目录
                        with tarfile.open(tmp_file_path, "r:*") as tar:
                            tar.extractall(path=str(model_dir))
                        downloaded.append(model_name)
                    finally:
                        # 删除临时文件
                        if os.path.exists(tmp_file_path):
                            os.unlink(tmp_file_path)

                except requests.RequestException as e:
                    return {
                        "success": False,
                        "error": f"下载模型 {model_name} 失败: {str(e)}",
                        "downloaded": downloaded,
                    }

            return {
                "success": True,
                "error": None,
                "downloaded": downloaded,
            }

        except Exception as e:
            return {
                "success": False,
                "error": f"下载模型时发生错误: {str(e)}",
                "downloaded": downloaded,
            }