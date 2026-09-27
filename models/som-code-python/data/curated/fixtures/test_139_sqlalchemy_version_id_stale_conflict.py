import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session
from sqlalchemy.orm.exc import StaleDataError

from candidate import Base, ConflictError, Document, create_document, rename


@pytest.fixture
def engine(tmp_path):
    engine = create_engine(f"sqlite:///{tmp_path / 'docs.db'}")
    Base.metadata.create_all(engine)
    yield engine
    engine.dispose()


def test_versions_start_at_one_and_increment(engine):
    with Session(engine) as s:
        doc = create_document(s, "draft")
        assert doc.version == 1
        rename(doc, s, "final")
        assert doc.version == 2


def test_concurrent_rename_raises_conflict(engine):
    with Session(engine) as s:
        doc_id = create_document(s, "draft").id
    with Session(engine) as a, Session(engine) as b:
        doc_a = a.get(Document, doc_id)
        doc_b = b.get(Document, doc_id)
        rename(doc_b, b, "from b")
        with pytest.raises(
            ConflictError, match=f"document {doc_id} was changed"
        ) as info:
            rename(doc_a, a, "from a")
        assert isinstance(info.value.__cause__, StaleDataError)
        assert doc_a.title == "from b"
        rename(doc_a, a, "from a again")
        assert doc_a.version == 3


def test_blank_title_is_rejected_without_touching_the_row(engine):
    with Session(engine) as s:
        doc = create_document(s, "draft")
        with pytest.raises(ValueError, match="title must not be blank"):
            rename(doc, s, "   ")
        assert doc.title == "draft"
        assert doc.version == 1
