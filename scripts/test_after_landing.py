"""The tier's phase selection, through its real shell entry point."""

from collections import Counter
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("after-landing.sh")
WHOLE = ["process-lab", "process-programs", "process-build", "process-tests",
         "qa-bundle", "qa-build", "qa-tests", "browser-tests"]
TEST_PHASES = {"process-lab", "process-tests", "qa-tests", "browser-tests"}
BINARY = "fixture::story"
TEST = "chosen_test"
STARVED = "skein-world: starved: waited 10 ms runnable, used 1 ms of CPU, patience 5 ms"

LOG = """
import os
from pathlib import Path

def log(phase):
    with Path(os.environ["PHASE_LOG"]).open("a") as stream:
        stream.write(phase + "\\n")
"""

CARGO = LOG + """
import json
import sys
import xml.etree.ElementTree as ET

arguments = sys.argv[1:]
story = json.loads(Path(os.environ["STORY"]).read_text())
with Path(os.environ["CARGO_LOG"]).open("a") as stream:
    stream.write(json.dumps(arguments) + "\\n")
if arguments[0] == "metadata":
    package = {"id": "fixture", "name": "fixture", "targets": [
        {"name": name, "kind": ["test"], "test": True}
        for name in story["targets"]]}
    print(json.dumps({"packages": [package], "workspace_members": ["fixture"]}))
elif arguments[0] == "build":
    log("process-programs")
elif arguments[:2] == ["nextest", "run"]:
    profile = arguments[arguments.index("--profile") + 1]
    if "--no-run" in arguments:
        log(profile + "-build")
    else:
        log(profile + "-tests")
        suite = ET.Element("testsuite")
        failed = False
        for binary, name, fails in story["cases"][profile]:
            if "-E" in arguments:
                expression = arguments[arguments.index("-E") + 1]
                matcher = "(binary_id(=" + binary + ") & test(=" + name + "))"
                if matcher not in expression:
                    continue
            case = ET.SubElement(suite, "testcase", classname=binary, name=name)
            if fails:
                ET.SubElement(case, "failure")
                failed = True
            for tag, text in story.get("output", {}).get(name, {}).items():
                ET.SubElement(case, tag).text = text
        report = Path(os.environ["CARGO_TARGET_DIR"]) / "nextest" / profile / "after-landing.xml"
        report.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(suite).write(report)
        sys.exit(1 if failed else 0)
else:
    raise AssertionError(arguments)
"""


