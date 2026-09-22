import numpy as np


class SafeImagePixelRescaler:
    def __init__(self, target_dtype: str = "uint8") -> None:
        if target_dtype not in ("uint8", "uint16"):
            raise ValueError(f"Unsupported target_dtype: {target_dtype}")
        self.target_dtype = target_dtype

    def rescale_intensity(
        self,
        image: np.ndarray,
        scale: float = 1.0,
        offset: float = 0.0,
    ) -> np.ndarray:
        if not isinstance(image, np.ndarray):
            raise TypeError("image must be a numpy ndarray")

        max_val = 255.0 if self.target_dtype == "uint8" else 65535.0
        work = image.astype(np.float64) * scale + offset
        clipped = np.clip(work, 0.0, max_val)
        return np.round(clipped).astype(self.target_dtype)
