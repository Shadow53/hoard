from .hoard_tester import HoardTester, HoardFile, Environment
import os
import secrets
import subprocess


class OperationCheckerTester(HoardTester):
    def __init__(self):
        # Do setup
        self.reset()
        # We are not changing env on this test
        self.env = {"USE_ENV": "1"}

    def _run1(self):
        # Run hoard
        print("========= HOARD RUN #1 =========")
        self.run_hoard("backup")

    def _run2(self):
        # Read UUID and delete file
        self.old_uuid = self.uuid
        os.remove(self.get_uuid_path())
        # Go again, this time with a different uuid
        # This should still succeed because the files have the same checksum
        print("========= HOARD RUN #2 =========")
        print("  After removing the UUID file  ")
        self.run_hoard("backup")

    def _run3(self):
        # Modify a file and backup again so checksums are different the next time
        # This should succeed because this UUID had the last backup
        self.old_content = self.read_hoard_file(Environment.First, HoardFile.AnonFile)
        new_content = secrets.token_bytes(1024)
        assert new_content != self.old_content, "new content should differ from old"
        self.write_hoard_file(Environment.First, HoardFile.AnonFile, new_content)
        print("========= HOARD RUN #3 =========")
        print(" After replacing a file content ")
        self.run_hoard("backup")

    def _run4(self):
        # Swap UUIDs and change the file again and try to back up
        # Should fail because a different machine has the most recent backup
        assert self.old_uuid != self.uuid, "new UUID should not match old one"
        self.uuid = self.old_uuid
        assert self.uuid == self.old_uuid, "UUID should now be set to old one"
        self.write_hoard_file(Environment.First, HoardFile.AnonFile, self.old_content)
        try:
            print("========= HOARD RUN #4 =========")
            print("   After using first UUID/File  ")
            self.run_hoard("backup")
            raise AssertionError("Using the first UUID should have failed (1)")
        except subprocess.CalledProcessError:
            pass
        # Once more to verify it should always fail
        try:
            print("========= HOARD RUN #5 =========")
            print("    Doing it again to be sure   ")
            self.run_hoard("backup")
            raise AssertionError("Using the first UUID should have failed (2)")
        except subprocess.CalledProcessError:
            pass

    def _run5(self):
        # Do it again but forced, and it should succeed
        print("========= HOARD RUN #6 =========")
        print("    Doing it again to be sure   ")
        self.force = True
        self.run_hoard("backup")

    def run_test(self):
        self._run1()
        self._run2()
        self._run3()
        self._run4()
        self._run5()
