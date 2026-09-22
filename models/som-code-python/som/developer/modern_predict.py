from .modern_model import ModernBasePredictor
from .modern_train import RUN
from .predict import DeveloperPredictor


class ModernPredictor(DeveloperPredictor):
    def __init__(self,checkpoint=RUN):
        super().__init__(checkpoint,base_factory=ModernBasePredictor)
