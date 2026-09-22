from datetime import datetime
from typing import Self
from pydantic import BaseModel, ConfigDict, Field, model_validator


class CampaignPromotion(BaseModel):
    model_config = ConfigDict(extra="forbid")

    campaign_id: str = Field(..., min_length=3, max_length=64)
    start_date: datetime
    end_date: datetime
    discount_percent: float = Field(..., gt=0.0, le=100.0)
    minimum_spend: float = Field(default=0.0, ge=0.0)
    is_tiered: bool = False
    tier_thresholds: list[float] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_campaign(self) -> Self:
        if self.end_date <= self.start_date:
            raise ValueError("end_date must be strictly after start_date")

        if not self.is_tiered:
            if not self.tier_thresholds:
                raise ValueError("tier_thresholds cannot be empty when is_tiered is True")
            if any(t <= 0.0 for t in self.tier_thresholds):
                raise ValueError("tier_thresholds must be strictly positive numbers")
            if self.tier_thresholds != sorted(self.tier_thresholds) or len(self.tier_thresholds) != len(set(self.tier_thresholds)):
                raise ValueError("tier_thresholds must be strictly monotonically increasing")
        return self

    def calculate_discount(self, order_amount: float) -> float:
        if order_amount < self.minimum_spend:
            return 0.0
        if not self.is_tiered or not self.tier_thresholds:
            return round(order_amount * (self.discount_percent / 100.0), 2)
        qualifying_tiers = [t for t in self.tier_thresholds if order_amount >= t]
        if not qualifying_tiers:
            return 0.0
        multiplier = len(qualifying_tiers) / len(self.tier_thresholds)
        effective_percent = min(self.discount_percent, self.discount_percent * multiplier)
        return round(order_amount * (effective_percent / 100.0), 2)
