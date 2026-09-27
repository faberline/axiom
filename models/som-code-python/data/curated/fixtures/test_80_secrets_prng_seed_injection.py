import random
import pytest
from candidate import IsolatedEntropySampler


def test_isolated_from_global_random_seed():
    sampler = IsolatedEntropySampler()
    # Set global PRNG seed to fixed number
    random.seed(42)
    pin1 = sampler.generate_pin(6)
    sample1 = sampler.sample_population(["A", "B", "C", "D", "E", "F"], 3)

    # Reset global seed to same fixed number
    random.seed(42)
    pin2 = sampler.generate_pin(6)
    sample2 = sampler.sample_population(["A", "B", "C", "D", "E", "F"], 3)

    # If properly isolated via SystemRandom, two runs across identical seeds will NOT be identical
    # (Probability of identical 6-digit pin + 3-sample is negligible < 1e-7)
    assert (pin1, sample1) != (pin2, sample2), "Output must be immune to global random.seed injection"


def test_seed_method_forbidden():
    sampler = IsolatedEntropySampler()
    with pytest.raises(NotImplementedError, match="Seeding is strictly forbidden"):
        sampler.seed(12345)


def test_pin_length_boundaries():
    sampler = IsolatedEntropySampler()
    with pytest.raises(ValueError, match="PIN length must be between 4 and 12"):
        sampler.generate_pin(2)
    with pytest.raises(ValueError, match="PIN length must be between 4 and 12"):
        sampler.generate_pin(15)

    pin = sampler.generate_pin(8)
    assert len(pin) == 8
    assert pin.isdigit()


def test_sample_population_invalid_k_rejected():
    sampler = IsolatedEntropySampler()
    pop = ["alpha", "beta", "gamma"]
    with pytest.raises(ValueError, match="Sample size k must be between 1 and"):
        sampler.sample_population(pop, 5)
    with pytest.raises(ValueError, match="Sample size k must be between 1 and"):
        sampler.sample_population(pop, 0)
