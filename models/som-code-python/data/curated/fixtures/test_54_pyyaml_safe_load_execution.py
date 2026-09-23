import pytest
from candidate import PipelineManifestParser, ConfigValidationError, parse_manifest


@pytest.fixture
def parser():
    return PipelineManifestParser()


def test_valid_manifest_parsed(parser):
    content = """
version: '2.0'
pipeline: deployment
steps:
  - run: echo build
  - run: echo deploy
"""
    data = parser.parse_manifest(content)
    assert data["version"] == "2.0"
    assert data["pipeline"] == "deployment"
    assert len(data["steps"]) == 2


def test_parse_manifest_helper():
    content = """
version: '1.0'
pipeline: test
steps:
  - run: pytest
"""
    data = parse_manifest(content)
    assert data["version"] == "1.0"
    assert data["pipeline"] == "test"


def test_arbitrary_code_execution_blocked(parser):
    exploit_payload = """
version: '1.0'
pipeline: exploit
steps:
  - !!python/object/apply:os.system ['echo COMPROMISED']
"""
    with pytest.raises(ConfigValidationError) as exc_info:
        parser.parse_manifest(exploit_payload)
    assert "syntax" in str(exc_info.value).lower() or "constructor" in str(exc_info.value).lower()


def test_python_object_instantiation_blocked(parser):
    obj_payload = """
version: '1.0'
pipeline: test
steps:
  - !!python/tuple [1, 2, 3]
"""
    with pytest.raises(ConfigValidationError):
        parser.parse_manifest(obj_payload)


def test_scalar_root_rejected(parser):
    with pytest.raises(ConfigValidationError) as exc_info:
        parser.parse_manifest('"just a plain scalar string"')
    assert "mapping" in str(exc_info.value).lower() or "root" in str(exc_info.value).lower()


def test_missing_required_key_rejected(parser):
    incomplete = """
version: '1.0'
pipeline: test
"""
    with pytest.raises(ConfigValidationError) as exc_info:
        parser.parse_manifest(incomplete)
    assert "steps" in str(exc_info.value)


def test_required_keys_boundary_validation():
    with pytest.raises(ValueError, match="required_keys must not be empty"):
        PipelineManifestParser(required_keys=())


def test_empty_content_rejected(parser):
    with pytest.raises(ConfigValidationError):
        parser.parse_manifest("   ")


def test_type_error_on_non_string(parser):
    with pytest.raises(TypeError):
        parser.parse_manifest(12345)  # type: ignore
