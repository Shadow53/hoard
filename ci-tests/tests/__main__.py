import os
import subprocess
import sys
from pathlib import Path
from testers.ignore_filter import IgnoreFilterTester
from testers.last_paths import LastPathsTester
from testers.operations import OperationCheckerTester

for var in ["CI", "GITHUB_ACTIONS"]:
    val = os.environ.get(var)
    if val is None or val != "true":
        raise RuntimeError("These tests must be run on GitHub Actions!")

if __name__ == "__main__":
    if len(sys.argv) == 1:
        raise RuntimeError("One argument - the test - is required")
    try:
        if sys.argv[1] == "last_paths":
            print("Running last_paths test")
            LastPathsTester().run_test()
        elif sys.argv[1] == "operation":
            print("Running operation test")
            OperationCheckerTester().run_test()
        elif sys.argv[1] == "ignore":
            print("Running ignore filter test")
            IgnoreFilterTester().run_test()
        else:
            raise RuntimeError(f"Invalid argument {sys.argv[1]}")
    except Exception:
        print("\nHoards:")
        subprocess.run(["tree", str(LastPathsTester.data_dir_path())])
        print("\nHome:")
        subprocess.run(["tree", "-aL", "3", str(Path.home())])
        raise
