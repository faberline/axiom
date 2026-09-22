from .coder_model import CoderBasePredictor
from .coder_train import RUN
from .predict import DeveloperPredictor


class CoderPredictor(DeveloperPredictor):
    def __init__(self,checkpoint=RUN):
        super().__init__(checkpoint,base_factory=CoderBasePredictor)
