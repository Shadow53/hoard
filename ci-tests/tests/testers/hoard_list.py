from .hoard_tester import HoardTester


class ListHoardsTester(HoardTester):
    def __init__(self):
        super().__init__()
        self.reset()

    def run_test(self):
        expected = b"anon_dir\nanon_file\nnamed\n"
        output = self.run_hoard("list", capture_output=True)
        assert expected in output.stdout

        self.env["HOARD_LOG"] = "info"
        output = self.run_hoard("list", capture_output=True)
        assert output.stdout == expected
