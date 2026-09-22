from ..paths import ROOT, read_json, sha256, write_json

MODEL_PATH = ROOT / "models/qwen3.5-4b-4bit"
LOCK = ROOT / "som-research-modern.sources.lock.json"


def download():
    from huggingface_hub import snapshot_download, hf_hub_download
    lock=read_json(LOCK)
    snapshot_download(lock["model"]["repo"],revision=lock["model"]["revision"],local_dir=MODEL_PATH,
        token=False,allow_patterns=["*.json","*.safetensors","*.jinja","*.txt"])
    for name in ("LICENSE","README.md"):
        hf_hub_download(lock["upstream"]["repo"],name,revision=lock["upstream"]["reference_revision"],
                        local_dir=MODEL_PATH,token=False)
    write_json(MODEL_PATH/"sources.json",{"lock":lock,"files":{p.name:sha256(p) for p in MODEL_PATH.iterdir()
               if p.is_file() and p.name!="sources.json"}})
    print("Pinned Qwen3.5 model downloaded and hashed.",flush=True)


def verify():
    manifest=read_json(MODEL_PATH/"sources.json")
    if manifest["lock"]!=read_json(LOCK):
        raise ValueError("Modern model source lock changed.")
    for name,digest in manifest["files"].items():
        if sha256(MODEL_PATH/name)!=digest:
            raise ValueError(f"Modern model file changed: {name}")
    return {"status":"passed","source":manifest["lock"],"files_checked":len(manifest["files"])}


if __name__=="__main__":
    download()
