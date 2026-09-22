"""Fresh family holdout for the coder-backed selector, never a training source."""
import json
import random

from ..paths import ROOT, read_json, sha256, write_json
from .data import instantiate, check_cases, make_row
from .fixtures import Fixture

FINAL = ROOT / "data/som/research/coder-final"


def cases():
    result=[]
    def add(name,domain,requirement,sources,checks,dom=False):
        result.append(Fixture(name,domain,requirement,tuple(sources),checks,True,dom))
    add("merge_override","python","Merge two dictionaries into a new one. Values from the second dictionary win. Do not mutate either input.",(
        "def f(a,b):\n    return {**a,**b}","def f(a,b):\n    return {**b,**a}",
        "def f(a,b):\n    a.update(b)\n    return a","def f(a,b):\n    return a | {}"),
        "a={@K@:1,'x':2}; b={@K@:@N@,'y':3}; c=f(a,b)\nassert c=={@K@:@N@,'x':2,'y':3}\nassert a=={@K@:1,'x':2}\nassert b=={@K@:@N@,'y':3}")
    add("literal_regex","python","Return whether the entire input text equals a literal pattern. Regex metacharacters in the pattern must be treated literally.",(
        "import re\ndef f(pattern,text):\n    return re.fullmatch(re.escape(pattern),text) is not None",
        "import re\ndef f(pattern,text):\n    return re.fullmatch(pattern,text) is not None",
        "import re\ndef f(pattern,text):\n    return re.search(re.escape(pattern),text) is not None",
        "def f(pattern,text):\n    return pattern in text"),
        "assert f('a.b','a.b') is True\nassert f('a.b','axb') is False\nassert f('a.b','xa.by') is False\nassert f('[x]','[x]') is True")
    add("decimal_round","python","Round a decimal money string to two places using ROUND_HALF_UP. Return Decimal, without converting through binary float.",(
        "from decimal import Decimal,ROUND_HALF_UP\ndef f(s):\n    return Decimal(s).quantize(Decimal('0.01'),rounding=ROUND_HALF_UP)",
        "from decimal import Decimal,ROUND_HALF_UP\ndef f(s):\n    return Decimal(float(s)).quantize(Decimal('0.01'),rounding=ROUND_HALF_UP)",
        "from decimal import Decimal,ROUND_DOWN\ndef f(s):\n    return Decimal(s).quantize(Decimal('0.01'),rounding=ROUND_DOWN)",
        "def f(s):\n    return round(float(s),2)"),
        "from decimal import Decimal\nassert f('1.005')==Decimal('1.01')\nassert f('1.019')==Decimal('1.02')\nassert isinstance(f('2.00'),Decimal)")
    add("adjacent_pairs","python","Return adjacent pairs from the input sequence, preserving order. Empty and one-item sequences have no pairs.",(
        "def f(xs):\n    return list(zip(xs,xs[1:]))","def f(xs):\n    return list(zip(xs,xs))",
        "def f(xs):\n    return list(zip(xs[::2],xs[1::2]))","def f(xs):\n    return list(zip(xs[1:],xs))"),
        "assert f([1,@N@,99])==[(1,@N@),(@N@,99)]\nassert f([])==[]\nassert f([1])==[]")
    add("pop_default","python","Remove and return dictionary key @K@. Use @N@ only when the key is absent, preserving None and zero values.",(
        "def f(data):\n    return data.pop(@K@,@N@)","def f(data):\n    return data.get(@K@,@N@)",
        "def f(data):\n    return data.pop(@K@,None)","def f(data):\n    return data.pop(@K@,@N@) or @N@"),
        "a={@K@:0}; assert f(a)==0; assert @K@ not in a\nassert f({})==@N@\na={@K@:None}; assert f(a) is None; assert @K@ not in a")
    add("newline_normalization","python","Normalize CRLF and bare CR into one LF each. Preserve leading and trailing newlines.",(
        "def f(s):\n    return s.replace('\\r\\n','\\n').replace('\\r','\\n')",
        "def f(s):\n    return s.replace('\\r','\\n').replace('\\r\\n','\\n')",
        "def f(s):\n    return s.replace('\\r','')",
        "def f(s):\n    return s.replace('\\r\\n','\\n').replace('\\r','\\n').strip()"),
        "assert f('\\r\\na\\rb\\r\\n')=='\\na\\nb\\n'\nassert f('')==''")
    add("aria_boolean","frontend","Set aria-expanded to the string true or false to match the boolean state. False must remain an explicit attribute value.",(
        "function f(el,value) { el.setAttribute('aria-expanded',value?'true':'false'); }",
        "function f(el,value) { el.toggleAttribute('aria-expanded',value); }",
        "function f(el,value) { if(value) el.setAttribute('aria-expanded','true'); else el.removeAttribute('aria-expanded'); }",
        "function f(el,value) { el.setAttribute('aria-expanded',value?'false':'true'); }"),
        "const el=document.createElement('button'); f(el,true); assert.equal(el.getAttribute('aria-expanded'),'true'); f(el,false); assert.equal(el.getAttribute('aria-expanded'),'false');",True)
    add("form_multiple","frontend","Return all selected form values for a repeated field name, in document order. Return an empty array when none exists.",(
        "function f(form,name) { return new form.ownerDocument.defaultView.FormData(form).getAll(name); }",
        "function f(form,name) { return new form.ownerDocument.defaultView.FormData(form).get(name); }",
        "function f(form,name) { return new form.ownerDocument.defaultView.FormData(form).getAll(name)[0]; }",
        "function f(form,name) { return []; }"),
        "const form=document.createElement('form'); form.innerHTML='<input name=tag value=a><input name=tag value=b>'; assert.equal(JSON.stringify(f(form,'tag')),'[\"a\",\"b\"]'); assert.equal(JSON.stringify(f(form,'absent')),'[]');",True)
    add("listener_cleanup","frontend","Install a click callback and return a cleanup function. After cleanup, later clicks must not call the callback.",(
        "function f(el,cb) { el.addEventListener('click',cb); return ()=>el.removeEventListener('click',cb); }",
        "function f(el,cb) { el.addEventListener('click',cb); return ()=>el.removeEventListener('click',()=>cb()); }",
        "function f(el,cb) { el.addEventListener('click',cb); return ()=>el.removeEventListener('mousedown',cb); }",
        "function f(el,cb) { el.addEventListener('click',cb); return cb; }"),
        "const el=document.createElement('button'); let n=0; const cleanup=f(el,()=>n++); el.dispatchEvent(new Event('click')); assert.equal(n,1); cleanup(); el.dispatchEvent(new Event('click')); assert.equal(n,1);",True)
    add("propagate_async_error","frontend","Run all asynchronous loaders. Resolve to their values in order, or reject if any loader rejects.",(
        "async function f(loaders) { return await Promise.all(loaders.map(load=>load())); }",
        "async function f(loaders) { return await Promise.allSettled(loaders.map(load=>load())); }",
        "async function f(loaders) { return await Promise.all(loaders.map(load=>load())).catch(()=>[]); }",
        "async function f(loaders) { return []; }"),
        "assert.equal(JSON.stringify(await f([async()=>1,async()=>@N@])),JSON.stringify([1,@N@])); let rejected=false; try { await f([async()=>{throw new Error('fail')}]); } catch { rejected=true; } assert.equal(rejected,true);")
    add("number_input","frontend","Read a number input as a number. Preserve decimal fractions and return NaN when it is empty.",(
        "function f(el) { return el.valueAsNumber; }","function f(el) { return Number(el.value); }",
        "function f(el) { return parseInt(el.value,10); }","function f(el) { return el.value; }"),
        "const el=document.createElement('input'); el.type='number'; el.value='1.5'; assert.equal(f(el),1.5); el.value=''; assert.equal(Number.isNaN(f(el)),true);",True)
    add("css_custom_property","frontend","Set the inline CSS custom property --level to the given numeric value, leaving other style properties intact.",(
        "function f(el,n) { el.style.setProperty('--level',String(n)); }",
        "function f(el,n) { el.style.level=n; }",
        "function f(el,n) { el.setAttribute('--level',String(n)); }",
        "function f(el,n) { el.style.setProperty('level',String(n)); }"),
        "const el=document.createElement('div'); el.style.color='red'; f(el,@N@); assert.equal(el.style.getPropertyValue('--level'),String(@N@)); assert.equal(el.style.color,'red');",True)
    return result


