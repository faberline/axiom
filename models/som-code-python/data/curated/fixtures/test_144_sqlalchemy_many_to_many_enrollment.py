import pytest
from sqlalchemy import create_engine, func, select
from sqlalchemy.orm import Session

from candidate import Base, Course, Student, enroll, enrollments, roster


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add_all(
            [
                Student(name="zoe"),
                Student(name="adam"),
                Student(name="mia"),
                Course(title="math", capacity=2),
                Course(title="art", capacity=5),
                Course(title="music", capacity=5),
            ]
        )
        s.commit()
        yield s


def test_roster_is_sorted_by_name(session):
    assert enroll(session, 1, 1) is True
    assert enroll(session, 2, 1) is True
    assert roster(session, 1) == ["adam", "zoe"]
    assert session.scalar(select(func.count()).select_from(enrollments)) == 2


def test_enrolling_twice_is_a_no_op(session):
    assert enroll(session, 1, 2) is True
    assert enroll(session, 1, 2) is False
    assert roster(session, 2) == ["zoe"]


def test_capacity_is_enforced_per_course(session):
    enroll(session, 1, 1)
    enroll(session, 2, 1)
    with pytest.raises(ValueError, match="course math is full"):
        enroll(session, 3, 1)
    assert enroll(session, 1, 1) is False
    assert roster(session, 1) == ["adam", "zoe"]


def test_capacity_counts_the_course_not_the_student(session):
    enroll(session, 3, 2)
    enroll(session, 3, 3)
    assert enroll(session, 3, 1) is True


def test_unknown_ids_raise(session):
    with pytest.raises(LookupError, match="student 9 not found"):
        enroll(session, 9, 1)
    with pytest.raises(LookupError, match="course 9 not found"):
        enroll(session, 1, 9)
