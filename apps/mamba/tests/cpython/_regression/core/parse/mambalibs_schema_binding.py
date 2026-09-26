# RUN: parse
#
# Integration fixture: cclab_schema_mamba native module
#
# This fixture tests parse-level correctness of the BaseModel / Field syntax.
# Full JIT execution requires project-mode with schema-mamba linked in.
#
# Full E2E run (from a project directory with a matching mamba.toml):
#   cclab mamba run
#
# Expected output when run with native module wired:
#   validation passed: True
#   json_schema: {"title":"UserCreate","type":"object","properties":{...}}

from cclab_schema_mamba import BaseModel, Field


class UserCreate(BaseModel):
    username: str = Field(min_length=3, description="Login name")
    age: int = Field(min_value=0, max_value=150)
    email: str = Field(description="Contact email")


# Validate valid data
result: bool = UserCreate.validate({"username": "alice", "age": 30, "email": "a@b.com"})
print("validation passed:", result)

# Export JSON Schema
schema: str = UserCreate.__json_schema__()
print("json_schema:", schema)
