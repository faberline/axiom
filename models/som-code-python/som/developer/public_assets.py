"""Fetch only immutable, hash-checked public benchmark source files."""
import urllib.request

from ..paths import ROOT, read_json, sha256, write_json

LOCK=ROOT/"som-research-humanevalpack.sources.lock.json"
DIRECTORY=ROOT/"data/humanevalpack-v3/source"


def verify():
    lock=read_json(LOCK)
    if read_json(DIRECTORY/"sources.json")!=lock:
        raise ValueError("HumanEvalPack source lock changed.")
    files={**lock["files"],lock["attribution"]["file"]:lock["attribution"]}
    for name,record in files.items():
        if "/" in name or sha256(DIRECTORY/name)!=record["sha256"]:
            raise ValueError(f"HumanEvalPack source changed: {name}")
    return {"status":"passed","files":len(files),"revision":lock["revision"]}


def download():
    lock=read_json(LOCK); DIRECTORY.mkdir(parents=True,exist_ok=True)
    files={**lock["files"],lock["attribution"]["file"]:lock["attribution"]}
    for name,record in files.items():
        if "/" in name or ".." in name:
            raise ValueError("Invalid public source filename.")
        dest=DIRECTORY/name
        if dest.exists() and sha256(dest)==record["sha256"]:
            continue
        request=urllib.request.Request(record["url"],headers={"User-Agent":"SOM-local-research"})
        with urllib.request.urlopen(request,timeout=120) as response:
            data=response.read()
        temp=DIRECTORY/(name+".partial"); temp.write_bytes(data)
        if sha256(temp)!=record["sha256"]:
            raise ValueError(f"Downloaded source hash differs: {name}")
        temp.replace(dest)
    write_json(DIRECTORY/"sources.json",lock)
    print(verify(),flush=True)
