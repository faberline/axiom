"""All expensive operations are explicit commands."""
import argparse
import json
import sys
from pathlib import Path

from .paths import ROOT


def main():
    parser = argparse.ArgumentParser(prog="som", description="Train and use SOM locally on Apple silicon.")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("dev-prepare")
    commands.add_parser("dev-verify")
    dev_smoke = commands.add_parser("dev-smoke")
    dev_smoke.add_argument("--resume", action="store_true")
    for name in ("dev-train", "dev-evaluate"):
        child = commands.add_parser(name)
        child.add_argument("--run", type=Path, default=ROOT / "runs/som/research/developer")
        if name == "dev-train":
            child.add_argument("--resume", action="store_true")
    dev_predict = commands.add_parser("dev-predict")
    dev_predict.add_argument("--checkpoint", type=Path, default=ROOT / "runs/som/research/developer")
    dev_predict.add_argument("--domain", choices=("python", "frontend"), required=True)
    dev_predict.add_argument("--input", type=Path)
    commands.add_parser("coder-download")
    commands.add_parser("coder-verify")
    commands.add_parser("coder-prepare-final")
    for name in ("coder-train", "coder-evaluate"):
        child=commands.add_parser(name)
        child.add_argument("--run",type=Path,default=ROOT/"runs/som/research/coder")
        if name=="coder-train":
            child.add_argument("--resume",action="store_true")
    coder_predict=commands.add_parser("coder-predict")
    coder_predict.add_argument("--checkpoint",type=Path,default=ROOT/"runs/som/research/coder")
    coder_predict.add_argument("--domain",choices=("python","frontend"),required=True)
    coder_predict.add_argument("--input",type=Path)
    for name in ("modern-download","modern-prepare","modern-verify","public-download","public-prepare"):
        commands.add_parser(name)
    for name in ("modern-train","modern-evaluate"):
        child=commands.add_parser(name)
        child.add_argument("--run",type=Path,default=ROOT/"runs/som/research/modern")
        if name=="modern-train":
            child.add_argument("--resume",action="store_true")
    modern_predict=commands.add_parser("modern-predict")
    modern_predict.add_argument("--checkpoint",type=Path,default=ROOT/"runs/som/research/modern")
    modern_predict.add_argument("--domain",choices=("python","frontend"),required=True)
    modern_predict.add_argument("--input",type=Path)
    for name in ("mbpp-download","mbpp-prepare","ranker-prepare","ranker-prepare-final","ranker-verify"):
        commands.add_parser(name)
    for name in ("ranker-train","ranker-evaluate"):
        child=commands.add_parser(name)
        child.add_argument("--run",type=Path,default=ROOT/"runs/som/research/ranker")
        if name=="ranker-train":
            child.add_argument("--resume",action="store_true")
    ranker_predict=commands.add_parser("ranker-predict")
    ranker_predict.add_argument("--checkpoint",type=Path,default=ROOT/"runs/som/research/ranker")
    ranker_predict.add_argument("--domain",choices=("python","frontend"),required=True)
    ranker_predict.add_argument("--input",type=Path)
    commands.add_parser("next-prepare-final")
    for name in ("next-train", "next-evaluate", "next-diagnostics"):
        child=commands.add_parser(name)
        child.add_argument("--run",type=Path,default=ROOT/"runs/som/research/next")
        if name=="next-train":
            child.add_argument("--resume",action="store_true")
    next_predict=commands.add_parser("next-predict")
    next_predict.add_argument("--checkpoint",type=Path,default=ROOT/"runs/som/research/next")
    next_predict.add_argument("--domain",choices=("python","frontend"),required=True)
    next_predict.add_argument("--input",type=Path)
    specialist_prepare = commands.add_parser("specialist-prepare")
    specialist_prepare.add_argument("--domain", choices=("frontend", "python"), required=True)
    specialist_verify = commands.add_parser("specialist-verify")
    specialist_verify.add_argument("--domain", choices=("frontend", "python"), required=True)
    specialist_smoke=commands.add_parser("specialist-smoke")
    specialist_smoke.add_argument("--domain",choices=("frontend", "python"),required=True)
    specialist_train=commands.add_parser("specialist-train")
    specialist_train.add_argument("--domain",choices=("frontend","python","mixed"),required=True)
    specialist_train.add_argument("--run",type=Path)
    specialist_train.add_argument("--resume",action="store_true")
    specialist_evaluate=commands.add_parser("specialist-evaluate")
    specialist_evaluate.add_argument("--domain",choices=("frontend","python","mixed"),required=True)
    specialist_evaluate.add_argument("--run",type=Path)
    specialist_predict=commands.add_parser("specialist-predict")
    specialist_predict.add_argument("--checkpoint",type=Path)
    specialist_predict.add_argument("--domain",choices=("frontend", "python"),required=True)
    specialist_predict.add_argument("--input",type=Path)
    specialist_audit=commands.add_parser("specialist-family-audit")
    specialist_audit.add_argument("--domain",choices=("frontend", "python"),required=True)
    args = parser.parse_args()
    try:
        if args.command == "next-prepare-final":
            from .developer.next_holdout import prepare
            prepare()
        elif args.command == "next-train":
            from .developer.next_train import train
            train(args.run,resume=args.resume)
        elif args.command == "next-evaluate":
            from .developer.next_evaluate import evaluate
            evaluate(args.run)
        elif args.command == "next-diagnostics":
            from .developer.next_diagnostics import evaluate
            evaluate(args.run)
        elif args.command == "next-predict":
            from .developer.next_predict import NextPredictor
            value=json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(NextPredictor(args.checkpoint).predict(value,args.domain),indent=2))
        elif args.command == "specialist-prepare":
            from .specialists.som_data import prepare
            print(json.dumps(prepare(args.domain), indent=2))
        elif args.command == "specialist-verify":
            from .specialists.som_data import verify
            print(json.dumps(verify(args.domain),indent=2))
        elif args.command == "specialist-smoke":
            from .specialists.som_protocol import validate_domain
            validate_domain(args.domain)
            # Python smoke uses its separate 32-row contract.  Formal Python
            # preflight stays mandatory for train and evaluation.
            if args.domain != "python":
                from .specialists.som_preflight import require_ready
                require_ready(args.domain)
            from .specialists.som_train import smoke
            smoke(args.domain)
        elif args.command == "specialist-train":
            from .specialists.som_protocol import validate_domain
            validate_domain(args.domain, training=True)
            from .specialists.som_preflight import require_ready
            require_ready("frontend" if args.domain == "mixed" else args.domain)
            from .specialists.som_train import train
            train(args.domain,args.run,resume=args.resume)
        elif args.command == "specialist-evaluate":
            from .specialists.som_protocol import validate_domain
            validate_domain(args.domain, training=True)
            from .specialists.som_preflight import require_ready
            require_ready("frontend" if args.domain == "mixed" else args.domain)
            from .specialists.som_eval import evaluate
            result=evaluate(args.domain,args.run)
            if result is not None:
                print(json.dumps(result,indent=2))
        elif args.command == "specialist-predict":
            from . import SOMPredictor
            value=json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(SOMPredictor(args.checkpoint).predict(value,args.domain),indent=2))
        elif args.command == "specialist-family-audit":
            from .specialists.som_data import audit
            print(json.dumps(audit(args.domain),indent=2))
        elif args.command == "mbpp-download":
            from .developer.mbpp_assets import download
            download()
        elif args.command == "mbpp-prepare":
            from .developer.mbpp_cases import prepare
            prepare()
        elif args.command == "ranker-prepare":
            from .developer.ranker_data import prepare
            prepare()
        elif args.command == "ranker-prepare-final":
            from .developer.ranker_holdout import prepare
            prepare()
        elif args.command == "ranker-verify":
            from .developer.ranker_data import verify
            from .developer.ranker_holdout import rows
            from .developer.modern_assets import verify as assets
            print(json.dumps({"data":verify(),"fresh_rows":len(rows()),"assets":assets()},indent=2))
        elif args.command == "ranker-train":
            from .developer.ranker_train import train
            train(args.run,resume=args.resume)
        elif args.command == "ranker-evaluate":
            from .developer.ranker_evaluate import evaluate
            evaluate(args.run)
        elif args.command == "ranker-predict":
            from .developer.ranker_predict import RankerPredictor
            value=json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(RankerPredictor(args.checkpoint).predict(value,args.domain),indent=2))
        elif args.command == "modern-download":
            from .developer.modern_assets import download
            download()
        elif args.command == "public-download":
            from .developer.public_assets import download
            download()
        elif args.command == "public-prepare":
            from .developer.public_cases import prepare
            prepare()
        elif args.command == "modern-prepare":
            from .developer.modern_data import prepare
            prepare()
        elif args.command == "modern-verify":
            from .developer.modern_assets import verify as assets
            from .developer.modern_data import verify as data
            from .developer.public_assets import verify as sources
            from .developer.public_cases import rows
            print(json.dumps({"assets":assets(),"data":data(),"public_sources":sources(),"public_rows":len(rows())},indent=2))
        elif args.command == "modern-train":
            from .developer.modern_train import train
            train(args.run,resume=args.resume)
        elif args.command == "modern-evaluate":
            from .developer.modern_evaluate import evaluate
            evaluate(args.run)
        elif args.command == "modern-predict":
            from .developer.modern_predict import ModernPredictor
            value=json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(ModernPredictor(args.checkpoint).predict(value,args.domain),indent=2))
        elif args.command == "coder-download":
            from .developer.coder_assets import download
            download()
        elif args.command == "coder-verify":
            from .developer.coder_assets import verify
            print(json.dumps(verify(),indent=2))
        elif args.command == "coder-prepare-final":
            from .developer.final_cases import prepare
            prepare()
        elif args.command == "coder-train":
            from .developer.coder_train import train
            train(args.run,resume=args.resume)
        elif args.command == "coder-evaluate":
            from .developer.coder_evaluate import evaluate
            evaluate(args.run)
        elif args.command == "coder-predict":
            from .developer.coder_predict import CoderPredictor
            value=json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(CoderPredictor(args.checkpoint).predict(value,args.domain),indent=2))
        elif args.command == "dev-prepare":
            from .developer.data import prepare
            prepare()
        elif args.command == "dev-verify":
            from .developer.data import verify
            print(json.dumps(verify(), indent=2))
        elif args.command in ("dev-smoke", "dev-train"):
            from .developer.train import train, SMOKE
            smoke = args.command == "dev-smoke"
            train(SMOKE if smoke else args.run, smoke=smoke, resume=args.resume)
        elif args.command == "dev-evaluate":
            from .developer.evaluate import evaluate
            evaluate(args.run)
        elif args.command == "dev-predict":
            from .developer.predict import DeveloperPredictor
            value = json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(DeveloperPredictor(args.checkpoint).predict(value, args.domain), indent=2))
        elif args.command == "download":
            from .assets import download
            download()
        elif args.command == "prepare":
            from .data import prepare
            prepare()
        elif args.command == "verify":
            from .verification import verify
            print(json.dumps(verify(), indent=2))
        elif args.command in ("smoke", "train"):
            from .train import train
            smoke = args.command == "smoke"
            train(ROOT / "runs" / "smoke" if smoke else args.run, smoke=smoke, resume=args.resume)
        elif args.command == "evaluate":
            from .evaluate import evaluate
            evaluate(args.run, fresh=args.fresh)
        elif args.command == "predict":
            from .model import Predictor
            value = json.loads(args.input.read_text() if args.input else sys.stdin.read())
            print(json.dumps(Predictor(args.checkpoint).predict(value), indent=2))
    except (ValueError, FileNotFoundError, RuntimeError) as error:
        parser.exit(1, f"Error: {error}\n")


if __name__ == "__main__":
    main()
