"""Authored browser/async diagnostic cases; never used to tune weights or gates."""
import json
import random
from datetime import datetime,timezone

from ..paths import ROOT,read_json,write_json,sha256
from .fixtures import Fixture
from .data import instantiate,check_cases
from .ranker_data import base_row,populate

DATA=ROOT/"data/som/research/ranker-browser"


def fixtures():
    values=[]
    def add(name,requirement,sources,checks,dom=False):
        values.append(Fixture("browser_"+name,"frontend",requirement,tuple(sources),checks,True,dom))
    add("debounce_latest","Return a debounced callback. Use injected schedule(callback) and cancel(handle). Each call replaces the pending call. Only the latest arguments reach emit when its scheduled callback runs.",(
        "function f(emit,schedule,cancel){let id;return (...args)=>{if(id!==undefined)cancel(id);id=schedule(()=>{id=undefined;emit(...args)});};}",
        "function f(emit,schedule,cancel){return (...args)=>schedule(()=>emit(...args));}",
        "function f(emit,schedule,cancel){let id;return (...args)=>{if(id!==undefined)return;id=schedule(()=>{id=undefined;emit(...args)});};}",
        "function f(emit,schedule,cancel){let id;return (...args)=>{if(id!==undefined)cancel(id);id=schedule(()=>{id=undefined;emit(args)});};}"),
        "let seq=0;const jobs=new Map(),out=[];const call=f((...a)=>out.push(a),cb=>{const id=seq++;jobs.set(id,cb);return id},id=>jobs.delete(id));call('old',1);call('new',@N@);assert.equal(jobs.size,1);for(const cb of jobs.values())cb();jobs.clear();assert.equal(JSON.stringify(out),JSON.stringify([['new',@N@]]));call('third');for(const cb of jobs.values())cb();assert.equal(out.length,2);")
    add("latest_response","Return an async request function. Every call invokes load(key). Only the most recently STARTED request may call render when it resolves. Older requests must not overwrite newer results, even if they finish later.",(
        "function f(load,render){let sequence=0;return async key=>{const mine=++sequence;const value=await load(key);if(mine===sequence)render(value);};}",
        "function f(load,render){return async key=>render(await load(key));}",
        "function f(load,render){return async key=>{let sequence=0;const mine=++sequence;const value=await load(key);if(mine===sequence)render(value);};}",
        "function f(load,render){let called=false;return async key=>{if(called)return;called=true;render(await load(key));};}"),
        "const pending={},out=[];const call=f(key=>new Promise(resolve=>pending[key]=resolve),x=>out.push(x));const first=call('a'),second=call('b');assert.equal(typeof pending.b,'function');pending.b('new');await second;pending.a('old');await first;assert.equal(JSON.stringify(out),JSON.stringify(['new']));const third=call('c');assert.equal(typeof pending.c,'function');pending.c('third');await third;assert.equal(out[1],'third');")
    add("focus_wrap","Move focus to the next enabled button after current in root. Wrap to the first enabled button at the end. Set only that button tabIndex to 0; set all other buttons to -1. Return the selected button, or null if none are enabled.",(
        "function f(root,current){const all=[...root.querySelectorAll('button')],active=all.filter(b=>!b.disabled);if(!active.length)return null;const next=active[(active.indexOf(current)+1)%active.length];for(const b of all)b.tabIndex=b===next?0:-1;next.focus();return next;}",
        "function f(root,current){const all=[...root.querySelectorAll('button')];if(!all.length)return null;const next=all[(all.indexOf(current)+1)%all.length];for(const b of all)b.tabIndex=b===next?0:-1;next.focus();return next;}",
        "function f(root,current){const all=[...root.querySelectorAll('button')],active=all.filter(b=>!b.disabled);if(!active.length)return null;const next=active[Math.min(active.indexOf(current)+1,active.length-1)];for(const b of all)b.tabIndex=b===next?0:-1;next.focus();return next;}",
        "function f(root,current){const all=[...root.querySelectorAll('button')],active=all.filter(b=>!b.disabled);if(!active.length)return null;const next=active[(active.indexOf(current)+1)%active.length];next.tabIndex=0;next.focus();return next;}"),
        "const root=document.createElement('div');root.innerHTML='<button>A</button><button disabled>B</button><button>C</button>';document.body.append(root);const [a,b,c]=root.children;assert.equal(f(root,a),c);assert.equal(document.activeElement,c);assert.equal(a.tabIndex,-1);assert.equal(b.tabIndex,-1);assert.equal(f(root,c),a);for(const x of root.children)x.disabled=true;assert.equal(f(root,a),null);",True)
    add("clone_template","Append a deep clone of template.content to target for each call. Preserve the original template for reuse. Replace the clone's [data-label] text with label as literal text, never HTML.",(
        "function f(template,target,label){const part=template.content.cloneNode(true);part.querySelector('[data-label]').textContent=label;target.append(part);}",
        "function f(template,target,label){const part=template.content;part.querySelector('[data-label]').textContent=label;target.append(part);}",
        "function f(template,target,label){const part=template.content.cloneNode(true);part.querySelector('[data-label]').innerHTML=label;target.append(part);}",
        "function f(template,target,label){const part=template.content.cloneNode(false);part.querySelector('[data-label]').textContent=label;target.append(part);}"),
        "const template=document.createElement('template');template.innerHTML='<section><span data-label>old</span></section>';const target=document.createElement('div');f(template,target,'<b>@N@</b>');assert.equal(template.content.childNodes.length,1);assert.equal(template.content.querySelector('span').textContent,'old');assert.equal(target.querySelector('b'),null);assert.equal(target.querySelector('span').textContent,'<b>@N@</b>');f(template,target,'second');assert.equal(target.children.length,2);",True)
    add("restore_focus","Open the panel by clearing hidden and focus its first button. Return a cleanup function that hides the panel and restores focus to the element that was active BEFORE opening. Restore focus only if that element is still connected.",(
        "function f(panel){const previous=panel.ownerDocument.activeElement;panel.hidden=false;panel.querySelector('button').focus();return ()=>{panel.hidden=true;if(previous?.isConnected)previous.focus();};}",
        "function f(panel){panel.hidden=false;panel.querySelector('button').focus();const previous=panel.ownerDocument.activeElement;return ()=>{panel.hidden=true;if(previous?.isConnected)previous.focus();};}",
        "function f(panel){panel.hidden=false;panel.querySelector('button').focus();return ()=>{panel.hidden=true;};}",
        "function f(panel){const previous=panel.ownerDocument.activeElement;panel.hidden=false;panel.querySelector('button').focus();return ()=>{if(previous?.isConnected)previous.focus();};}"),
        "const trigger=document.createElement('button'),panel=document.createElement('div');panel.hidden=true;panel.innerHTML='<button>inside</button>';document.body.append(trigger,panel);trigger.focus();const close=f(panel);assert.equal(panel.hidden,false);assert.equal(document.activeElement,panel.firstChild);close();assert.equal(panel.hidden,true);assert.equal(document.activeElement,trigger);trigger.focus();const second=f(panel);trigger.remove();second();assert.equal(panel.hidden,true);",True)
    add("optimistic_rollback","Read the previous value with get(), set next immediately, then await save(next). On success, keep next and return true. On failure, restore the previous value and return false. Handle falsy previous values exactly.",(
        "async function f(get,set,save,next){const previous=get();set(next);try{await save(next);return true;}catch{set(previous);return false;}}",
        "async function f(get,set,save,next){set(next);const previous=get();try{await save(next);return true;}catch{set(previous);return false;}}",
        "async function f(get,set,save,next){const previous=get();set(next);try{await save(next);return true;}catch{set(previous||null);return false;}}",
        "async function f(get,set,save,next){const previous=get();set(next);try{await save(next);set(previous);return true;}catch{set(previous);return false;}}"),
        "let value=0,seen;assert.equal(await f(()=>value,x=>value=x,async x=>{seen=value;throw new Error('offline')},@N@),false);assert.equal(seen,@N@);assert.equal(value,0);assert.equal(await f(()=>value,x=>value=x,async x=>{assert.equal(x,@N@)},@N@),true);assert.equal(value,@N@);")
    return values


