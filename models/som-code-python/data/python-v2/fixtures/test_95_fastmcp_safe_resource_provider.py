import pytest
from pathlib import Path
import candidate
from candidate import SafeMCPResourceProvider, MCPResourceContent


def test_path_traversal_sibling_prefix_defense(tmp_path):
    root = tmp_path / "sandbox"
    root.mkdir()
    sibling = tmp_path / "sandbox_escape"
    sibling.mkdir()
    secret_file = sibling / "secret.txt"
    secret_file.write_text("classified", encoding="utf-8")

    provider = SafeMCPResourceProvider(root)
    # Traversal attempting to access sibling folder whose name starts with 'sandbox'
    is_safe = provider.is_path_safe("../sandbox_escape/secret.txt")
    assert is_safe is False, "Path traversal to sibling directory with matching prefix must be rejected"


def test_absolute_path_traversal_rejected(tmp_path):
    root = tmp_path / "sandbox"
    root.mkdir()
    provider = SafeMCPResourceProvider(root)

    with pytest.raises(PermissionError, match="Access denied"):
        provider.read_resource("file:///etc/passwd")


def test_exact_max_file_size_boundary_allowed(tmp_path):
    root = tmp_path / "sandbox"
    root.mkdir()
    exact_file = root / "boundary.txt"
    # Write exactly 100 bytes
    payload = "A" * 100
    exact_file.write_text(payload, encoding="utf-8")

    # max_file_size is 100, exactly equal to file size
    provider = SafeMCPResourceProvider(root, max_file_size=100)
    res = provider.read_resource("file://boundary.txt")
    assert res.content == payload
    assert res.size_bytes == 100


def test_default_max_file_size_is_one_megabyte(tmp_path):
    provider = SafeMCPResourceProvider(tmp_path)
    assert provider.max_file_size == 1_000_000, f"Expected 1,000,000 bytes, got {provider.max_file_size}"


def test_read_resource_content_is_str(tmp_path):
    root = tmp_path / "sandbox"
    root.mkdir()
    doc = root / "readme.txt"
    doc.write_text("Hello MCP", encoding="utf-8")

    provider = SafeMCPResourceProvider(root)
    res = provider.read_resource("file://readme.txt")
    assert isinstance(res.content, str)
    assert res.content == "Hello MCP"
    assert res.mime_type == "text/plain"


def test_directory_target_raises_is_a_directory_error(tmp_path):
    root = tmp_path / "sandbox"
    root.mkdir()
    subfolder = root / "docs"
    subfolder.mkdir()

    provider = SafeMCPResourceProvider(root)
    with pytest.raises(IsADirectoryError):
        provider.read_resource("file://docs")