class AfterLanding(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix=".test-after-landing-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "scripts").mkdir()
        shutil.copyfile(SCRIPT, self.root / "scripts/after-landing.sh")
        (self.root / "bin").mkdir()
        for tool in ("python3", "dirname"):
            (self.root / "bin" / tool).symlink_to(shutil.which(tool))
        self.executable("bin/cargo", CARGO)
        self.executable("scripts/bundle.sh", LOG + '\nlog("qa-bundle")\n')
        lab = self.root / "lab/tests/process/test_stand_in.py"
        lab.parent.mkdir(parents=True)
        lab.write_text(LOG + """
import unittest

class Process(unittest.TestCase):
    def test_stand_in(self):
        log("process-lab")
""")
        self.env = {"PATH": str(self.root / "bin"),
                    "PHASE_LOG": str(self.root / "phases"),
                    "CARGO_LOG": str(self.root / "cargo-arguments"),
                    "STORY": str(self.root / "story.json")}
        for variable in ("SKEIN_TEST_CHROMIUM", "TEMPER_FORGEJO", "TEMPER_FORGEJO_RUNNER"):
            self.executable("bin/" + variable, "pass\n")
            self.env[variable] = str(self.root / "bin" / variable)
        self.story = {"targets": ["story"],
                      "cases": {profile: [[BINARY, TEST, False]]
                                for profile in ("process", "qa", "browser")}}

    def executable(self, name, source):
        path = self.root / name
        path.write_text(f"#!{sys.executable}\n" + source)
        path.chmod(0o755)

    def run_script(self, *arguments):
        (self.root / "story.json").write_text(json.dumps(self.story))
        for name in ("phases", "cargo-arguments"):
            (self.root / name).unlink(missing_ok=True)
        result = subprocess.run(["/bin/sh", "scripts/after-landing.sh", *arguments],
                                cwd=self.root, env=self.env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                timeout=10)
        log = self.root / "phases"
        phases = log.read_text().splitlines() if log.exists() else []
        return result.returncode, result.stdout, phases

    def test_whole_runs_every_phase(self):
        code, output, phases = self.run_script()
        self.assertEqual(code, 0, output)
        self.assertEqual(phases, WHOLE)

    def test_two_parts(self):
        rows = [
            ("1/2", ["process-programs", "qa-bundle", "qa-build", "qa-tests", "browser-tests"]),
            ("2/2", ["process-lab", "process-programs", "process-build", "process-tests"]),
        ]
        for part, expected in rows:
            with self.subTest(part=part):
                code, output, phases = self.run_script("--part", part)
                self.assertEqual(code, 0, output)
                self.assertEqual(phases, expected)

    def test_parts_are_disjoint_and_whole(self):
        for count in range(1, 6):
            with self.subTest(count=count):
                seen = Counter()
                for number in range(1, count + 1):
                    part = f"{number}/{count}"
                    code, output, phases = self.run_script("--part", part)
                    self.assertEqual(code, 0, f"{part}: {output}")
                    seen.update(phase for phase in phases if phase in TEST_PHASES)
                self.assertEqual(seen, Counter({phase: 1 for phase in TEST_PHASES}))

    def test_one_part_is_the_whole(self):
        code, output, whole = self.run_script()
        self.assertEqual(code, 0, output)
        code, output, phases = self.run_script("--part", "1/1")
        self.assertEqual(code, 0, output)
        self.assertEqual(phases, whole)

    def test_an_empty_part_is_green(self):
        (self.root / "bin/cargo").unlink()
        for variable in ("SKEIN_TEST_CHROMIUM", "TEMPER_FORGEJO", "TEMPER_FORGEJO_RUNNER"):
            Path(self.env.pop(variable)).unlink()
        (self.root / "target").symlink_to(self.root / "unused-target")
        self.assertEqual(self.run_script("--part", "5/5"), (0, "", []))

    def test_only_ignores_the_part(self):
        # An unrelated failing case would turn an unfiltered QA run red.
        self.story["cases"]["qa"].append([BINARY, "other_test", True])
        code, output, phases = self.run_script("--part", "2/2", "--only", f"qa {BINARY} {TEST}")
        self.assertEqual(code, 0, output)
        self.assertEqual(phases, ["process-programs", "qa-bundle", "qa-build", "qa-tests"])
        commands = [json.loads(line) for line in (self.root / "cargo-arguments").read_text().splitlines()]
        command = commands[-1]
        self.assertEqual(command[command.index("-E") + 1],
                         f"(binary_id(={BINARY}) & test(={TEST}))")

    def test_a_part_names_its_failures(self):
        self.story["cases"]["qa"] = [[BINARY, TEST, True]]
        code, output, _ = self.run_script("--part", "1/2")
        self.assertEqual(code, 1, output)
        self.assertEqual(output.splitlines(), [f"after-landing: failed qa {BINARY} {TEST}"])

    def test_a_starved_pass_is_named(self):
        self.story["output"] = {TEST: {"system-err": "before\n" + STARVED + "\nafter"}}
        for arguments in (("--part", "1/2"), ("--only", f"qa {BINARY} {TEST}")):
            with self.subTest(arguments=arguments):
                code, output, _ = self.run_script(*arguments)
                self.assertEqual(code, 0, output)
                profiles = ("browser", "qa") if "--part" in arguments else ("qa",)
                self.assertEqual(output.splitlines(),
                                 [f"after-landing: starved {profile} {BINARY} {TEST}"
                                  for profile in profiles])

    def test_only_a_pass_is_starved(self):
        rows = [
            (True, {"system-err": STARVED}, [f"after-landing: failed qa {BINARY} {TEST}"]),
            (False, {"system-out": "mentioned " + STARVED}, []),
            (False, {"system-out": STARVED, "skipped": ""}, []),
        ]
        for fails, output, expected in rows:
            with self.subTest(fails=fails, output=output):
                self.story["cases"]["qa"] = [[BINARY, TEST, fails]]
                self.story["output"] = {TEST: output}
                code, output, _ = self.run_script("--only", "qa-tests")
                self.assertEqual(code, 1 if fails else 0, output)
                self.assertEqual(output.splitlines(), expected)

    def test_a_red_run_names_starved_passes_after_failures(self):
        self.story["cases"]["qa"].append([BINARY, "other_test", True])
        self.story["output"] = {TEST: {"system-out": STARVED}}
        code, output, _ = self.run_script("--only", "qa-tests")
        self.assertEqual(code, 1, output)
        self.assertEqual(output.splitlines(), [f"after-landing: failed qa {BINARY} other_test",
                                              f"after-landing: starved qa {BINARY} {TEST}"])

    def test_a_part_checks_only_its_tools(self):
        Path(self.env.pop("SKEIN_TEST_CHROMIUM")).unlink()
        code, output, phases = self.run_script("--part", "2/2")
        self.assertEqual(code, 0, output)
        self.assertEqual(phases, ["process-lab", "process-programs", "process-build", "process-tests"])
        self.assertEqual(self.run_script("--part", "1/2"),
                         (3, "after-landing: lacks Chromium\n", []))

    def test_a_bad_part_is_refused(self):
        rows = [("--part", part) for part in ("0/2", "3/2", "1/0", "2", "a/b", "01/2")]
        rows += [("--part", "1/2", "--part", "2/2"),
                 ("--part", "0/2", "--only", f"qa {BINARY} {TEST}")]
        for arguments in rows:
            with self.subTest(arguments=arguments):
                code, output, phases = self.run_script(*arguments)
                self.assertEqual(code, 2, output)
                self.assertIn("error:", output)
                self.assertEqual(phases, [])


if __name__ == "__main__":
    unittest.main()
