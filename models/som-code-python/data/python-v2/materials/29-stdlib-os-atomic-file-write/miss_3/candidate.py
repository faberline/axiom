import os
import json
import tempfile
from pathlib import Path
from typing import Any, Union


class AtomicFileOps:
    @staticmethod
    def write_text_atomic(target_path: Union[str, Path], content: str, encoding: str = "utf-8") -> None:
        target = Path(target_path).resolve()
        target.parent.mkdir(parents=True, exist_ok=True)
        temp_file = tempfile.NamedTemporaryFile(mode="w", encoding=encoding, dir=target.parent, delete=False)
        temp_path = Path(temp_file.name)
        try:
            temp_file.write(content)
            temp_file.flush()
            temp_file.close()
            os.replace(temp_path, target)
        except Exception:
            temp_file.close()
            if temp_path.exists():
                try:
                    temp_path.unlink()
                except OSError:
                    pass
            raise

    @staticmethod
    def write_json_atomic(target_path: Union[str, Path], data: Any, indent: int = 2) -> None:
        content = json.dumps(data, indent=indent)
        AtomicFileOps.write_text_atomic(target_path, content)
