import copy

import pytest

from som.developer.modern_data import DATA, verify
from som.developer.modern_input import prompt
from som.developer.public_cases import DATA as PUBLIC, execute, rows as public_rows
from som.paths import read_json, read_rows


def test_modern_splits_counts_and_public_boundary():
    result=verify()
    assert result["counts"]=={"train":768,"validation":160,"calibration":320,"seen":320}
    development=[r for s in result["counts"] for r in read_rows(DATA/f"{s}.jsonl")]
    public=public_rows()
    assert len(public)==240
    assert len({r["family"] for r in public})==40
    assert not {r["family"] for r in public}&{r["family"] for r in development}
    assert not {r["state"] for r in public}&{r["state"] for r in development}
    assert not {r["id"] for r in public}&{r["id"] for r in development}
    for case in read_json(PUBLIC/"cases.json"):
        assert [r["passed"] for r in case["oracle"]]==[True,False,False]
        assert all(r["valid_syntax"] for r in case["oracle"])


def test_public_oracles_fail_on_wrong_answers_not_only_syntax():
    py=execute("python",["def f(x): return x+1","def f(x): return x-1"],"assert f(2)==3")
    js=execute("js",["const f=x=>x+1;","const f=x=>x-1;"],"console.assert(f(2)===3);")
    for result in (py,js):
        assert [r["valid_syntax"] for r in result]==[True,True]
        assert [r["passed"] for r in result]==[True,False]


def test_modern_prompt_requires_real_token_list_and_does_not_truncate():
    value={"state":"code","question":"choose","candidates":[{"id":"a","text":"A"},{"id":"b","text":"B"}]}
    class Tokenizer:
        tokens=[1,2,3]
        def apply_chat_template(self,*args,**kwargs):
            assert kwargs["return_dict"] is False
            assert kwargs["enable_thinking"] is False
            return self.tokens
        def encode(self,text,**kwargs):
            return [ord(text)]
    tokenizer=Tokenizer()
    assert prompt(value,tokenizer)==([1,2,3],[65,66])
    tokenizer.tokens={"input_ids":[1,2,3]}
    with pytest.raises(ValueError,match="flat list"):
        prompt(value,tokenizer)
    tokenizer.tokens=[1]*1537
    with pytest.raises(ValueError,match="No text was truncated"):
        prompt(value,tokenizer)