def prepare():
    rng=random.Random(271828)
    originals=[]
    rows=[]
    for fixture in cases():
        for i in range(20):
            case=instantiate(fixture,i,rng)
            originals.append(case)
            rows.append(make_row(case,rng,missing=i<6))
    outcomes=check_cases(originals)
    rng.shuffle(rows)
    FINAL.mkdir(parents=True,exist_ok=True)
    write_json(FINAL/"cases.json",originals)
    write_json(FINAL/"rows.json",rows)
    write_json(FINAL/"manifest.json",{"seed":271828,"families":[f.name for f in cases()],"n":len(rows),
        "rows_sha256":sha256(FINAL/"rows.json"),"source_sha256":sha256(ROOT/"som/developer/final_cases.py"),
        "oracle_results":outcomes,"exposure":"Fresh authored families for coder v2. Never used for training, calibration or checkpoint selection."})
    print(json.dumps({"fresh_final_cases":len(rows),"families":len(cases()),"checks":"passed"}),flush=True)


def rows():
    manifest=read_json(FINAL/"manifest.json")
    if sha256(FINAL/"rows.json")!=manifest["rows_sha256"] or sha256(ROOT/"som/developer/final_cases.py")!=manifest["source_sha256"]:
        raise ValueError("Final coder cases changed.")
    return read_json(FINAL/"rows.json")


if __name__=="__main__":
    prepare()
