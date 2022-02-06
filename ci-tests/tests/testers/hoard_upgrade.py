from datetime import datetime, timedelta
import os
import uuid
from .hoard_tester import HoardTester
from ._operation import ChecksumType, Direction, OperationV1, OperationV2, PileV2, datetime_to_timestamp, HoardType


class HoardUpgradeTester(HoardTester):
    def __init__(self):
        super().__init__()
        third_dt = datetime.now()
        second_dt = third_dt - timedelta(hours=2)
        first_dt = second_dt - timedelta(hours=2)
        first_timestamp = datetime_to_timestamp(first_dt)
        second_timestamp = datetime_to_timestamp(second_dt)
        third_timestamp = datetime_to_timestamp(third_dt)
        self.anon_v1 = [
            OperationV1(
                timestamp=first_timestamp,
                is_backup=True,
                hoard_name="anon_file",
                hoard={HoardType.ANONYMOUS: {"": "d3369a026ace494f56ead54d502a00dd"}}
            ),
            OperationV1(
                timestamp=second_timestamp,
                is_backup=False,
                hoard_name="anon_file",
                hoard={HoardType.ANONYMOUS: {"": "d3369a026ace494f56ead54d502a00dd"}}
            ),
            OperationV1(
                timestamp=third_timestamp,
                is_backup=True,
                hoard_name="anon_file",
                hoard={HoardType.ANONYMOUS: {}}
            ),
        ]
        self.anon_v2 = [
            OperationV2(
                timestamp=first_timestamp,
                direction="backup",
                hoard="anon_file",
                files=PileV2(
                    created={"": {ChecksumType.MD5: "d3369a026ace494f56ead54d502a00dd"}},
                    modified={},
                    deleted=[],
                    unmodified={},
                )
            ),
            OperationV2(
                timestamp=second_timestamp,
                direction=Direction.RESTORE,
                hoard="anon_file",
                files=PileV2(
                    created={},
                    modified={},
                    deleted=[],
                    unmodified={"": {ChecksumType.MD5: "d3369a026ace494f56ead54d502a00dd"}},
                )
            ),
            OperationV2(
                timestamp=third_timestamp,
                direction=Direction.BACKUP,
                hoard="anon_file",
                files=PileV2(
                    created={},
                    modified={},
                    deleted=[""],
                    unmodified={},
                )
            ),
        ]
        self.anon_dir_v1 = [
            OperationV1(
                timestamp=first_timestamp,
                is_backup=True,
                hoard_name="anon_dir",
                hoard={HoardType.ANONYMOUS:{
                    "file_1": "ba9d332813a722b273a95fa13dd88d94",
                    "file_2": "92ed3b5f07b44bc4f70d0b24d5e1867c",
                }}
            ),
            OperationV1(
                timestamp=second_timestamp,
                is_backup=True,
                hoard_name="anon_dir",
                hoard={HoardType.ANONYMOUS: {
                    "file_1": "1cfab2a192005a9a8bdc69106b4627e2",
                    "file_2": "92ed3b5f07b44bc4f70d0b24d5e1867c",
                    "file_3": "797b373a9c4ec0d6de0a31a90b5bee8e",
                }}
            ),
            OperationV1(
                timestamp=third_timestamp,
                is_backup=True,
                hoard_name="anon_dir",
                hoard={HoardType.ANONYMOUS: {
                    "file_1": "1cfab2a192005a9a8bdc69106b4627e2",
                    "file_3": "1deb21ef3bb87be4ad71d73fff6bb8ec",
                }}
            ),
        ]
        self.anon_dir_v2 = [
            OperationV2(
                timestamp=first_timestamp,
                direction=Direction.BACKUP,
                hoard="anon_dir",
                files=PileV2(
                    created={
                        "file_1": {ChecksumType.MD5: "ba9d332813a722b273a95fa13dd88d94"},
                        "file_2": {ChecksumType.MD5: "92ed3b5f07b44bc4f70d0b24d5e1867c"},
                    },
                    modified={},
                    deleted=[],
                    unmodified={},
                )
            ),
            OperationV2(
                timestamp=second_timestamp,
                direction=Direction.BACKUP,
                hoard="anon_dir",
                files=PileV2(
                    created={
                        "file_3": {ChecksumType.MD5: "797b373a9c4ec0d6de0a31a90b5bee8e"},
                    },
                    modified={
                        "file_1": {ChecksumType.MD5: "1cfab2a192005a9a8bdc69106b4627e2"},
                    },
                    deleted=[],
                    unmodified={
                        "file_2": {ChecksumType.MD5: "92ed3b5f07b44bc4f70d0b24d5e1867c"},
                    },
                )
            ),
            OperationV2(
                timestamp=third_timestamp,
                direction=Direction.BACKUP,
                hoard="anon_dir",
                files=PileV2(
                    created={},
                    modified={
                        "file_3": {ChecksumType.MD5: "1deb21ef3bb87be4ad71d73fff6bb8ec"},
                    },
                    deleted=["file_2"],
                    unmodified={
                        "file_1": {ChecksumType.MD5: "1cfab2a192005a9a8bdc69106b4627e2"},
                    },
                )
            ),
        ]
        self.named_v1 = [
            OperationV1(
                timestamp=first_timestamp,
                is_backup=True,
                hoard_name="named",
                hoard={HoardType.NAMED: {
                    "single_file": {"": "d3369a026ace494f56ead54d502a00dd"},
                    "dir": {
                        "file_1": "ba9d332813a722b273a95fa13dd88d94",
                        "file_2": "92ed3b5f07b44bc4f70d0b24d5e1867c",
                    },
                }}
            ),
            OperationV1(
                timestamp=second_timestamp,
                is_backup=False,
                hoard_name="named",
                hoard={HoardType.NAMED: {
                    "single_file": {"": "d3369a026ace494f56ead54d502a00dd"},
                    "dir": {
                        "file_1": "1cfab2a192005a9a8bdc69106b4627e2",
                        "file_2": "92ed3b5f07b44bc4f70d0b24d5e1867c",
                        "file_3": "797b373a9c4ec0d6de0a31a90b5bee8e",
                    },
                }}
            ),
            OperationV1(
                timestamp=third_timestamp,
                is_backup=True,
                hoard_name="named",
                hoard={HoardType.NAMED: {
                    "single_file": {},
                    "dir": {
                        "file_1": "1cfab2a192005a9a8bdc69106b4627e2",
                        "file_3": "1deb21ef3bb87be4ad71d73fff6bb8ec",
                    },
                }}
            ),
        ]
        self.named_v2 = [
            OperationV2(
                timestamp=first_timestamp,
                direction="backup",
                hoard="named",
                files={
                    "single_file": PileV2(
                        created={"": {ChecksumType.MD5: "d3369a026ace494f56ead54d502a00dd"}},
                        modified={},
                        deleted=[],
                        unmodified={},
                    ),
                    "dir": PileV2(
                        created={
                            "file_1": {ChecksumType.MD5: "ba9d332813a722b273a95fa13dd88d94"},
                            "file_2": {ChecksumType.MD5: "92ed3b5f07b44bc4f70d0b24d5e1867c"},
                        },
                        modified={},
                        deleted=[],
                        unmodified={},
                    )
                }
            ),
            OperationV2(
                timestamp=second_timestamp,
                direction="restore",
                hoard="named",
                files={
                    "single_file": PileV2(
                        created={},
                        modified={},
                        deleted=[],
                        unmodified={"": {ChecksumType.MD5: "d3369a026ace494f56ead54d502a00dd"}},
                    ),
                    "dir": PileV2(
                        created={
                            "file_3": {ChecksumType.MD5: "797b373a9c4ec0d6de0a31a90b5bee8e"},
                        },
                        modified={
                            "file_1": {ChecksumType.MD5: "1cfab2a192005a9a8bdc69106b4627e2"},
                        },
                        deleted=[],
                        unmodified={
                            "file_2": {ChecksumType.MD5: "92ed3b5f07b44bc4f70d0b24d5e1867c"},
                        },
                    )
                }
            ),
            OperationV2(
                timestamp=third_timestamp,
                direction="backup",
                hoard="named",
                files={
                    "single_file": PileV2(
                        created={},
                        modified={},
                        deleted=[""],
                        unmodified={},
                    ),
                    "dir": PileV2(
                        created={},
                        modified={
                            "file_3": {ChecksumType.MD5: "1deb21ef3bb87be4ad71d73fff6bb8ec"},
                        },
                        deleted=["file_2"],
                        unmodified={
                            "file_1": {ChecksumType.MD5: "1cfab2a192005a9a8bdc69106b4627e2"},
                        },
                    )
                }
            ),
        ]

    def setup(self):
        self.uuid = str(uuid.uuid4())
        all_v1 = self.anon_v1 + self.anon_dir_v1 + self.named_v1
        for op in all_v1:
            path = op.path(self.data_dir_path(), self.uuid)
            os.makedirs(path.parent, exist_ok=True)
            with open(path, "w", encoding="utf-8") as file:
                file.write(op.json())

    def run_test(self):
        self.reset(config_file="hoard-upgrade-config.toml")
        self.run_hoard("upgrade")
        self.flush()

        all_v2 = self.anon_v2 + self.anon_dir_v2 + self.named_v2
        for op in all_v2:
            path = op.path(self.data_dir_path(), self.uuid)
            print(f"=== Checking upgraded log at {path}")
            upgraded = OperationV2.parse_file(path)
            assert upgraded == op, f"expected {op}, got {upgraded}"
