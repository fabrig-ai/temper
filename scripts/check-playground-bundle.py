"""Check a built static playground (playground.md, 7 and 9), without a browser.

    heavy scripts/bundle.sh --playground target/playground-bundle
    python3 -I -B scripts/check-playground-bundle.py target/playground-bundle
"""
import hashlib
from html.parser import HTMLParser
from pathlib import Path
import re
import sys


class References(HTMLParser):
    def __init__(self):
        super().__init__()
        self.paths = []

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name in ("src", "href") and value.startswith("/bundle/"):
                self.paths.append(value)


def check(out):
    shell = Path(__file__).resolve().parent.parent / "crates/temper-client-shell-web"
    directories = [path for path in out.iterdir() if path.is_dir()]
    assert len(directories) == 1, "one digest directory"
    directory, = directories
    assert directory.is_dir() and re.fullmatch(r"[0-9a-f]{16}", directory.name), "bundle digest"
    prefix = f"/bundle/{directory.name}/"
    page = (out / "index.html").read_text()
    assert "{bundle}" not in page, "page markers replaced"
    assert page == (shell / "playground/index.html").read_text().replace("{bundle}", prefix), "playground page"
    headers = (out / "headers").read_text()
    assert headers == (shell / "playground/headers").read_text().replace("{bundle}", prefix), "playground headers"

    modules = ("temper_client_shell_web", "temper_playground")
    stylesheets = ("skein-ui.css", "temper.css", "task-pages.css", "playground.css")
    expected = {"boot.js", "worker.js", *stylesheets}
    for module in modules:
        expected.update((f"{module}.js", f"{module}_bg.wasm"))
        assert (directory / f"{module}_bg.wasm").read_bytes().startswith(b"\0asm\x01\0\0\0"), module
        assert f"{module}_bg.wasm" in (directory / f"{module}.js").read_text(), "glue finds its module"
    for starter, module in zip(("boot.js", "worker.js"), modules):
        assert (directory / starter).read_text() == f'import init from "./{module}.js";\ninit();\n', starter
    for stylesheet in stylesheets[1:]:
        assert (directory / stylesheet).read_bytes() == (shell / stylesheet).read_bytes(), stylesheet
    for source in (shell / "files").rglob("*"):
        if source.is_file():
            name = "files/" + source.relative_to(shell / "files").as_posix()
            expected.add(name)
            assert (directory / name).read_bytes() == source.read_bytes(), name
    actual = {path.relative_to(out).as_posix() for path in out.rglob("*") if path.is_file()}
    wanted = {"index.html", "headers"} | {f"{directory.name}/{name}" for name in expected}
    assert actual == wanted, f"file list: missing {wanted - actual}, extra {actual - wanted}"

    references = References()
    references.feed(page)
    assert references.paths, "page references its bundle"
    for reference in references.paths:
        assert reference.startswith(prefix) and (out / reference.removeprefix("/bundle/")).is_file(), reference

    digest = hashlib.sha256()
    for name in sorted(expected):
        data = (directory / name).read_bytes()
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(len(data).to_bytes(8, "little"))
        digest.update(data)
    assert digest.hexdigest()[:16] == directory.name, "digest covers every bundled file"
    print(f"playground bundle: {len(actual)} files; page references resolve through {prefix}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python3 -I -B scripts/check-playground-bundle.py OUT")
    check(Path(sys.argv[1]))
