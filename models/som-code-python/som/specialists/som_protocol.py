"""Public SOM v1 domain routing."""
from . import som_config

PROTOCOL = "som-v1"


def validate_protocol(protocol: str = PROTOCOL) -> str:
    if protocol != PROTOCOL:
        raise ValueError("Protocol must be som-v1.")
    return protocol


def config_for(protocol: str = PROTOCOL):
    validate_protocol(protocol)
    return som_config


def validate_domain(domain: str, *, training: bool = False) -> str:
    allowed = som_config.TRAIN_DOMAINS if training else som_config.DOMAINS
    if domain not in allowed:
        names = "frontend, python, or mixed" if training else "frontend or python"
        raise ValueError(f"SOM supports {names} only.")
    return domain


def default_run(domain: str) -> str:
    validate_domain(domain, training=True)
    return som_config.run_path(domain)
