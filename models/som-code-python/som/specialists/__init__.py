"""Domain-specialist candidate repair rankers.

The public entry point is :class:`SpecialistPredictor`.  This package never
executes, imports, or applies supplied code.
"""
__all__ = ("SpecialistPredictor",)

def __getattr__(name):
    # Data preparation must work in a CPU-only process.  Do not import MLX until
    # callers actually ask for the inference class.
    if name == "SpecialistPredictor":
        from .predict import SpecialistPredictor
        return SpecialistPredictor
    raise AttributeError(name)
