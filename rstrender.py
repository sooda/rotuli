#!/usr/bin/python3
from docutils.core import publish_parts
from sys import stdin

overrides = {
    "initial_header_level": 3,
}

part = publish_parts(stdin.read(), writer_name="html", settings_overrides=overrides)["body"]
print(part)
