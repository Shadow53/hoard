import hashlib
import os
import subprocess
import sys
from pathlib import Path
from testers.cleanup import LogCleanupTester
from testers.correct_errors import CorrectErrorsTester
from testers.hoard_edit import EditCommandTester
from testers.hoard_list import ListHoardsTester
from testers.hoard_tester import HoardFile, Environment
from testers.ignore_filter import IgnoreFilterTester
from testers.last_paths import LastPathsTester
from testers.no_config_dir import MissingConfigDirTester
from testers.operations import OperationCheckerTester
from testers.yaml_support import YAMLSupportTester


for var in ["CI", "GITHUB_ACTIONS"]:
    val = os.environ.get(var)
    if val is None or val != "true":
        raise RuntimeError("These tests must be run on GitHub Actions!")


def print_logs():
    print("\n### Logs:")
    data_dir = LastPathsTester.data_dir_path()
    for dirpath, _, filenames in os.walk(data_dir):
        for file in filenames:
            if file.endswith(".log"):
                path = str(Path(dirpath).joinpath(file))
                print(f"\n##########\n\t{path}")
                sys.stdout.flush()
                subprocess.run(["cat", path])
                sys.stdout.flush()
                print("\n##########")


def print_checksums():
    print("\n### Checksums:")
    for env in list(Environment):
        for file in list(HoardFile):
            if file is not HoardFile.AnonDir and file is not HoardFile.NamedDir1 and file is not HoardFile.NamedDir2:
                path = Path.home().joinpath(f"{env.value}_{file.value}")
                with open(path, "rb") as file:
                    print(f"{path}: {hashlib.md5(file.read()).hexdigest()}")


TEST_MAPPING = {
    "cleanup": ("cleanup", LogCleanupTester),
    "edit_command": ("edit command", EditCommandTester),
    "errors": ("expected errors", CorrectErrorsTester),
    "ignore": ("ignore filter", IgnoreFilterTester),
    "last_paths": ("last paths", LastPathsTester),
    "list_hoards": ("list command", ListHoardsTester),
    "missing_config": ("missing config dir", MissingConfigDirTester),
    "operation": ("operation", OperationCheckerTester),
    "yaml": ("YAML compat", YAMLSupportTester),
}


if __name__ == "__main__":
    if len(sys.argv) == 1:
        raise RuntimeError("One argument - the test - is required")
    successful = []
    try:
        test_arg = sys.argv[1]
        if test_arg == "all":
            print("Running all tests")
            for desc, cls in TEST_MAPPING.values():
                print(f"=== Running {desc} test ===")
                cls().run_test()
                successful.append(desc)
        elif test_arg in TEST_MAPPING:
            desc, cls = TEST_MAPPING[test_arg]
            print(f"Running {desc} test")
            cls().run_test()
        else:
            raise RuntimeError(f"Invalid argument {test_arg}")
    except Exception:
        sys.stdout.flush()
        if desc:
            print(f"=== Error while running {desc} test ===")
        if len(successful) > 0:
            print(f"=== Successful tests: {', '.join(successful)} ===")
        data_dir = LastPathsTester.data_dir_path()
        print("\n### Hoards:")
        sys.stdout.flush()
        subprocess.run(["tree", str(data_dir)])
        print("\n### Home:")
        sys.stdout.flush()
        subprocess.run(["tree", "-aL", "3", str(Path.home())])
        print_checksums()
        print_logs()
        print("\n### Configs:")
        config_dir = LastPathsTester.config_file_path().parent
        for file in os.listdir(config_dir):
            file_path = config_dir.joinpath(file)
            if file_path.is_file():
                with open(file_path, "r", encoding="utf-8") as opened:
                    print(f"##### {file_path}\n")
                    print(opened.read())
        raise
