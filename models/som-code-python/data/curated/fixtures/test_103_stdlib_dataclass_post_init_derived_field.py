import dataclasses

import pytest

from candidate import Parcel


def test_volumetric_weight_wins_for_bulky_parcels():
    parcel = Parcel(2.0, 50, 40, 30)
    assert parcel.billable_kg == 12.0


def test_actual_weight_wins_for_dense_parcels():
    parcel = Parcel(10.0, 10, 10, 10)
    assert parcel.billable_kg == 10.0


def test_weight_limit_is_inclusive():
    assert Parcel(70.0, 10, 10, 10).billable_kg == 70.0
    for weight in (0.0, 70.01):
        with pytest.raises(ValueError, match="weight"):
            Parcel(weight, 10, 10, 10)


def test_rejects_non_positive_dimensions():
    with pytest.raises(ValueError, match="dimensions"):
        Parcel(1.0, 10, 0, 10)


def test_parcel_is_frozen_and_billable_is_derived():
    parcel = Parcel(1.0, 10, 10, 10)
    with pytest.raises(dataclasses.FrozenInstanceError):
        parcel.weight_kg = 5.0
    with pytest.raises(TypeError):
        Parcel(1.0, 10, 10, 10, [], 99.0)


def test_tags_are_not_shared_between_parcels():
    first = Parcel(1.0, 10, 10, 10)
    first.tags.append("fragile")
    second = Parcel(1.0, 10, 10, 10)
    assert second.tags == []
