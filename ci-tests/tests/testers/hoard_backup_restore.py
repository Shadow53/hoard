import os
import shutil
from pathlib import Path

from .hoard_tester import HoardTester, Environment, Hoard, HoardFile, HOARDS_DIRNAME


class HoardBackupRestoreTester(HoardTester):
    def run_test(self):
        home = Path.home()

        self.env["USE_ENV"] = "1"
        self.args = ["--force"]
        self.reset()

        # Run single backup
        self.run_hoard("backup")
        self.assert_first_tree()

        # Change file contents on system, restore, then ensure same content again
        self.setup()
        self.run_hoard("restore")
        self.assert_first_tree()

        # Change env and restore again. Now 1 and 2 files should match.
        self.env["USE_ENV"] = "2"
        self.run_hoard("restore")
        self.assert_first_tree()
        self.assert_second_tree()

        # Remove files, then backup
        os.remove(home.joinpath(f"{Environment.Second}_{HoardFile.AnonDir2}"))
        # Remove entire single-file hoard
        os.remove(home.joinpath(f"{Environment.Second}_{HoardFile.AnonFile}"))
        # Remove entire pile
        shutil.rmtree(home.joinpath(f"{Environment.Second}_{HoardFile.NamedDir1}"))
        # Remove one file from pile
        os.remove(home.joinpath(f"{Environment.Second}_{HoardFile.NamedDir22}"))

        self.run_hoard("backup")
        self.assert_second_tree()

        # Restoring should delete files from env 1
        self.env["USE_ENV"] = "1"
        self.run_hoard("restore")
        self.assert_second_tree()
        self.assert_first_tree()