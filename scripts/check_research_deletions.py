"""Reject direct research filesystem deletion outside the checked cache entry point."""

import ast
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ENTRY_FILE = 'research/seamone_research/cache.py'
ENTRY_NAME = 'remove_inside'
ENTRY = f'seamone_research.cache.{ENTRY_NAME}(root, target), defined in {ENTRY_FILE}'
FILESYSTEM = {'os', 'posix', 'nt', 'shutil', 'pathlib', 'subprocess', 'huggingface_hub'}
DELETIONS = {'shutil.move', 'os.remove', 'os.removedirs', 'posix.remove', 'nt.remove', 'os.rename', 'os.replace'}
METHODS = {'unlink', 'rmdir', 'rmtree'}
BUILTINS = {'getattr', '__import__', 'vars', 'list', 'set', 'dict', 'tuple', 'str', 'eval', 'exec', 'compile'}
COLLECTIONS = {'builtins.list', 'builtins.set', 'builtins.dict', 'builtins.tuple'}


def literal(node):
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return node.value
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
        left, right = literal(node.left), literal(node.right)
        if left is not None and right is not None:
            return left + right
    return None


class DeletionChecker(ast.NodeVisitor):
    def __init__(self, path):
        self.path = Path(path).as_posix()
        self.bindings = {}
        self.scope = []
        self.errors = {}

    def symbol(self, node):
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return 'builtins.str'
        if isinstance(node, ast.Name):
            return self.bindings.get(node.id, 'builtins' if node.id == '__builtins__' else 'builtins.' + node.id if node.id in BUILTINS else '?' + node.id)
        if isinstance(node, (ast.List, ast.ListComp)):
            return 'builtins.list'
        if isinstance(node, (ast.Set, ast.SetComp)):
            return 'builtins.set'
        if isinstance(node, (ast.Dict, ast.DictComp)):
            return 'builtins.dict'
        if isinstance(node, ast.Attribute):
            base = self.symbol(node.value)
            if base.startswith('pathlib.Path') and node.attr in {'name', 'stem', 'suffix', 'anchor', 'drive', 'root'}:
                return 'builtins.str'
            return base if node.attr == '__dict__' else base + '.' + node.attr
        if isinstance(node, ast.Subscript):
            if self.symbol(node.value) == 'pathlib.Path.parents':
                return 'pathlib.Path'
            key = literal(node.slice)
            return self.symbol(node.value) + '.' + key if key is not None else '?subscript'
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div) and self.symbol(node.left).startswith('pathlib.Path'):
            return 'pathlib.Path'
        if isinstance(node, ast.Call):
            function = self.symbol(node.func)
            if function in {'builtins.__import__', 'importlib.import_module'} and node.args:
                name = literal(node.args[0])
                if name is not None:
                    return name.split('.')[0] if function == 'builtins.__import__' else name
            if function == 'builtins.getattr' and len(node.args) >= 2:
                name = literal(node.args[1])
                if name is not None:
                    return self.symbol(node.args[0]) + '.' + name
            if isinstance(node.func, ast.Attribute) and node.func.attr == '__getattribute__' and node.args:
                name = literal(node.args[0])
                if name is not None:
                    return self.symbol(node.func.value) + '.' + name
            if function == 'builtins.vars' and node.args:
                return self.symbol(node.args[0])
            if function.startswith('pathlib.Path.'):
                name = function.rsplit('.', 1)[-1]
                if name in {'resolve', 'absolute', 'expanduser', 'with_name', 'with_suffix', 'joinpath', 'with_stem'}:
                    return 'pathlib.Path'
                if name in {'read_text', 'as_posix', 'as_uri'}:
                    return 'builtins.str'
            if function.startswith('huggingface_hub.'):
                return 'huggingface_hub.result'
            if function in COLLECTIONS or function in {'pathlib.Path', 'builtins.str'}:
                return function
        return '?expression'

    def reject(self, node, reason, *, always=False):
        if not always and self.path == ENTRY_FILE and self.scope == [ENTRY_NAME]:
            return
        self.errors[(node.lineno, node.col_offset)] = (
            f'{self.path}:{node.lineno}:{node.col_offset + 1}: {reason}; use {ENTRY}'
        )

    def check_symbol(self, node, symbol):
        base, _, name = symbol.rpartition('.')
        root = symbol.split('.')[0]
        if root == 'subprocess' or (root in {'os', 'posix', 'nt'} and (name in {'system', 'popen'} or name.startswith(('exec', 'spawn')))):
            self.reject(node, f'process execution {symbol} can bypass cache ownership', always=True)
        if root == 'huggingface_hub' and (name.startswith('delete') or name == 'execute'):
            self.reject(node, f'shared Hugging Face cache deletion {symbol} is forbidden', always=True)
        if name in {'delete_revisions', 'delete_cache', 'delete_cached_repo'}:
            self.reject(node, f'shared cache deletion {symbol} is forbidden', always=True)
        if symbol in {'builtins.eval', 'builtins.exec', 'builtins.compile'}:
            self.reject(node, f'dynamic Python execution {symbol} is forbidden', always=True)
        if name == 'rename' or (name == 'replace' and root in {'pathlib', 'os', 'posix', 'nt'}):
            if not (self.path == ENTRY_FILE and self.scope == ['publish_partial'] and name == 'replace'):
                self.reject(node, f'direct path replacement {symbol} is forbidden; use cache.publish_partial', always=True)
        if symbol in DELETIONS or name in METHODS or (name == 'remove' and base.startswith('?')):
            self.reject(node, f'direct deletion callable {symbol.lstrip("?")} is forbidden')

    def visit_Import(self, node):
        for alias in node.names:
            self.bindings[alias.asname or alias.name.split('.')[0]] = alias.name if alias.asname else alias.name.split('.')[0]

    def visit_ImportFrom(self, node):
        module = node.module or ''
        for alias in node.names:
            if alias.name == '*' and module.split('.')[0] in FILESYSTEM:
                self.reject(node, 'wildcard filesystem imports hide deletion calls')
            symbol = module + '.' + alias.name
            self.bindings[alias.asname or alias.name] = symbol
            self.check_symbol(node, symbol)

    def visit_Assign(self, node):
        self.visit(node.value)
        for target in node.targets:
            if isinstance(target, ast.Name):
                self.bindings[target.id] = self.symbol(node.value)
            else:
                self.visit(target)

    def visit_AnnAssign(self, node):
        if node.value is not None:
            self.visit(node.value)
            if isinstance(node.target, ast.Name):
                self.bindings[node.target.id] = self.symbol(node.value)

    def visit_FunctionDef(self, node):
        for decorator in node.decorator_list:
            self.visit(decorator)
        for value in [*node.args.defaults, *filter(None, node.args.kw_defaults)]:
            self.visit(value)
        outer = self.bindings
        self.bindings = outer.copy()
        for arg in [*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs]:
            annotation = arg.annotation
            if annotation is not None:
                self.visit(annotation)
            if isinstance(annotation, ast.Subscript):
                annotation = annotation.value
            inferred = self.symbol(annotation) if annotation else '?parameter'
            if isinstance(annotation, ast.BinOp) and isinstance(annotation.op, ast.BitOr):
                parts = (annotation.left, annotation.right)
                if any(self.symbol(part) == 'builtins.str' for part in parts) and any(isinstance(part, ast.Constant) and part.value is None for part in parts):
                    inferred = 'builtins.str'
            self.bindings[arg.arg] = inferred
        for arg in (node.args.vararg, node.args.kwarg):
            if arg is not None:
                self.bindings[arg.arg] = '?parameter'
        if node.returns is not None:
            self.visit(node.returns)
        self.scope.append(node.name)
        for statement in node.body:
            self.visit(statement)
        self.scope.pop()
        self.bindings = outer

    visit_AsyncFunctionDef = visit_FunctionDef

    def visit_ClassDef(self, node):
        outer = self.bindings
        self.bindings = outer.copy()
        self.scope.append(node.name)
        self.generic_visit(node)
        self.scope.pop()
        self.bindings = outer

    def visit_Lambda(self, node):
        self.scope.append('<lambda>')
        self.generic_visit(node)
        self.scope.pop()

    def visit_Name(self, node):
        if isinstance(node.ctx, ast.Load):
            self.check_symbol(node, self.symbol(node))

    def visit_Attribute(self, node):
        self.check_symbol(node, self.symbol(node))
        self.generic_visit(node)

    def visit_Subscript(self, node):
        self.check_symbol(node, self.symbol(node))
        base = self.symbol(node.value)
        if literal(node.slice) is None and base in FILESYSTEM | {'pathlib.Path'}:
            self.reject(node, 'dynamic filesystem callable lookup is forbidden')
        self.generic_visit(node)

    def visit_Call(self, node):
        function = self.symbol(node.func)
        self.check_symbol(node, function)
        self.check_symbol(node, self.symbol(node))
        publisher = self.path == ENTRY_FILE and self.scope == ['publish_partial']
        if (function.rsplit('.', 1)[-1] == 'replace' and len(node.args) < 2
                and function != 'dataclasses.replace' and not publisher):
            self.reject(node, 'one-argument replacement can move a path; use cache.publish_partial', always=True)
        if function in {'builtins.__import__', 'importlib.import_module'}:
            if not node.args or literal(node.args[0]) is None:
                self.reject(node, 'dynamic imports cannot establish a safe deletion boundary')
        if function == 'builtins.getattr' and len(node.args) >= 2:
            base = self.symbol(node.args[0])
            if literal(node.args[1]) is None and (base == 'builtins' or base.startswith('?') or base.split('.')[0] in FILESYSTEM):
                self.reject(node, 'dynamic filesystem attribute lookup is forbidden')
        if isinstance(node.func, ast.Attribute) and node.func.attr == '__getattribute__':
            if not node.args or literal(node.args[0]) is None:
                self.reject(node, 'dynamic filesystem attribute lookup is forbidden')
        if isinstance(node.func, ast.Subscript) and literal(node.func.slice) is None:
            base = self.symbol(node.func.value)
            if base in FILESYSTEM | {'pathlib.Path'}:
                self.reject(node, 'dynamic filesystem callable lookup is forbidden')
        self.generic_visit(node)


