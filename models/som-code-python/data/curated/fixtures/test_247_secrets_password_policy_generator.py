import random
import string

import pytest

from candidate import ALPHABET, CLASSES, MIN_LENGTH, generate_password

SAMPLES = [generate_password() for _ in range(400)]


def test_default_length_and_alphabet():
    assert all(len(p) == 16 for p in SAMPLES)
    assert all(set(p) <= set(ALPHABET) for p in SAMPLES)


def test_every_class_is_present():
    for password in SAMPLES:
        for cls in CLASSES:
            assert set(password) & set(cls), (password, cls)


def test_required_characters_are_shuffled():
    firsts = {p[0] in string.ascii_lowercase for p in SAMPLES}
    assert firsts == {True, False}
    lasts = {p[-1] in CLASSES[3] for p in SAMPLES}
    assert lasts == {True, False}


def test_remaining_characters_use_the_full_alphabet():
    digits = sum(ch in string.digits for p in SAMPLES for ch in p)
    assert digits / len(SAMPLES) > 2.0


def test_custom_length_and_minimum():
    assert len(generate_password(MIN_LENGTH)) == MIN_LENGTH
    assert len(generate_password(40)) == 40
    with pytest.raises(ValueError, match="at least 12"):
        generate_password(11)


def test_not_reproducible_from_random_seed():
    random.seed(0)
    first = generate_password()
    random.seed(0)
    assert generate_password() != first
