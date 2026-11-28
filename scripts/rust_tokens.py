"""Lex Rust declarations and attributes without interpreting comments as code."""

import re
from typing import NamedTuple


class Token(NamedTuple):
    text: str
    start: int
    end: int


def tokens(source, *, include_comments=False, normalize_raw_identifiers=False):
    position = 0
    while position < len(source):
        start = position
        if source[position].isspace():
            position += 1
            continue
        if source.startswith("//", position):
            end = source.find("\n", position)
            position = len(source) if end < 0 else end + 1
            if include_comments:
                yield Token(source[start:position].rstrip("\n"), start, position)
            continue
        if source.startswith("/*", position):
            position += 2
            depth = 1
            while position < len(source) and depth:
                if source.startswith("/*", position):
                    depth += 1
                    position += 2
                elif source.startswith("*/", position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
            if depth:
                raise ValueError("unclosed Rust comment")
            if include_comments:
                yield Token(source[start:position], start, position)
            continue
        raw = re.match(r'(?:br|cr|r)(#*)"', source[position:])
        if raw:
            closing = '"' + raw[1]
            end = source.find(closing, position + raw.end())
            if end < 0:
                raise ValueError("unclosed Rust raw string")
            position = end + len(closing)
        elif source[position] == '"':
            position += 1
            while position < len(source):
                if source[position] == "\\":
                    position += 2
                elif source[position] == '"':
                    position += 1
                    break
                else:
                    position += 1
        elif normalize_raw_identifiers and source.startswith('r#', position) and (
            identifier := re.match(r'r#([A-Za-z_]\w*)', source[position:])
        ):
            position += identifier.end()
            yield Token(identifier[1], start, position)
            continue
        else:
            char = re.match(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'", source[position:])
            word = re.match(r"[A-Za-z_]\w*|[0-9]+", source[position:])
            position += len(char[0]) if char else len(word[0]) if word else 1
        yield Token(source[start:position], start, position)


def enclosing_attribute(code, index):
    depth = 0
    for cursor in range(index - 1, -1, -1):
        text = code[cursor].text
        if text == ']':
            depth += 1
        elif text == '[':
            if depth:
                depth -= 1
            elif cursor > 0 and code[cursor - 1].text == '#':
                return cursor + 1
            elif cursor > 1 and [item.text for item in code[cursor - 2:cursor]] == ['#', '!']:
                return cursor + 1
            else:
                return None
        elif not depth and text in {';', '{', '}'}:
            return None
    return None


def module_path_attribute(code, index):
    start = enclosing_attribute(code, index)
    return start is not None and code[start].text in {'path', 'cfg_attr'}


def attributes(source):
    code = list(tokens(source))
    for start, token in enumerate(code):
        if token.text != "#":
            continue
        bracket = start + 1
        if bracket < len(code) and code[bracket].text == "!":
            bracket += 1
        if bracket >= len(code) or code[bracket].text != "[":
            continue
        depth = 1
        end = bracket + 1
        while end < len(code) and depth:
            depth += (code[end].text == "[") - (code[end].text == "]")
            end += 1
        if depth:
            raise ValueError("unclosed Rust attribute")
        content = code[bracket + 1:end - 1]
        if any(item.text in ("allow", "expect") and index + 1 < len(content)
               and content[index + 1].text == "(" for index, item in enumerate(content)):
            yield source[token.start:code[end - 1].end]
