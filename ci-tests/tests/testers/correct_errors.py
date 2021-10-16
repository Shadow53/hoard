from .hoard_tester import HoardTester


class CorrectErrorsTester(HoardTester):
    def __init__(self):
        super()
        self.reset(config_file="missing-parent-config.toml")

    def run_test(self):
        self.targets = ["missing"]
        result = self.run_hoard("backup", allow_failure=True, capture_output=True)
        assert b"source path does not exist" in result.stdout
