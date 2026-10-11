"""Check the installable VSIX, including assets that TypeScript cannot check."""

import json
from pathlib import PurePosixPath
import sys
import unittest
import zipfile


class VSCodePackageTests(unittest.TestCase):
    def setUp(self):
        self.archive = zipfile.ZipFile(self.package_path)
        self.addCleanup(self.archive.close)
        self.files = set(self.archive.namelist())
        self.manifest = json.loads(self.archive.read("extension/package.json"))

    def test_minimum_editor_version_is_preserved(self):
        self.assertEqual(self.manifest["engines"]["vscode"], "^1.93.0")

    def test_entrypoint_and_runtime_dependency_are_included(self):
        entrypoint = str(PurePosixPath("extension") / self.manifest["main"])
        self.assertIn(entrypoint, self.files)
        dependency = "extension/node_modules/jsonrpc-lite/"
        manifest = json.loads(self.archive.read(dependency + "package.json"))
        self.assertIn(dependency + manifest["main"], self.files)

    def test_webview_and_activity_bar_assets_are_included(self):
        for asset in ("webview/main.js", "webview/styles.css", "resources/ck-icon.svg"):
            with self.subTest(asset=asset):
                self.assertIn("extension/" + asset, self.files)
                self.assertGreater(len(self.archive.read("extension/" + asset)), 0)

    def test_development_files_are_excluded(self):
        for name in self.files:
            self.assertFalse(name.startswith("extension/src/"), name)
            self.assertFalse(name.endswith((".ts", ".map", ".vsix")), name)
            self.assertFalse(name.startswith("extension/node_modules/@vscode/vsce/"), name)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Usage: test_vscode_package.py path/to/extension.vsix")
    VSCodePackageTests.package_path = sys.argv[1]
    unittest.main(argv=[sys.argv[0]], verbosity=2)
