import pytest
from candidate import AuditEvent, AuditLogger


def test_record_event_defaults_independent():
    logger = AuditLogger()
    evt1 = logger.record_event("login")
    assert evt1.tags == []
    assert evt1.metadata == {}
    evt1.tags.append("mutated")
    evt2 = logger.record_event("logout")
    assert evt2.tags == []
    assert evt1.tags is not evt2.tags


def test_record_event_default_metadata_empty():
    logger = AuditLogger()
    evt = logger.record_event("payment")
    assert evt.metadata == {}


def test_record_event_tags_limit_validation():
    logger = AuditLogger()
    with pytest.raises(ValueError, match="tags cannot exceed 10 items"):
        logger.record_event("action", tags=[f"t{i}" for i in range(11)])


def test_record_event_tags_boundary_ten_allowed():
    logger = AuditLogger()
    tags = [f"t{i}" for i in range(10)]
    evt = logger.record_event("action", tags=tags)
    assert len(evt.tags) == 10


def test_clear_cleans_index_and_events():
    logger = AuditLogger()
    logger.record_event("deploy", tags=["ci", "release"])
    assert len(logger.get_events_by_tag("ci")) == 1
    logger.clear()
    assert logger.count() == 0
    assert logger.get_events_by_tag("ci") == []
    assert logger.get_events_by_tag("release") == []


def test_type_validations():
    logger = AuditLogger()
    with pytest.raises(ValueError):
        logger.record_event("")
    with pytest.raises(TypeError):
        logger.record_event("ok", tags="not-a-list")
    with pytest.raises(TypeError):
        logger.record_event("ok", metadata="not-a-dict")
    with pytest.raises(TypeError):
        logger.record_event("ok", payload="not-a-dict")
