import pytest

from candidate import GradeScale


def scale():
    return GradeScale([60, 70, 80, 90], ["F", "D", "C", "B", "A"])


def test_threshold_values_belong_to_the_upper_bucket():
    s = scale()
    assert [s.grade(x) for x in (0, 59.9, 60, 89.5, 90, 100)] == ["F", "F", "D", "B", "A", "A"]


def test_out_of_range_scores_are_rejected():
    with pytest.raises(ValueError, match="score out of range"):
        scale().grade(100.5)
    with pytest.raises(ValueError, match="score out of range"):
        scale().grade(-1)


def test_label_count_must_match_thresholds():
    with pytest.raises(ValueError, match="one more entry than thresholds"):
        GradeScale([50], ["F", "P", "D"])


def test_thresholds_must_strictly_increase():
    with pytest.raises(ValueError, match="strictly increasing"):
        GradeScale([60, 60], ["F", "D", "A"])


def test_histogram_lists_every_label_including_empty_ones():
    assert scale().histogram([95, 91, 65]) == {"F": 0, "D": 1, "C": 0, "B": 0, "A": 2}
