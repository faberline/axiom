"""Students and courses linked through an association table."""

from sqlalchemy import Column, ForeignKey, String, Table
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, relationship


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


enrollments = Table(
    "enrollments",
    Base.metadata,
    Column("student_id", ForeignKey("students.id"), primary_key=True),
    Column("course_id", ForeignKey("courses.id"), primary_key=True),
)


class Student(Base):
    """A student taking any number of courses."""

    __tablename__ = "students"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(String(50))
    courses: Mapped[list["Course"]] = relationship(
        secondary=enrollments, back_populates="students"
    )


class Course(Base):
    """A course with a fixed number of seats."""

    __tablename__ = "courses"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str] = mapped_column(String(50))
    capacity: Mapped[int]
    students: Mapped[list[Student]] = relationship(
        secondary=enrollments, back_populates="courses"
    )


def enroll(session: Session, student_id: int, course_id: int) -> bool:
    """Add a student to a course; return False if already enrolled."""
    student = session.get(Student, student_id)
    if student is None:
        raise LookupError(f"student {student_id} not found")
    course = session.get(Course, course_id)
    if course is None:
        raise LookupError(f"course {course_id} not found")
    if course in student.courses:
        return False
    if len(course.students) >= course.capacity:
        raise ValueError(f"course {course.title} is full")
    student.courses.append(course)
    session.commit()
    return True


def roster(session: Session, course_id: int) -> list[str]:
    """Names of the students in a course, alphabetically."""
    course = session.get(Course, course_id)
    if course is None:
        raise LookupError(f"course {course_id} not found")
    return [student.name for student in course.students]
