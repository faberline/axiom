"""Download only the pinned MBPP train/validation source files."""
import urllib.request
from ..paths import read_json,sha256,write_json
from .mbpp_cases import DATA,LOCK,verify_sources


def download():
    lock=read_json(LOCK)
    for name,record in lock["files"].items():
        dest=DATA/"source"/name
        if dest.exists() and sha256(dest)==record["sha256"]:
            continue
        dest.parent.mkdir(parents=True,exist_ok=True); temp=dest.with_suffix(dest.suffix+".partial")
        with urllib.request.urlopen(record["url"],timeout=90) as response, temp.open("wb") as handle:
            while chunk:=response.read(1024*1024):
                handle.write(chunk)
        if sha256(temp)!=record["sha256"]:
            raise ValueError(f"MBPP checksum mismatch: {name}")
        temp.replace(dest)
    write_json(DATA/"source/sources.json",lock)
    verify_sources()


if __name__=="__main__":
    download()
