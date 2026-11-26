"""Reject direct research filesystem deletion outside the checked cache entry point."""

import ast
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ENTRY_FILE = 'research/seamone_research/cache.py'
ENTRY_NAME = 'remove_inside'
ENTRY = f'seamone_research.cache.{ENTRY_NAME}(root, target), defined in {ENTRY_FILE}'
FILESYSTEM = {'os', 'posix', 'nt', 'shutil', 'pathlib'}
DELETIONS = {'shutil.move', 'os.remove', 'os.removedirs', 'posix.remove', 'nt.remove'}
METHODS = {'unlink', 'rmdir', 'rmtree'}
BUILTINS = {'getattr', '__import__', 'vars', 'list', 'set', 'dict', 'tuple', 'str'}
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
        if isinstance(node, ast.Name):
            return self.bindings.get(node.id, 'builtins.' + node.id if node.id in BUILTINS else '?' + node.id)
        if isinstance(node, (ast.List, ast.ListComp)):
            return 'builtins.list'
        if isinstance(node, (ast.Set, ast.SetComp)):
            return 'builtins.set'
        if isinstance(node, (ast.Dict, ast.DictComp)):
            return 'builtins.dict'
        if isinstance(node, ast.Attribute):
            base = self.symbol(node.value)
            return base if node.attr == '__dict__' else base + '.' + node.attr
        if isinstance(node, ast.Subscript):
            key = literal(node.slice)
            return self.symbol(node.value) + '.' + key if key is not None else '?subscript'
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
            if function in COLLECTIONS or function == 'pathlib.Path':
                return function
        return '?expression'

    def reject(self, node, reason):
        if self.path == ENTRY_FILE and self.scope == [ENTRY_NAME]:
            return
        self.errors[(node.lineno, node.col_offset)] = (
            f'{self.path}:{node.lineno}:{node.col_offset + 1}: {reason}; use {ENTRY}'
        )

    def check_symbol(self, node, symbol):
        base, _, name = symbol.rpartition('.')
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
        if literal(node.slice) is None and base.split('.')[0] in FILESYSTEM:
            self.reject(node, 'dynamic filesystem callable lookup is forbidden')
        self.generic_visit(node)

    def visit_Call(self, node):
        function = self.symbol(node.func)
        self.check_symbol(node, function)
        self.check_symbol(node, self.symbol(node))
        if function in {'builtins.__import__', 'importlib.import_module'}:
            if not node.args or literal(node.args[0]) is None:
                self.reject(node, 'dynamic imports cannot establish a safe deletion boundary')
        if function == 'builtins.getattr' and len(node.args) >= 2:
            base = self.symbol(node.args[0])
            if literal(node.args[1]) is None and (base.startswith('?') or base.split('.')[0] in FILESYSTEM):
                self.reject(node, 'dynamic filesystem attribute lookup is forbidden')
        if isinstance(node.func, ast.Attribute) and node.func.attr == '__getattribute__':
            if not node.args or literal(node.args[0]) is None:
                self.reject(node, 'dynamic filesystem attribute lookup is forbidden')
        if isinstance(node.func, ast.Subscript) and literal(node.func.slice) is None:
            base = self.symbol(node.func.value)
            if base.split('.')[0] in FILESYSTEM:
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
    return [checker.errors[key] for key in sorted(checker.errors)]


def main():
    names = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', 'research'], cwd=ROOT
    ).decode().split('\0')
    errors = []
    for name in sorted(set(names)):
        path = ROOT / name
        if path.suffix == '.py' and path.is_file():
            errors.extend(violations(path.read_text(encoding='utf-8'), name))
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == '__main__':
    sys.exit(main())
