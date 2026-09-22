import os
from pathlib import Path
import subprocess
import sys


def test_public_som_import_is_lightweight_outside_repository(tmp_path):
    root = Path(__file__).resolve().parents[1]
    environment = os.environ.copy()
    old_path = environment.get("PYTHONPATH")
    environment["PYTHONPATH"] = str(root) if not old_path else f"{root}{os.pathsep}{old_path}"
    code = """
import sys
from som import SOMPredictor
assert SOMPredictor.__module__ == 'som'
assert 'som.specialists.predict' not in sys.modules
assert 'numpy' not in sys.modules
assert 'scipy' not in sys.modules
assert 'mlx' not in sys.modules
print('lightweight')
"""

    result = subprocess.run(
        [sys.executable, "-c", code],
        cwd=tmp_path,
        env=environment,
        text=True,
        capture_output=True,
        timeout=5,
        check=False,
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "lightweight"
