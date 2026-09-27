import pytest
from sqlalchemy import create_engine, event, select
from sqlalchemy.orm import Session

from candidate import Base, Contact, import_contacts


@pytest.fixture
def session():
    engine = create_engine("sqlite://")

    # pysqlite never emits BEGIN itself, so the first SAVEPOINT would act as
    # the outer transaction; take over transaction control as SQLAlchemy documents.
    @event.listens_for(engine, "connect")
    def _no_driver_transactions(dbapi_connection, _record):
        dbapi_connection.isolation_level = None

    @event.listens_for(engine, "begin")
    def _emit_begin(connection):
        connection.exec_driver_sql("BEGIN")

    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add(Contact(email="a@x.io", name="Ann"))
        s.commit()
        yield s


def emails(session):
    return list(session.scalars(select(Contact.email).order_by(Contact.email)))


def test_bad_rows_are_skipped_and_good_rows_kept(session):
    rejected = import_contacts(
        session,
        [
            ("b@x.io", "Bob"),
            ("A@X.IO", "Ann again"),
            ("not-an-email", "Nobody"),
            ("c@x.io", "Cy"),
            ("b@x.io", "Bob twice"),
        ],
    )
    assert rejected == ["A@X.IO", "not-an-email", "b@x.io"]
    assert emails(session) == ["a@x.io", "b@x.io", "c@x.io"]


def test_the_import_is_committed(session):
    import_contacts(session, [("d@x.io", "Di"), ("a@x.io", "dup")])
    session.rollback()
    assert emails(session) == ["a@x.io", "d@x.io"]


def test_emails_are_stored_lower_case(session):
    assert import_contacts(session, [("E@X.io", "Ed")]) == []
    assert session.scalar(select(Contact.name).where(Contact.email == "e@x.io")) == "Ed"
