from .ranker_model import RankerBasePredictor, RUN
from .predict import DeveloperPredictor


class RankerPredictor(DeveloperPredictor):
    """Rank 2–7 supplied repairs and an internal human-review option."""
    def __init__(self,checkpoint=RUN):
        super().__init__(checkpoint,base_factory=RankerBasePredictor)
