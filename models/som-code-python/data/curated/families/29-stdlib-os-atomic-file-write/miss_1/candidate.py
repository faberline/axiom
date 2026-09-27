"""Replace files atomically so readers never see a partial write."""

import contextlib
import json
import os
from pathlib import Path
from typing import Any


class AtomicFileOps:
    """Write-then-rename helpers that leave the target whole on failure."""

    @staticmethod
    def write_text_atomic(
        target_path: str | Path, content: str, encoding: str = "utf-8"
    ) -> None:
        """Write content to a synced temp file beside target, then swap it in."""
        target = Path(target_path).resolve()
        target.parent.mkdir(parents=True, exist_ok=True)
        temp_path: Path | None = None
        try:
            with open(target, mode="w", encoding=encoding) as temp_file:
                temp_path = Path(temp_file.name)
                temp_file.write(content)
                temp_file.flush()
                os.fsync(temp_file.fileno())
            os.replace(temp_path, target)
        except Exception:
            if temp_path is not None:
                with contextlib.suppress(OSError):
                    temp_path.unlink()
            raise

    @staticmethod
    def write_json_atomic(target_path: str | Path, data: Any, indent: int = 2) -> None:
        """Serialize data as JSON and write it atomically."""
        content = json.dumps(data, indent=indent)
        AtomicFileOps.write_text_atomic(target_path, content)
