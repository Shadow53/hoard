from .hoard_tester import HoardTester
from pathlib import Path
import os


class IgnoreFilterTester(HoardTester):
    def __init__(self):
        self.reset()
        # We are not changing env on this test
        self.env = {"USE_ENV": "1"}
        # All three of global, hoard, and pile-ignore files should be ignored.
        self.global_file = "global_ignore"
        self.hoard_file = "ignore_for_hoard"
        self.pile_file = "spilem"
        self.nested_file = "nested_dir/.hidden"

        # Create files to ignore
        self.anon_dir_root = Path.home().joinpath("first_anon_dir")
        self.named_dir1_root = Path.home().joinpath("first_named_dir1")
        self.named_dir2_root = Path.home().joinpath("first_named_dir2")

        for root in [self.anon_dir_root, self.named_dir1_root, self.named_dir2_root]:
            for file in [self.global_file, self.hoard_file, self.pile_file, self.nested_file]:
                self.generate_file(root.joinpath(file))

    def run_test(self):
        # Run hoard
        self.run_hoard("backup")

        # Delete unexpected files for assert_same_tree
        # Named dir1 pile should ignore all
        for file in [self.global_file, self.hoard_file, self.pile_file]:
            os.remove(self.named_dir1_root.joinpath(file))
        # Named dir2 pile should only ignore hoard and global
        for file in [self.global_file, self.hoard_file]:
            os.remove(self.named_dir2_root.joinpath(file))
        # Anon dir should only ignore global
        os.remove(self.anon_dir_root.joinpath(self.global_file))

        # Assert trees
        home = Path.home()
        data_dir = self.data_dir_path()
        self.assert_same_tree(
            home.joinpath("first_anon_dir"),
            data_dir.joinpath("hoards", "anon_dir"),
            extra_files=[self.pile_file, self.hoard_file, self.nested_file]
        )
        self.assert_same_tree(
            home.joinpath("first_named_dir1"),
            data_dir.joinpath("hoards", "named", "dir1"),
            # The file name should match the glob ".hidden",
            # but the path should not match because it is not in the root
            # of the pile directory.
            extra_files=[self.nested_file]
        )
        self.assert_same_tree(
            home.joinpath("first_named_dir2"),
            data_dir.joinpath("hoards", "named", "dir2"),
            extra_files=[self.pile_file, self.nested_file]
        )
