from ..paths import ROOT, read_json, sha256, write_json

MODEL_PATH = ROOT / "models/qwen2.5-coder-7b-4bit"


def download():
    from huggingface_hub import snapshot_download, hf_hub_download
    lock = read_json(ROOT / "som-research-coder.sources.lock.json")
    snapshot_download(lock["model"]["repo"],revision=lock["model"]["revision"],local_dir=MODEL_PATH,
                      token=False,allow_patterns=["*.json","*.safetensors","*.txt","README.md"])
    hf_hub_download(lock["upstream"]["repo"],"LICENSE",revision=lock["upstream"]["reference_revision"],
                    local_dir=MODEL_PATH,token=False)
    write_json(MODEL_PATH / "sources.json", {"lock":lock,"files":{p.name:sha256(p) for p in MODEL_PATH.iterdir()
               if p.is_file() and p.name != "sources.json"}})
    print("Pinned coder model is ready.",flush=True)


def verify():
    manifest=read_json(MODEL_PATH/"sources.json")
    if manifest["lock"]!=read_json(ROOT/"som-research-coder.sources.lock.json"):
        raise ValueError("Coder model source lock changed.")
    for name,digest in manifest["files"].items():
        if sha256(MODEL_PATH/name)!=digest:
            raise ValueError(f"Coder model file changed: {name}")
    return {"status":"passed","source":manifest["lock"],"files_checked":len(manifest["files"]),
            "manifest_sha256":sha256(MODEL_PATH/"sources.json")}


if __name__ == "__main__":
    download()
