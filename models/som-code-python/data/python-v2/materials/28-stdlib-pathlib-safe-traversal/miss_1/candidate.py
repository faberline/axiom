from pathlib import Path
from typing import Union


class SafeFileSandbox:
    def __init__(self, root_dir: Union[str, Path]):
        self.root_dir = Path(root_dir).resolve()

    def resolve_safe_path(self, untrusted_path: str) -> Path:
        clean = untrusted_path.lstrip("/\\")
        resolved = (self.root_dir / clean).resolve()
        if not str(resolved).startswith(str(self.root_dir)):
            raise PermissionError(f"Access denied: path '{untrusted_path}' escapes sandbox root '{self.root_dir}'")
        return resolved

    def safe_read_text(self, untrusted_path: str, encoding: str = "utf-8") -> str:
        safe_p = self.resolve_safe_path(untrusted_path)
        return safe_p.read_text(encoding=encoding)

    def safe_write_text(self, untrusted_path: str, content: str, encoding: str = "utf-8") -> Path:
        safe_p = self.resolve_safe_path(untrusted_path)
        safe_p.parent.mkdir(parents=True, exist_ok=True)
        safe_p.write_text(content, encoding=encoding)
        return safe_p