def prepare():
    from transformers import AutoTokenizer
    from .modern_assets import MODEL_PATH
    from .ranker_input import check_request,prompt
    rng=random.Random(51); cases=[]; planned=[]
    for fixture in fixtures():
        for i in range(8):
            case=instantiate(fixture,i,rng); cases.append(case)
            planned.append(base_row(case,0,i<3,rng))
    outcomes=check_cases(cases)
    records=populate(planned,{c["id"]:c for c in cases},rng)
    tokenizer=AutoTokenizer.from_pretrained(MODEL_PATH,local_files_only=True)
    for row in records:
        check_request(row,tokenizer)
        for c in row["candidates"]:
            if c["id"]!="__review__":
                prompt(row["state"],row["question"],c["text"],tokenizer)
    write_json(DATA/"cases.json",cases); write_json(DATA/"rows.json",records); write_json(DATA/"outcomes.json",outcomes)
    write_json(DATA/"manifest.json",{"created_utc":datetime.now(timezone.utc).isoformat(),"rows":len(records),"families":6,
        "hashes":{str(p.relative_to(ROOT)):sha256(p) for p in (DATA/"cases.json",DATA/"rows.json",DATA/"outcomes.json",ROOT/"som/developer/ranker_browser.py")},
        "role":"Supplemental authored diagnostic, frozen before the formal v4 run's first optimizer update and before model evaluation on these cases. The separate 32-example smoke test had already finished. Not used for training, calibration, checkpoint selection, or changing acceptance gates.",
        "limits":"Six authored mechanisms with eight related examples each. jsdom is not a real-browser or full-application acceptance test."})
    print(json.dumps({"status":"passed","rows":len(records),"families":6}),flush=True)


def rows():
    for p,digest in read_json(DATA/"manifest.json")["hashes"].items():
        if sha256(ROOT/p)!=digest:
            raise ValueError("Browser diagnostic changed.")
    return read_json(DATA/"rows.json")


if __name__=="__main__":
    prepare()