def violations(source, path):
    try:
        tree = ast.parse(source, filename=str(path))
    except SyntaxError as error:
        return [f'{path}:{error.lineno}: cannot inspect Python syntax: {error.msg}']
    checker = DeletionChecker(path)
    checker.visit(tree)
    if Path(path).as_posix() == ENTRY_FILE:
        entries = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == ENTRY_NAME]
        if len(entries) != 1:
            checker.errors[(1, 0)] = f'{path}:1: define exactly one checked {ENTRY}'
        publishers = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'publish_partial']
        if len(publishers) > 1:
            checker.errors[(1, 1)] = f'{path}:1: define only one checked cache.publish_partial helper'
    return [checker.errors[key] for key in sorted(checker.errors)]


def recipe_violations(source):
    diagnostics = []
    for number, line in enumerate(source.splitlines(), 1):
        if (line.startswith('\t') and re.search(r'(?:^|[\s/;|&])(?:rm|rmdir|unlink)(?:\s|$)', line[1:].lstrip('@-+ '))):
            diagnostics.append(f'research/Makefile:{number}: direct recipe deletion is forbidden; use seamone_research.cache.remove_inside')
    return diagnostics


def main():
    names = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', 'research'], cwd=ROOT
    ).decode().split('\0')
    errors = recipe_violations((ROOT / 'research/Makefile').read_text(encoding='utf-8'))
    for name in sorted(set(names)):
        path = ROOT / name
        if path.suffix == '.py' and path.is_file():
            errors.extend(violations(path.read_text(encoding='utf-8'), name))
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == '__main__':
    sys.exit(main())
