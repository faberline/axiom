"""Small transfer probe from pinned, inspected Exercism implementations.

Six functions, not sixty independent problems. These are never training data.
"""
import json
import random
import urllib.request

from ..paths import ROOT, read_json, sha256, write_json
from .data import check_cases, make_row

DIRECTORY = ROOT / "data/som/research/developer-transfer"
REVISIONS = {"python": "1f6aab8667bf653b10cc3799f94352fcdb749db6",
             "javascript": "9be84b9eb31eea9beabbe3f7021a12101379d6e5"}


def prepare():
    DIRECTORY.mkdir(parents=True, exist_ok=True)
    source_records, cases = [], []
    for language, revision in REVISIONS.items():
        base = f"https://raw.githubusercontent.com/exercism/{language}/{revision}"
        license_file = DIRECTORY / f"{language}-LICENSE"
        with urllib.request.urlopen(base + "/LICENSE", timeout=30) as response:
            license_file.write_bytes(response.read())
        for exercise in ("leap", "isogram", "pangram"):
            name = "example.py" if language == "python" else "proof.ci.js"
            url = base + f"/exercises/practice/{exercise}/.meta/{name}"
            with urllib.request.urlopen(url, timeout=30) as response:
                original = response.read().decode()
            source_file = DIRECTORY / f"{language}-{exercise}.{language == 'python' and 'py' or 'js'}"
            source_file.write_text(original)
            source_records.append({"repository": f"exercism/{language}", "revision": revision,
                                   "url": url, "sha256": sha256(source_file), "license_file": license_file.name})
            source = original if language == "python" else original.replace("export ", "")
            if language == "python" and exercise == "leap":
                wrong = [source.replace("% 400", "% 100"), source.replace(" and ", " or "),
                         source.replace("year % 4 == 0", "year % 4 != 0")]
                checks = "assert leap_year(2000) is True\nassert leap_year(2004) is True\nassert leap_year(1900) is False\nassert leap_year(2001) is False"
            elif language == "python" and exercise == "isogram":
                wrong = [source.replace(".lower()", ""), source.replace(" if char.isalpha()", ""), source.replace(" == ", " <= ")]
                checks = "assert is_isogram('six-year-old') is True\nassert is_isogram('Alphabet') is False\nassert is_isogram('aba') is False\nassert is_isogram('') is True"
            elif language == "python":
                wrong = [source.replace("all(", "any("), source.replace(".lower()", ""),
                         source.replace("for char in ascii_lowercase", "for char in ascii_lowercase[:-1]")]
                checks = "assert is_pangram('ABCDEFGHIJKLMNOPQRSTUVWXYZ') is True\nassert is_pangram('abcdefghijklmnopqrstuvwxy') is False\nassert is_pangram('abc') is False\nassert is_pangram('') is False"
            elif exercise == "leap":
                wrong = [source.replace("% 400", "% 100"), source.replace("&&", "||"), source.replace("year % 4 === 0", "year % 4 !== 0")]
                checks = "assert.equal(isLeap(2000),true); assert.equal(isLeap(2004),true); assert.equal(isLeap(1900),false); assert.equal(isLeap(2001),false);"
            elif exercise == "isogram":
                wrong = [source.replace(".toLowerCase()", ""), source.replace("/ |-/g", "/ /g"),
                         source.replace("=== stringNoSpaceOrHyphen.length", "<= stringNoSpaceOrHyphen.length")]
                checks = "assert.equal(isIsogram('six-year-old'),true); assert.equal(isIsogram('Alphabet'),false); assert.equal(isIsogram('aba'),false); assert.equal(isIsogram(''),true);"
            else:
                wrong = [source.replace(".toLowerCase()", ""), source.replace("=== alphaLength", ">= alphaLength-1"), source.replace("/[^a-z]+/gi", "/[^a-z]+/g")]
                checks = "assert.equal(isPangram('ABCDEFGHIJKLMNOPQRSTUVWXYZ'),true); assert.equal(isPangram('abcdefghijklmnopqrstuvwxy'),false); assert.equal(isPangram('abcdefghijklmnopqrstuvwxyA'),false); assert.equal(isPangram(''),false);"
            requirement = {
                "leap": "Return whether a Gregorian year is a leap year. Centuries must be divisible by 400; other years must be divisible by 4.",
                "isogram": "Return whether letters appear at most once, ignoring case, spaces and hyphens. The empty string is an isogram.",
                "pangram": "Return whether all 26 English letters occur, ignoring case and non-letter characters."}[exercise]
            cases.append({"id": f"exercism:{language}:{exercise}", "task": "python" if language == "python" else "frontend",
                          "family": f"exercism_{exercise}", "requirement": requirement, "sources": [source,*wrong],
                          "checks": checks, "dom": False, "held_out_family": True})
    outcomes = check_cases(cases)
    rng = random.Random(6942)
    rows = []
    for case in cases:
        for i in range(10):
            row = make_row({**case,"id":case["id"]+f":{i}"}, rng, missing=i<3)
            rows.append(row)
    write_json(DIRECTORY / "cases.json",cases)
    write_json(DIRECTORY / "rows.json",rows)
    write_json(DIRECTORY / "manifest.json",{"sources":source_records,"original_functions":6,"candidate_order_variants":len(rows),
               "oracle_results":outcomes,"rows_sha256":sha256(DIRECTORY/"rows.json"),
               "limits":"Only three basic algorithm families in each language. JavaScript functions are not DOM or React acceptance. Public pretraining exposure is unknown."})
    return {"functions":6,"variants":len(rows),"execution_checks":"passed"}


def rows():
    manifest=read_json(DIRECTORY/"manifest.json")
    if manifest["rows_sha256"] != sha256(DIRECTORY/"rows.json"):
        raise ValueError("Transfer probe changed.")
    return read_json(DIRECTORY/"rows.json")


if __name__ == "__main__":
    print(json.dumps(prepare()),flush=True)
