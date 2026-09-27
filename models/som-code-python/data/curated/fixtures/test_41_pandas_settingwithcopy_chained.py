"""Oracle test suite for 41-pandas-settingwithcopy-chained."""
import pytest
import candidate
from candidate import ConditionalRecordUpdater, DataFrame


def test_gold_updates_matching_rows():
    updater = ConditionalRecordUpdater()
    df = DataFrame({
        "id": [1, 2, 3, 4],
        "score": [45.0, 75.0, 80.0, 95.0],
        "status": ["pending", "pending", "pending", "pending"],
    })
    res = updater.update_records(df, "score", 75.0, "status", "passed", inplace=False)
    assert res["status"][0] == "pending"
    assert res["status"][1] == "passed"
    assert res["status"][2] == "passed"
    assert res["status"][3] == "passed"


def test_inplace_false_preserves_original():
    updater = ConditionalRecordUpdater()
    df = DataFrame({
        "id": [1, 2, 3],
        "score": [50.0, 85.0, 90.0],
        "status": ["pending", "pending", "pending"],
    })
    res = updater.update_records(df, "score", 80.0, "status", "approved")
    # Catches miss_1 with inplace=True default mutating df
    assert df["status"][1] == "pending", "Original DataFrame should not be mutated when inplace=False"
    assert df["status"][2] == "pending"
    assert res["status"][1] == "approved"
    assert res["status"][2] == "approved"


def test_negative_threshold_rejected():
    updater = ConditionalRecordUpdater()
    df = DataFrame({"score": [10.0, 20.0], "status": ["open", "open"]})
    # Catches miss_2 which omits non-negative threshold validation
    with pytest.raises(ValueError, match="threshold"):
        updater.update_records(df, "score", -1.0, "status", "closed")


def test_exact_threshold_boundary_updated():
    updater = ConditionalRecordUpdater()
    df = DataFrame({
        "id": [101, 102],
        "score": [75.0, 80.0],
        "status": ["queued", "queued"],
    })
    res = updater.update_records(df, "score", 80.0, "status", "ready")
    # Catches miss_3 which uses > 80.0 instead of >= 80.0
    assert res["status"][0] == "queued"
    assert res["status"][1] == "ready", "Row with score exactly equal to threshold must be updated"


def test_below_threshold_rows_unmodified():
    updater = ConditionalRecordUpdater()
    df = DataFrame({
        "id": [201, 202, 203],
        "score": [60.0, 70.0, 90.0],
        "status": ["initial", "initial", "initial"],
    })
    res = updater.update_records(df, "score", 80.0, "status", "completed")
    # Catches miss_4 which inverts condition to < 80.0
    assert res["status"][0] == "initial", "Row with score below threshold must not be updated"
    assert res["status"][1] == "initial", "Row with score below threshold must not be updated"
    assert res["status"][2] == "completed"


def test_chained_indexing_avoided_persistence():
    updater = ConditionalRecordUpdater()
    df = DataFrame({
        "id": [301, 302],
        "score": [85.0, 90.0],
        "status": ["draft", "draft"],
    })
    res = updater.update_records(df, "score", 80.0, "status", "published")
    # Catches miss_5 which uses chained indexing target_df[mask][col] = val that drops updates
    assert res["status"][0] == "published", "Chained indexing assignment failed to persist updates"
    assert res["status"][1] == "published", "Chained indexing assignment failed to persist updates"


def test_invalid_column_names_rejected():
    updater = ConditionalRecordUpdater()
    df = DataFrame({"score": [50.0], "status": ["pending"]})
    with pytest.raises(ValueError, match="filter_col"):
        updater.update_records(df, "nonexistent", 50.0, "status", "active")
    with pytest.raises(ValueError, match="update_col"):
        updater.update_records(df, "score", 50.0, "nonexistent", "active")
