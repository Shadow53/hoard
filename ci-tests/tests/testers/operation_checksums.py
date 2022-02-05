import os
from hashlib import md5, sha256
from pathlib import Path
from .hoard_tester import HoardTester
from ._operation import OperationV2, ChecksumType, Direction


class OperationChecksumTester(HoardTester):
    file_path = Path.home().joinpath("testing.txt")

    def __init__(self):
        super().__init__()
        self.reset("operation-checksums-config.toml")

    def setup(self):
        self.generate_file(self.file_path)
        self.run_hoard("backup")

    def _get_operation(self, hoard_name: str) -> OperationV2:
        folder = self.data_dir_path().joinpath("history").joinpath(self.uuid).joinpath(hoard_name)
        file_path = None
        with os.scandir(folder) as it:
            for entry in it:
                if entry.is_file() and entry.path != "last_paths.json":
                    file_path = entry.path
                    break
        if file_path is None:
            raise RuntimeError(f"could not find operation file for {hoard_name}")
        return OperationV2.parse_file(file_path)

    def _assert_checksum_matches(self, hoard_name: str, type: ChecksumType, value: str):
        op = self._get_operation(hoard_name)
        assert op.direction == Direction.BACKUP
        assert op.hoard == hoard_name
        expected = {type: value}
        received = op.files.created.get(Path(""))
        assert received == expected, f"expected {expected}, got {received} in operation {op}"

    def run_test(self):
        with open(self.file_path, "rb") as file:
            content = file.read()
        md5_checksum = md5(content).hexdigest()
        sha256_checksum = sha256(content).hexdigest()

        self._assert_checksum_matches("md5", ChecksumType.MD5, md5_checksum)
        self._assert_checksum_matches("sha256", ChecksumType.SHA256, sha256_checksum)
        self._assert_checksum_matches("default", ChecksumType.SHA256, sha256_checksum)
