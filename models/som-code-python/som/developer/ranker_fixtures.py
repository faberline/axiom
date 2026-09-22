"""Additional executable frontend exercises, split by whole task family."""
from .fixtures import Fixture


def fixtures():
    result=[]
    def add(name,requirement,sources,checks,split="train",dom=False):
        result.append((split,Fixture("v4_"+name,"frontend",requirement,tuple(sources),checks,split!="train",dom)))

    add("omit_key","Return a new object with own property @K@ removed. Preserve other own properties and the original object.",(
        "function f(x) { const {[@K@]:unused,...rest}=x; return rest; }",
        "function f(x) { delete x[@K@]; return x; }",
        "function f(x) { return {...x,[@K@]:undefined}; }",
        "function f(x) { return {[@K@]:x[@K@]}; }"),
        "const x={[@K@]:@N@,other:3}; const y=f(x); assert.equal(Object.hasOwn(y,@K@),false); assert.equal(y.other,3); assert.equal(x[@K@],@N@); assert.notEqual(x,y);")
    add("nested_state","Set profile.@K@ to a value. Return a new root and profile object. Preserve other data and the old state.",(
        "function f(s,v) { return {...s,profile:{...s.profile,[@K@]:v}}; }",
        "function f(s,v) { s.profile[@K@]=v; return {...s}; }",
        "function f(s,v) { return {...s,profile:{[@K@]:v}}; }",
        "function f(s,v) { return {...s,[@K@]:v}; }"),
        "const s={profile:{[@K@]:0,other:7},extra:9}; const y=f(s,@N@); assert.equal(y.profile[@K@],@N@); assert.equal(s.profile[@K@],0); assert.equal(y.profile.other,7); assert.equal(y.extra,9); assert.notEqual(y.profile,s.profile);")
    add("toggle_membership","Toggle an item in a unique array. Remove it if present, otherwise append it. Keep order and do not mutate the input.",(
        "function f(xs,x) { return xs.includes(x)?xs.filter(v=>v!==x):[...xs,x]; }",
        "function f(xs,x) { return [...xs,x]; }",
        "function f(xs,x) { const i=xs.indexOf(x); if(i<0)xs.push(x);else xs.splice(i,1); return xs; }",
        "function f(xs,x) { return xs.filter(v=>v===x); }"),
        "const xs=[1,3]; assert.equal(JSON.stringify(f(xs,3)),'[1]'); assert.equal(JSON.stringify(f(xs,2)),'[1,3,2]'); assert.equal(JSON.stringify(xs),'[1,3]');")
    add("multi_sort","Sort records by group ascending, then numeric @K@ ascending. Return a new array and preserve the input.",(
        "function f(xs) { return [...xs].sort((a,b)=>a.group.localeCompare(b.group)||a[@K@]-b[@K@]); }",
        "function f(xs) { return xs.sort((a,b)=>a.group.localeCompare(b.group)||a[@K@]-b[@K@]); }",
        "function f(xs) { return [...xs].sort((a,b)=>a[@K@]-b[@K@]); }",
        "function f(xs) { return [...xs].sort((a,b)=>a.group.localeCompare(b.group)||b[@K@]-a[@K@]); }"),
        "const xs=[{id:1,group:'b',[@K@]:0},{id:2,group:'a',[@K@]:10},{id:3,group:'a',[@K@]:2}]; const y=f(xs); assert.equal(y.map(x=>x.id).join(','),'3,2,1'); assert.equal(xs[0].id,1);")
    add("partition","Return [matching, nonmatching] arrays using a predicate. Call the predicate once per item. Preserve item order.",(
        "function f(xs,p) { const yes=[],no=[]; for(const x of xs)(p(x)?yes:no).push(x); return [yes,no]; }",
        "function f(xs,p) { return [xs.filter(p),xs.filter(x=>!p(x))]; }",
        "function f(xs,p) { const yes=[],no=[]; for(const x of xs)(p(x)?no:yes).push(x); return [yes,no]; }",
        "function f(xs,p) { return [xs,[]]; }"),
        "let calls=0; const y=f([3,2,1],x=>{calls++;return x%2===1;}); assert.equal(JSON.stringify(y),'[[3,1],[2]]'); assert.equal(calls,3);")
    add("pagination","Return page p of an array with @N@ items per page. Page numbers start at one. Do not mutate the input.",(
        "function f(xs,p) { return xs.slice((p-1)*@N@,p*@N@); }",
        "function f(xs,p) { return xs.slice(p*@N@,(p+1)*@N@); }",
        "function f(xs,p) { return xs.splice((p-1)*@N@,@N@); }",
        "function f(xs,p) { return xs.slice((p-1)*@N@,p*@N@-1); }"),
        "const xs=Array.from({length:@N@*3},(_,i)=>i); assert.equal(JSON.stringify(f(xs,2)),JSON.stringify(xs.slice(@N@,@N@*2))); assert.equal(xs.length,@N@*3); assert.equal(f(xs,5).length,0);")
    add("async_filter","Filter items with an asynchronous predicate. Wait for its results and preserve the original item order.",(
        "async function f(xs,p) { const flags=await Promise.all(xs.map(p)); return xs.filter((_,i)=>flags[i]); }",
        "async function f(xs,p) { return xs.filter(p); }",
        "async function f(xs,p) { return await Promise.all(xs.map(p)); }",
        "async function f(xs,p) { const flags=await Promise.all(xs.map(p)); return xs.filter((_,i)=>!flags[i]); }"),
        "const y=await f([3,2,1],async x=>x%2===1); assert.equal(JSON.stringify(y),'[3,1]'); assert.equal((await f([],async()=>true)).length,0);")
    add("settled_values","Run asynchronous jobs and return only fulfilled values in job order. A rejected job must not reject the whole operation. Preserve falsy values.",(
        "async function f(jobs) { const r=await Promise.allSettled(jobs.map(j=>j())); return r.filter(x=>x.status==='fulfilled').map(x=>x.value); }",
        "async function f(jobs) { return await Promise.all(jobs.map(j=>j())); }",
        "async function f(jobs) { const r=await Promise.allSettled(jobs.map(j=>j())); return r.filter(x=>x.status==='fulfilled'&&x.value).map(x=>x.value); }",
        "async function f(jobs) { return await Promise.allSettled(jobs.map(j=>j())); }"),
        "const y=await f([async()=>0,async()=>{throw Error('x');},async()=>@N@]); assert.equal(JSON.stringify(y),JSON.stringify([0,@N@]));")
    add("retry_limit","Call an asynchronous job until it succeeds, with at most @N@ attempts. Return its value. If all attempts fail, throw the last error.",(
        "async function f(job) { for(let i=0;i<@N@;i++){try{return await job();}catch(e){if(i===@N@-1)throw e;}} }",
        "async function f(job) { for(let i=0;i<@N@;i++){try{return job();}catch(e){if(i===@N@-1)throw e;}} }",
        "async function f(job) { for(let i=0;i<@N@-1;i++){try{return await job();}catch(e){if(i===@N@-2)throw e;}} }",
        "async function f(job) { for(let i=0;i<=@N@;i++){try{return await job();}catch(e){if(i===@N@)throw e;}} }"),
        "let n=0; assert.equal(await f(async()=>{if(++n<@N@)throw Error('retry');return 0;}),0); assert.equal(n,@N@); n=0; await assert.rejects(()=>f(async()=>{n++;throw Error('last');}),/last/); assert.equal(n,@N@);")
    add("literal_prefix","Remove a nonempty literal prefix once if the string starts with it. Otherwise return the original string.",(
        "function f(s,p) { return s.startsWith(p)?s.slice(p.length):s; }",
        "function f(s,p) { return s.replaceAll(p,''); }",
        "function f(s,p) { return s.slice(p.length); }",
        "function f(s,p) { return s.replace(new RegExp('^'+p),''); }"),
        "assert.equal(f('a+a+','a+'),'a+'); assert.equal(f('zzabc','abc'),'zzabc'); assert.equal(f('[x]z','[x]'),'z');")
    add("unicode_prefix","Return the first @N@ Unicode code points of a string. Do not split surrogate pairs.",(
        "function f(s) { return Array.from(s).slice(0,@N@).join(''); }",
        "function f(s) { return s.slice(0,@N@); }",
        "function f(s) { return Array.from(s).slice(1,@N@+1).join(''); }",
        "function f(s) { return s; }"),
        "const s='😀'.repeat(@N@)+'z'; assert.equal(f(s),'😀'.repeat(@N@)); assert.equal(f('a'),'a');")
    add("map_own_values","Multiply every own numeric property by @N@ in a new object. Ignore inherited properties. Do not mutate the original.",(
        "function f(x) { return Object.fromEntries(Object.entries(x).map(([k,v])=>[k,v*@N@])); }",
        "function f(x) { const y={}; for(const k in x)y[k]=x[k]*@N@; return y; }",
        "function f(x) { for(const k of Object.keys(x))x[k]*=@N@; return x; }",
        "function f(x) { return Object.values(x).map(v=>v*@N@); }"),
        "const x=Object.assign(Object.create({inherited:9}),{[@K@]:2}); const y=f(x); assert.equal(y[@K@],2*@N@); assert.equal(Object.hasOwn(y,'inherited'),false); assert.equal(x[@K@],2);")
    add("form_reset","Restore all form controls to their HTML default values and checked states.",(
        "function f(form) { form.reset(); }",
        "function f(form) { for(const e of form.elements)e.value=''; }",
        "function f(form) { for(const e of form.elements)e.checked=false; }",
        "function f(form) { return form; }"),
        "document.body.innerHTML='<form><input value=initial><input type=checkbox checked></form>'; const form=document.querySelector('form'); form.elements[0].value='changed'; form.elements[1].checked=false; f(form); assert.equal(form.elements[0].value,'initial'); assert.equal(form.elements[1].checked,true);",dom=True)
    add("disabled_state","Make a control's disabled state equal to the supplied boolean. Calling with false must enable it.",(
        "function f(e,disabled) { e.disabled=disabled; }",
        "function f(e,disabled) { e.setAttribute('disabled',String(disabled)); }",
        "function f(e,disabled) { e.disabled=!disabled; }",
        "function f(e,disabled) { e.readOnly=disabled; }"),
        "const e=document.createElement('input'); f(e,true); assert.equal(e.disabled,true); f(e,false); assert.equal(e.disabled,false);",dom=True)
    add("replace_children","Replace every existing child with the supplied nodes in order. Move the same nodes rather than converting them to text.",(
        "function f(e,nodes) { e.replaceChildren(...nodes); }",
        "function f(e,nodes) { e.append(...nodes); }",
        "function f(e,nodes) { e.textContent=nodes.join(''); }",
        "function f(e,nodes) { e.replaceChildren(nodes[0]); }"),
        "const e=document.createElement('div'); e.textContent='old'; const a=document.createElement('b'),b=document.createElement('i'); f(e,[a,b]); assert.equal(e.childNodes.length,2); assert.equal(e.firstChild,a); assert.equal(e.lastChild,b); f(e,[]); assert.equal(e.childNodes.length,0);",dom=True)
    add("expanded_aria","Set aria-expanded to the literal string true or false from a boolean. Keep the attribute present in both states.",(
        "function f(e,on) { e.setAttribute('aria-expanded',String(on)); }",
        "function f(e,on) { e.toggleAttribute('aria-expanded',on); }",
        "function f(e,on) { e.dataset.expanded=String(on); }",
        "function f(e,on) { e.setAttribute('aria-expanded',on?'1':'0'); }"),
        "const e=document.createElement('button'); f(e,true); assert.equal(e.getAttribute('aria-expanded'),'true'); f(e,false); assert.equal(e.getAttribute('aria-expanded'),'false');",dom=True)

    add("selected_values","Return values of all selected options of a multiple select, in option order.",(
        "function f(e) { return Array.from(e.selectedOptions,o=>o.value); }",
        "function f(e) { return e.value; }",
        "function f(e) { return Array.from(e.options,o=>o.value); }",
        "function f(e) { return Array.from(e.selectedOptions,o=>o.textContent); }"),
        "const e=document.createElement('select'); e.multiple=true; e.innerHTML='<option value=a selected>A</option><option value=b>B</option><option value=c selected>C</option>'; assert.equal(JSON.stringify(f(e)),'[\"a\",\"c\"]');",split="validation",dom=True)
    add("event_cleanup","Install an input listener that calls a callback. Return a cleanup function that removes exactly that listener.",(
        "function f(e,cb) { const handler=()=>cb(); e.addEventListener('input',handler); return ()=>e.removeEventListener('input',handler); }",
        "function f(e,cb) { e.addEventListener('input',()=>cb()); return ()=>e.removeEventListener('input',()=>cb()); }",
        "function f(e,cb) { const handler=()=>cb(); e.addEventListener('input',handler); return ()=>e.removeEventListener('click',handler); }",
        "function f(e,cb) { const handler=()=>cb(); e.addEventListener('click',handler); return ()=>e.removeEventListener('click',handler); }"),
        "const e=document.createElement('input'); let n=0; const cleanup=f(e,()=>n++); e.dispatchEvent(new Event('input')); assert.equal(n,1); cleanup(); e.dispatchEvent(new Event('input')); assert.equal(n,1);",split="validation",dom=True)
    add("replace_query","Set a URL's query parameter @K@ to one value. Replace duplicate occurrences, retain other parameters and the fragment.",(
        "function f(s,v) { const u=new URL(s); u.searchParams.set(@K@,v); return u.toString(); }",
        "function f(s,v) { const u=new URL(s); u.searchParams.append(@K@,v); return u.toString(); }",
        "function f(s,v) { const u=new URL(s); u.search='?'+@K@+'='+v; return u.toString(); }",
        "function f(s,v) { return s+'&'+@K@+'='+v; }"),
        "const u=new URL(f('https://example.test/p?'+@K@+'=a&'+@K@+'=b&other=x#hash','a b&c')); assert.equal(JSON.stringify(u.searchParams.getAll(@K@)),JSON.stringify(['a b&c'])); assert.equal(u.searchParams.get('other'),'x'); assert.equal(u.hash,'#hash');",split="validation")
    add("deep_freeze","Recursively freeze an acyclic plain object and every nested object. Return the original object.",(
        "function f(x) { if(x&&typeof x==='object'){for(const v of Object.values(x))f(v); Object.freeze(x);} return x; }",
        "function f(x) { return Object.freeze(x); }",
        "function f(x) { for(const v of Object.values(x))if(v&&typeof v==='object')Object.freeze(v); return x; }",
        "function f(x) { return Object.freeze({...x}); }"),
        "const x={a:{b:{[@K@]:@N@}}}; assert.equal(f(x),x); assert.equal(Object.isFrozen(x),true); assert.equal(Object.isFrozen(x.a),true); assert.equal(Object.isFrozen(x.a.b),true);",split="validation")

    add("finite_number","Return true only for finite values whose type is number. Reject numeric strings, NaN, infinities, null, and booleans.",(
        "function f(x) { return Number.isFinite(x); }",
        "function f(x) { return Number.isFinite(Number(x)); }",
        "function f(x) { return typeof x==='number'; }",
        "function f(x) { return Number.isFinite(x)&&x>=0; }"),
        "assert.equal(f(-@N@),true); assert.equal(f('2'),false); assert.equal(f(NaN),false); assert.equal(f(Infinity),false); assert.equal(f(null),false);",split="calibration")
    add("same_origin_path","Resolve href against an absolute base URL. Return its pathname if the origins match; otherwise return null.",(
        "function f(href,base) { const u=new URL(href,base); return u.origin===new URL(base).origin?u.pathname:null; }",
        "function f(href,base) { return href.startsWith(base)?new URL(href).pathname:null; }",
        "function f(href,base) { const u=new URL(href,base); return u.hostname===new URL(base).hostname?u.pathname:null; }",
        "function f(href,base) { const u=new URL(href,base); return u.origin===new URL(base).origin?u.href:null; }"),
        "const base='https://example.test/app/'; assert.equal(f('../next?q=1',base),'/next'); assert.equal(f('https://evil.test/next',base),null); assert.equal(f('http://example.test/next',base),null);",split="calibration")
    add("form_values","Return all successful form values for name @K@ as an array. Include repeated fields and checked checkboxes, but exclude unchecked checkboxes.",(
        "function f(form) { return new form.ownerDocument.defaultView.FormData(form).getAll(@K@); }",
        "function f(form) { return new form.ownerDocument.defaultView.FormData(form).get(@K@); }",
        "function f(form) { return Array.from(form.elements).filter(e=>e.name===@K@).map(e=>e.value); }",
        "function f(form) { return [new form.ownerDocument.defaultView.FormData(form).get(@K@)]; }"),
        "const form=document.createElement('form'); form.innerHTML='<input name='+@K@+' value=a><input name='+@K@+' value=b><input name='+@K@+' type=checkbox value=c checked><input name='+@K@+' type=checkbox value=d>'; assert.equal(JSON.stringify(f(form)),'[\"a\",\"b\",\"c\"]');",split="calibration",dom=True)
    add("async_cleanup","Run cleanup exactly once after an asynchronous task settles. Preserve its resolved value or rejection. Do not clean up before it settles.",(
        "async function f(task,cleanup) { try{return await task();}finally{cleanup();} }",
        "async function f(task,cleanup) { try{return task();}finally{cleanup();} }",
        "async function f(task,cleanup) { const x=await task(); cleanup(); return x; }",
        "async function f(task,cleanup) { try{return await task();}finally{return cleanup();} }"),
        "const order=[]; const result=await f(async()=>{await Promise.resolve();order.push('task');return 0;},()=>order.push('cleanup')); assert.equal(result,0); assert.equal(order.join(','),'task,cleanup'); let n=0; await assert.rejects(()=>f(async()=>{throw Error('x');},()=>n++),/x/); assert.equal(n,1);",split="calibration")
    return result
