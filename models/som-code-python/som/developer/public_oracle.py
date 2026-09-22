"""Execution checks for inspected, pinned HumanEvalPack functions, never API input.

The syntax checks restrict these fixtures. They are not a security sandbox.
"""
import ast
import builtins
import contextlib
import io
import json
import random
import signal
import sys

MODULES={"typing","math","collections","random","copy","string","re","hashlib"}
BLOCKED={"open","eval","exec","compile","input","globals","locals","vars","dir","getattr","setattr","delattr","breakpoint","exit","quit"}


def inspect_python(source):
    tree=ast.parse(source)
    for node in ast.walk(tree):
        if isinstance(node,ast.Import):
            if any(a.name not in MODULES for a in node.names):
                raise ValueError("Unexpected fixture import.")
        if isinstance(node,ast.ImportFrom) and (node.level or node.module not in MODULES):
            raise ValueError("Unexpected fixture import.")
        if isinstance(node,ast.Name) and (node.id in BLOCKED or node.id.startswith("__")):
            raise ValueError("Fixture uses an unsupported dynamic or IO operation.")
        if isinstance(node,ast.Attribute) and node.attr.startswith("__"):
            raise ValueError("Fixture uses an unsupported private attribute.")
    return tree


class FixtureTimeout(BaseException):
    pass


def check_python(source,tests):
    try:
        inspect_python(source)
        inspect_python(tests)
        compiled=compile(source+"\n"+tests,"<pinned-fixture>","exec",optimize=0)
    except (SyntaxError,ValueError) as error:
        return {"passed":False,"valid_syntax":False,"error":type(error).__name__}
    safe={k:v for k,v in vars(builtins).items() if k not in BLOCKED}
    def importer(name,globals=None,locals=None,fromlist=(),level=0):
        if level or name not in MODULES:
            raise ImportError("Unsupported fixture import")
        return builtins.__import__(name,globals,locals,fromlist,level)
    safe["__import__"]=importer
    namespace={"__builtins__":safe,"__name__":"pinned_fixture"}
    def timeout(signum,frame):
        raise FixtureTimeout()
    old=signal.signal(signal.SIGALRM,timeout)
    try:
        random.seed(46)
        signal.setitimer(signal.ITIMER_REAL,.75)
        with contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):
            exec(compiled,namespace)
        return {"passed":True,"valid_syntax":True,"error":None}
    except BaseException as error:
        return {"passed":False,"valid_syntax":True,"error":type(error).__name__}
    finally:
        signal.setitimer(signal.ITIMER_REAL,0)
        signal.signal(signal.SIGALRM,old)


if __name__=="__main__":
    value=json.load(sys.stdin)
    print(json.dumps([check_python(source,value["tests"]) for source in value["sources"]]))
