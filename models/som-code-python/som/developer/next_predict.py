"""V5 wrapper with the same strict candidate and human-review contract."""
from .ranker_model import RankerBasePredictor
from .predict import DeveloperPredictor
from ..paths import ROOT


class NextPredictor(DeveloperPredictor):
    def __init__(self, checkpoint=ROOT / "runs/som/research/next"):
        super().__init__(checkpoint, base_factory=RankerBasePredictor)
