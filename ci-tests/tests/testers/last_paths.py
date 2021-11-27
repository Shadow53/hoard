from .hoard_tester import HoardTester
import subprocess


class LastPathsTester(HoardTester):
    def __init__(self):
        super().__init__()
        # Do setup
        self.reset()

    def run_test(self):
        # Run hoard with env "first"
        self.env = {"USE_ENV": "1"}
        self.run_hoard("backup")
        # Doing it again should still succeed
        self.run_hoard("backup")

        # Run hoard with env "second" - this should fail
        try:
            self.env = {"USE_ENV": "2"}
            self.run_hoard("backup")
            raise AssertionError(
                "Changing environment should have caused last_paths to fail"
            )
        except subprocess.CalledProcessError:
            pass
        # Doing it again with "first" should still succeed
        self.env = {"USE_ENV": "1"}
        self.run_hoard("backup")
        # Make sure the files are consistent with backing up "first"
        self.assert_first_tree()
        # Doing it with "second" but forced should succeed
        self.env = {"USE_ENV": "2"}
        self.force = True
        self.run_hoard("backup")
        # Make sure the files were overwritten
        self.assert_second_tree()
