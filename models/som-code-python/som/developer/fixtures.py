"""Authored repair exercises. Labels are accepted only after execution checks.

The held-out families never appear in training, validation, or calibration.
These fixtures are synthetic exercises, not claims about real repository bugs.
"""
from dataclasses import dataclass


@dataclass(frozen=True)
class Fixture:
    name: str
    domain: str
    requirement: str
    sources: tuple[str, ...]
    checks: str
    held_out: bool = False
    dom: bool = False


def fixtures():
    result = []
    def py(name, requirement, bodies, checks, held=False):
        result.append(Fixture(name, "python", requirement, tuple(bodies), checks, held))
    def js(name, requirement, bodies, checks, held=False, dom=False):
        result.append(Fixture(name, "frontend", requirement, tuple(bodies), checks, held, dom))

    py("threshold_filter", "Keep values greater than or equal to @N@, in their original order.", (
        "def f(xs):\n    return [x for x in xs if x >= @N@]",
        "def f(xs):\n    return [x for x in xs if x > @N@]",
        "def f(xs):\n    return [x for x in xs if x <= @N@]",
        "def f(xs):\n    return sorted(x for x in xs if x >= @N@)"),
       "assert f([@N@+2,@N@,@N@-1]) == [@N@+2,@N@]\nassert f([]) == []")
    py("stable_unique", "Remove duplicate strings. Keep the first occurrence and preserve empty strings.", (
        "def f(xs):\n    return list(dict.fromkeys(xs))",
        "def f(xs):\n    return sorted(set(xs))",
        "def f(xs):\n    return [x for x in dict.fromkeys(xs) if x]",
        "def f(xs):\n    return xs"),
       "assert f(['z','a','z','','a']) == ['z','a','']\nassert f([]) == []")
    py("none_default", "Use @N@ only when the input is None. Preserve zero, False, and empty strings.", (
        "def f(x):\n    return @N@ if x is None else x",
        "def f(x):\n    return x or @N@",
        "def f(x):\n    return @N@",
        "def f(x):\n    return x if x else @N@ + 1"),
       "assert f(None) == @N@\nassert f(0) == 0\nassert f('') == ''\nassert f(False) is False")
    py("sort_records", "Return records sorted by numeric @K@ ascending, without changing the input list.", (
        "def f(xs):\n    return sorted(xs, key=lambda x: x[@K@])",
        "def f(xs):\n    return sorted(xs, key=lambda x: x[@K@], reverse=True)",
        "def f(xs):\n    return sorted(xs, key=lambda x: str(x[@K@]))",
        "def f(xs):\n    xs.sort(key=lambda x: x[@K@])\n    return xs"),
       "xs=[{@K@:20},{@K@:3},{@K@:11}]\ny=f(xs)\nassert [r[@K@] for r in y] == [3,11,20]\nassert xs[0][@K@] == 20\nassert f([]) == []")
    py("clamp", "Clamp the input number to the inclusive range zero through @N@.", (
        "def f(x):\n    return max(0, min(x, @N@))",
        "def f(x):\n    return min(0, max(x, @N@))",
        "def f(x):\n    return min(x, @N@)",
        "def f(x):\n    return max(0, x)"),
       "assert f(-1)==0\nassert f(@N@+1)==@N@\nassert f(1)==1")
    py("csv_quoted", "Parse one CSV record, including quoted commas and empty fields.", (
        "import csv\ndef f(s):\n    return next(csv.reader([s]))",
        "def f(s):\n    return s.split(',')",
        "def f(s):\n    return s.split()",
        "def f(s):\n    return list(s)"),
       "assert f('a,\"b,c\",') == ['a','b,c','']\nassert f('x,y') == ['x','y']")
    py("fresh_default", "Append to a provided bucket. If no bucket is given, create a fresh list for each call.", (
        "def f(x, bucket=None):\n    if bucket is None:\n        bucket=[]\n    bucket.append(x)\n    return bucket",
        "def f(x, bucket=[]):\n    bucket.append(x)\n    return bucket",
        "def f(x, bucket=None):\n    bucket=list(bucket or [])\n    bucket.append(x)\n    return bucket",
        "def f(x, bucket=None):\n    return [x]"),
       "assert f(1)==[1]\nassert f(2)==[2]\na=[0]\nassert f(3,a) is a\nassert a==[0,3]")
    py("nested_lookup", "Read the value field in dictionary @K@. Use @N@ for a missing field. Preserve zero.", (
        "def f(row):\n    return row.get(@K@, {}).get('value', @N@)",
        "def f(row):\n    return row[@K@]['value']",
        "def f(row):\n    return row.get(@K@, {}).get('value') or @N@",
        "def f(row):\n    return row.get('value', @N@)"),
       "assert f({})==@N@\nassert f({@K@:{}})==@N@\nassert f({@K@:{'value':0}})==0\nassert f({@K@:{'value':99}})==99")
    py("utc_conversion", "Read an ISO timestamp with an explicit offset and return its hour in UTC.", (
        "from datetime import datetime, timezone\ndef f(s):\n    return datetime.fromisoformat(s).astimezone(timezone.utc).hour",
        "from datetime import datetime, timezone\ndef f(s):\n    return datetime.fromisoformat(s).replace(tzinfo=timezone.utc).hour",
        "from datetime import datetime\ndef f(s):\n    return datetime.fromisoformat(s).hour",
        "def f(s):\n    return int(s[11:13])+8"),
       "assert f('2025-01-01T12:00:00+08:00')==4\nassert f('2025-01-01T23:00:00-02:00')==1")
    py("parse_integer", "Parse a base-10 integer string. Return None for invalid strings or None. Reject decimal fractions.", (
        "def f(s):\n    try:\n        return int(s)\n    except (ValueError, TypeError):\n        return None",
        "def f(s):\n    try:\n        return int(float(s))\n    except (ValueError, TypeError):\n        return None",
        "def f(s):\n    try:\n        return int(s)\n    except (ValueError, TypeError):\n        return 0",
        "def f(s):\n    return s"),
       "assert f(' @N@ ')==@N@\nassert f(None) is None\nassert f('bad') is None\nassert f('2.5') is None")
    py("flatten_one", "Flatten a list by exactly one level, preserving order and any deeper nested values.", (
        "def f(groups):\n    return [x for group in groups for x in group]",
        "def f(groups):\n    return list(groups)",
        "def f(groups):\n    return [x for group in groups[::-1] for x in group]",
        "def f(groups):\n    return sorted(x for group in groups for x in group)"),
       "assert f([[2],[1]])==[2,1]\nassert f([[[1]],[2]])==[[1],2]\nassert f([])==[]")
    py("zip_strict", "Return paired items. Raise ValueError if the two input lengths differ.", (
        "def f(a,b):\n    return list(zip(a,b,strict=True))",
        "def f(a,b):\n    return list(zip(a,b))",
        "def f(a,b):\n    return dict(zip(a,b))",
        "from itertools import zip_longest\ndef f(a,b):\n    return list(zip_longest(a,b))"),
       "assert f([1],[2])==[(1,2)]\nassert f([],[])==[]\ntry:\n    f([1,2],[3])\nexcept ValueError:\n    pass\nelse:\n    raise AssertionError('unequal lengths accepted')")
    py("url_encoding", "Encode a query parameter named @K@. Preserve spaces, plus signs and ampersands in its value.", (
        "from urllib.parse import urlencode\ndef f(s):\n    return urlencode({@K@:s})",
        "def f(s):\n    return @K@+'='+s",
        "def f(s):\n    return @K@+'='+s.replace(' ','+')",
        "from urllib.parse import quote\ndef f(s):\n    return quote(@K@+'='+s,safe='')"),
       "from urllib.parse import parse_qs\nassert parse_qs(f('a b+c&d'))=={@K@:['a b+c&d']}")
    py("json_lines", "Parse JSON Lines into a list. Ignore blank lines, and preserve JSON types.", (
        "import json\ndef f(s):\n    return [json.loads(line) for line in s.splitlines() if line.strip()]",
        "import json\ndef f(s):\n    return json.loads(s)",
        "import json\ndef f(s):\n    return json.dumps(s)",
        "def f(s):\n    return [line for line in s.splitlines() if line.strip()]"),
       "assert f('1\\n\\n{\"a\":2}\\n')==[1,{'a':2}]\nassert f('   \\n')==[]")
    py("await_loader", "Call an asynchronous loader once and return its resolved value, including falsy values.", (
        "async def f(loader):\n    return await loader()",
        "async def f(loader):\n    return loader()",
        "async def f(loader):\n    return await loader() or @N@",
        "async def f(loader):\n    return None"),
       "import asyncio\nasync def loader():\n    return 0\nassert asyncio.run(f(loader))==0\nasync def other():\n    return @N@\nassert asyncio.run(f(other))==@N@")
    py("file_suffix", "Return the final filename extension in lowercase, including its dot. Return an empty string when absent.", (
        "from pathlib import Path\ndef f(s):\n    return Path(s).suffix.lower()",
        "def f(s):\n    return s.split('.')[1]",
        "def f(s):\n    return '.'+s.rsplit('.',1)[-1].lower()",
        "from pathlib import Path\ndef f(s):\n    return Path(s).suffix"),
       "assert f('folder.with.dot/archive.TAR.GZ')=='.gz'\nassert f('README')==''\nassert f('note.txt')=='.txt'")

    py("chunking", "Split a list into consecutive groups of at most @N@ items. Keep the final short group.", (
        "def f(xs):\n    return [xs[i:i+@N@] for i in range(0,len(xs),@N@)]",
        "def f(xs):\n    return [xs[i:i+@N@] for i in range(0,len(xs),@N@+1)]",
        "def f(xs):\n    return [xs[i:i+@N@] for i in range(0,len(xs)-@N@+1,@N@)]",
        "def f(xs):\n    return [xs[:@N@]]"),
       "xs=list(range(@N@*2+1))\nassert f(xs)==[xs[:@N@],xs[@N@:@N@*2],xs[@N@*2:]]\nassert f([])==[]", True)
    py("group_records", "Group records by @K@, retaining every record in each group and its input order.", (
        "def f(xs):\n    groups={}\n    for x in xs:\n        groups.setdefault(x[@K@],[]).append(x)\n    return groups",
        "def f(xs):\n    return {x[@K@]:x for x in xs}",
        "def f(xs):\n    return {x[@K@]:[x] for x in xs}",
        "def f(xs):\n    return {x[@K@]:xs for x in xs}"),
       "a={@K@:'a','v':1}; b={@K@:'b','v':2}; c={@K@:'a','v':3}\nassert f([a,b,c])=={'a':[a,c],'b':[b]}\nassert f([])=={}", True)
    py("count_values", "Count every occurrence of each hashable input value, including empty strings.", (
        "from collections import Counter\ndef f(xs):\n    return dict(Counter(xs))",
        "def f(xs):\n    return {x:1 for x in xs}",
        "def f(xs):\n    return {x:xs.count(x) for x in xs if x}",
        "def f(xs):\n    return len(xs)"),
       "assert f(['a','','a'])=={'a':2,'':1}\nassert f([])=={}", True)
    py("deep_copy", "Return an independent copy of a nested list. Editing the copy must not change the original.", (
        "from copy import deepcopy\ndef f(xs):\n    return deepcopy(xs)",
        "def f(xs):\n    return xs.copy()",
        "def f(xs):\n    return xs",
        "def f(xs):\n    return [str(x) for x in xs]"),
       "a=[[@N@],[]]; b=f(a)\nassert b==a\nb[0].append(99)\nassert a==[[@N@],[]]\nassert f([])==[]", True)

    js("numeric_sort", "Return numbers sorted ascending. Do not mutate the input array.", (
        "function f(xs) { return [...xs].sort((a,b)=>a-b); }",
        "function f(xs) { return [...xs].sort(); }",
        "function f(xs) { return xs.sort((a,b)=>a-b); }",
        "function f(xs) { return [...xs].sort((a,b)=>b-a); }"),
       "const xs=[20,3,11]; assert.equal(JSON.stringify(f(xs)),'[3,11,20]'); assert.equal(xs[0],20); assert.equal(f([]).length,0);")
    js("nullish_default", "Use @N@ when the value is null or undefined. Preserve zero, false and the empty string.", (
        "function f(x) { return x ?? @N@; }", "function f(x) { return x || @N@; }",
        "function f(x) { return x ? x : @N@+1; }", "function f(x) { return @N@; }"),
       "assert.equal(f(null),@N@); assert.equal(f(undefined),@N@); assert.equal(f(0),0); assert.equal(f(false),false); assert.equal(f(''),'');")
    js("state_append", "Append one item to React-style array state. Return a new array and leave the original unchanged.", (
        "function f(xs,item) { return [...xs,item]; }", "function f(xs,item) { return xs.push(item); }",
        "function f(xs,item) { xs.push(item); return xs; }", "function f(xs,item) { return [xs,item]; }"),
       "const xs=[1]; const y=f(xs,@N@); assert.equal(JSON.stringify(y),JSON.stringify([1,@N@])); assert.equal(xs.length,1); assert.notEqual(xs,y);")
    js("state_update", "Update @K@ on the record with the given id. Preserve the old array and old records.", (
        "function f(xs,id,value) { return xs.map(x=>x.id===id?{...x,[@K@]:value}:x); }",
        "function f(xs,id,value) { xs.find(x=>x.id===id)[@K@]=value; return xs; }",
        "function f(xs,id,value) { return xs.map(x=>({...x,[@K@]:value})); }",
        "function f(xs,id,value) { return xs.filter(x=>x.id!==id); }"),
       "const a={id:1,[@K@]:0}, b={id:2,[@K@]:77}; const xs=[a,b], y=f(xs,1,@N@); assert.equal(y.length,2); assert.equal(y[0][@K@],@N@); assert.equal(y[1][@K@],77); assert.equal(a[@K@],0); assert.notEqual(y,xs);")
    js("state_remove", "Remove the given id from array state without mutating the original array.", (
        "function f(xs,id) { return xs.filter(x=>x.id!==id); }",
        "function f(xs,id) { return xs.filter(x=>x.id===id); }",
        "function f(xs,id) { xs.splice(xs.findIndex(x=>x.id===id),1); return xs; }",
        "function f(xs,id) { return xs; }"),
       "const xs=[{id:1},{id:2}]; const y=f(xs,1); assert.equal(y.length,1); assert.equal(y[0].id,2); assert.equal(xs.length,2); assert.equal(f(xs,9).length,2);")
    js("async_map", "Apply an asynchronous multiply-by-@N@ operation to each item. Return an array of resolved values in input order.", (
        "async function f(xs) { return await Promise.all(xs.map(async x=>x*@N@)); }",
        "async function f(xs) { return await xs.map(async x=>x*@N@); }",
        "async function f(xs) { return xs.forEach(async x=>x*@N@); }",
        "async function f(xs) { return await Promise.all(xs.map(async x=>x+@N@)); }"),
       "assert.equal(JSON.stringify(await f([0,2])),JSON.stringify([0,2*@N@])); assert.equal(JSON.stringify(await f([])),'[]');")
    js("query_encode", "Build a query parameter @K@ without losing spaces, plus signs or ampersands in the value.", (
        "function f(s) { return new URLSearchParams({[@K@]:s}).toString(); }",
        "function f(s) { return @K@+'='+s; }",
        "function f(s) { return encodeURI(@K@+'='+s); }",
        "function f(s) { return encodeURIComponent(@K@+'='+s); }"),
       "const s='a b+c&d'; assert.equal(new URLSearchParams(f(s)).get(@K@),s);")
    js("parse_numbers", "Convert numeric strings to numbers. Preserve decimal fractions and exponent notation.", (
        "function f(xs) { return xs.map(Number); }", "function f(xs) { return xs.map(x=>parseInt(x,10)); }",
        "function f(xs) { return xs.map(x=>x*@N@); }", "function f(xs) { return xs; }"),
       "assert.equal(JSON.stringify(f(['1.5','2e2','-1'])),'[1.5,200,-1]'); assert.equal(f([]).length,0);")
    js("dedup_id", "Remove duplicate object ids. Keep the first record for each id and preserve its order.", (
        "function f(xs) { return xs.filter((x,i)=>xs.findIndex(y=>y.id===x.id)===i); }",
        "function f(xs) { return [...new Map(xs.map(x=>[x.id,x])).values()]; }",
        "function f(xs) { return [...new Set(xs)]; }",
        "function f(xs) { return xs.filter((x,i)=>i===0); }"),
       "const a={id:2,v:1}, b={id:1,v:2}, c={id:2,v:3}; const y=f([a,b,c]); assert.equal(y.length,2); assert.equal(y[0],a); assert.equal(y[1],b);")
    js("literal_text", "Replace an element's contents with literal user text. HTML-looking text must not create elements.", (
        "function f(el,s) { el.textContent=s; }", "function f(el,s) { el.innerHTML=s; }",
        "function f(el,s) { el.setAttribute('textContent',s); }", "function f(el,s) { el.append(s); }"),
       "const el=document.createElement('div'); el.textContent='old'; const s='<b>hello</b>'; f(el,s); assert.equal(el.textContent,s); assert.equal(el.children.length,0);", dom=True)
    js("prevent_submit", "Install a submit listener that prevents the browser default action without stopping event propagation.", (
        "function f(form) { form.addEventListener('submit',e=>e.preventDefault()); }",
        "function f(form) { form.addEventListener('submit',e=>false); }",
        "function f(form) { form.addEventListener('submit',e=>e.stopPropagation()); }",
        "function f(form) { form.addEventListener('click',e=>e.preventDefault()); }"),
       "const form=document.createElement('form'); document.body.append(form); let bubbled=0; document.body.addEventListener('submit',()=>bubbled++); f(form); const e=new Event('submit',{cancelable:true,bubbles:true}); form.dispatchEvent(e); assert.equal(e.defaultPrevented,true); assert.equal(bubbled,1);", dom=True)
    js("once_listener", "Call a callback on the first click only. Later clicks must not call it again.", (
        "function f(el,cb) { el.addEventListener('click',cb,{once:true}); }",
        "function f(el,cb) { el.addEventListener('click',cb); }",
        "function f(el,cb) { el.removeEventListener('click',cb); }",
        "function f(el,cb) { cb(); el.addEventListener('click',cb,{once:true}); }"),
       "const el=document.createElement('button'); let n=0; f(el,()=>n++); assert.equal(n,0); el.dispatchEvent(new Event('click')); el.dispatchEvent(new Event('click')); assert.equal(n,1);", dom=True)
    js("class_state", "Make the active class match the boolean enabled flag. Repeating the same call must not change the state.", (
        "function f(el,enabled) { el.classList.toggle('active',enabled); }",
        "function f(el,enabled) { el.classList.toggle('active'); }",
        "function f(el,enabled) { if(enabled) el.classList.add('active'); }",
        "function f(el,enabled) { el.classList.toggle('active',!enabled); }"),
       "const el=document.createElement('div'); f(el,true); f(el,true); assert.equal(el.classList.contains('active'),true); f(el,false); assert.equal(el.classList.contains('active'),false);", dom=True)
    js("dataset_number", "Read data-@KEYTEXT@ as a number. Use Number conversion, including exponent notation and NaN for malformed numbers.", (
        "function f(el) { return Number(el.dataset[@K@]); }",
        "function f(el) { return el.dataset[@K@]; }",
        "function f(el) { return parseInt(el.dataset[@K@],10); }",
        "function f(el) { return parseFloat(el.dataset[@K@]); }"),
       "const el=document.createElement('div'); el.dataset[@K@]='2e2'; assert.equal(f(el),200); el.dataset[@K@]='2px'; assert.equal(Number.isNaN(f(el)),true);", dom=True)
    js("css_selector", "Return the first element matching a CSS selector, or null when none matches.", (
        "function f(selector) { return document.querySelector(selector); }",
        "function f(selector) { return document.getElementById(selector); }",
        "function f(selector) { return document.querySelectorAll(selector); }",
        "function f(selector) { return document.querySelector('selector'); }"),
       "document.body.innerHTML='<div class=hit></div><div class=hit></div>'; assert.equal(f('.hit'),document.body.firstElementChild); assert.equal(f('.missing'),null);", dom=True)
    js("loader_error", "Resolve to {ok:true,data} after a successful loader call, or {ok:false} when it rejects.", (
        "async function f(load) { try { return {ok:true,data:await load()}; } catch { return {ok:false}; } }",
        "async function f(load) { return {ok:true,data:await load()}; }",
        "async function f(load) { try { return {ok:true,data:load()}; } catch { return {ok:false}; } }",
        "async function f(load) { return {ok:false}; }"),
       "const good=await f(async()=>@N@); assert.equal(good.ok,true); assert.equal(good.data,@N@); const bad=await f(async()=>{throw new Error('test')}); assert.equal(bad.ok,false);")

    js("checkbox_value", "Read the current boolean checked state of a checkbox, including changes made after page load.", (
        "function f(el) { return el.checked; }", "function f(el) { return el.value; }",
        "function f(el) { return el.hasAttribute('checked'); }", "function f(el) { return Boolean(el.getAttribute('checked')); }"),
       "const el=document.createElement('input'); el.type='checkbox'; el.checked=true; assert.equal(f(el),true); el.setAttribute('checked',''); el.checked=false; assert.equal(f(el),false);", held=True, dom=True)
    js("event_delegation", "Find the clicked item's data-id through nested child elements. The item must be inside the supplied root.", (
        "function f(root,e) { const item=e.target.closest('[data-id]'); return item&&root.contains(item)?item.dataset.id:null; }",
        "function f(root,e) { return e.target.dataset.id??null; }",
        "function f(root,e) { return root.querySelector('[data-id]')?.dataset.id??null; }",
        "function f(root,e) { return e.target.parentElement?.dataset.id??null; }"),
       "const root=document.createElement('div'); root.innerHTML='<button data-id=a></button><button data-id=b><span><b>hit</b></span></button>'; assert.equal(f(root,{target:root.querySelector('b')}),'b'); assert.equal(f(root,{target:root}),null);", held=True, dom=True)
    js("functional_state", "Queue a React-style count increment of @N@. Repeated queued calls must each use the latest prior state.", (
        "function f(setCount,count) { setCount(previous=>previous+@N@); }",
        "function f(setCount,count) { setCount(count+@N@); }",
        "function f(setCount,count) { count+=@N@; }",
        "function f(setCount,count) { setCount(()=>@N@); }"),
       "const queue=[]; const setCount=x=>queue.push(x); f(setCount,1); f(setCount,1); let value=1; for(const x of queue) value=typeof x==='function'?x(value):x; assert.equal(value,1+2*@N@);", held=True)
    js("json_roundtrip", "Store an object as JSON and read it back with the same values and types.", (
        "function f(storage,x) { storage.setItem(@K@,JSON.stringify(x)); return JSON.parse(storage.getItem(@K@)); }",
        "function f(storage,x) { storage.setItem(@K@,x); return storage.getItem(@K@); }",
        "function f(storage,x) { storage.setItem(@K@,JSON.stringify(x)); return storage.getItem(@K@); }",
        "function f(storage,x) { storage.setItem(@K@,String(x)); return JSON.parse(storage.getItem(@K@)); }"),
       "const storage={value:null,setItem(k,v){this.value=String(v)},getItem(k){return this.value}}; assert.equal(JSON.stringify(f(storage,{n:@N@,ok:false})),JSON.stringify({n:@N@,ok:false}));", held=True)
    return result
