"""Exercise the real finalization script with a local fake GitHub CLI."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/finalize-release.sh"


class ReleaseFinalizationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.log = self.root / "gh-call"
        gh = self.root / "gh"
        gh.write_text(
            '#!/usr/bin/env bash\n'
            'printf "%s\\n" "$@" > "$GH_CALL_LOG"\n'
            'exit "${GH_EXIT_CODE:-0}"\n'
        )
        gh.chmod(0o755)
        self.env = {
            **os.environ,
            "PATH": f"{self.root}{os.pathsep}{os.environ['PATH']}",
            "GH_CALL_LOG": str(self.log),
            "GH_EXIT_CODE": "0",
            "BUILD_RESULT": "success",
            "CRATES_RESULT": "success",
            "NPM_RESULT": "success",
            "RELEASE_TAG": "1.2.3",
            "GITHUB_REPOSITORY": "example/project",
        }

    def run_finalizer(self, **overrides):
        return subprocess.run(
            ["bash", str(SCRIPT)],
            env={**self.env, **overrides},
            capture_output=True,
            text=True,
            check=False,
        )

    def test_all_success_publishes_the_requested_release(self):
        result = self.run_finalizer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.log.read_text().splitlines(),
            ["release", "edit", "1.2.3", "--draft=false", "--repo=example/project"],
        )

    def test_unsuccessful_or_missing_upstream_result_keeps_release_draft(self):
        for job in ("BUILD_RESULT", "CRATES_RESULT", "NPM_RESULT"):
            for status in ("failure", "cancelled", "skipped", ""):
                with self.subTest(job=job, status=status):
                    result = self.run_finalizer(**{job: status})
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(self.log.exists(), "GitHub must not be called")

    def test_github_failure_is_reported_to_the_workflow(self):
        result = self.run_finalizer(GH_EXIT_CODE="17")
        self.assertEqual(result.returncode, 17)
        self.assertTrue(self.log.exists())


if __name__ == "__main__":
    unittest.main()
