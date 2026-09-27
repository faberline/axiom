"""Notes API that lets only named browser origins call it with credentials."""

from fastapi import FastAPI, Response
from fastapi.middleware.cors import CORSMiddleware

ALLOWED_ORIGINS = ["https://app.example.com", "https://admin.example.com"]

app = FastAPI(title="Notes")
app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_credentials=True,
    allow_methods=["GET", "POST", "DELETE"],
    allow_headers=["Authorization", "Content-Type"],
    max_age=3600,
)


@app.get("/notes/{note_id}")
def read_note(note_id: int, response: Response) -> dict[str, int | str]:
    """Return a note and its version header."""
    response.headers["X-Note-Version"] = "3"
    return {"id": note_id, "text": "hello"}


@app.delete("/notes/{note_id}", status_code=204)
def delete_note(note_id: int) -> None:
    """Pretend to delete a note."""
    del note_id
