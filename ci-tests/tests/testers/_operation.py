from typing import Dict, List, Tuple, Union
from datetime import datetime, date
from enum import Enum
from pathlib import Path

from pydantic import BaseModel


AnonHoardV1 = Dict[str, str]
NamedHoardV1 = Dict[str, AnonHoardV1]
Timestamp = Tuple[int, int, int, int, int, int, int, int, int]


def datetime_to_timestamp(dt: datetime) -> Timestamp:
    offset = dt.tzinfo
    offset = 0 if offset is None else offset.utcoffset(dt).total_seconds()
    return (
        dt.year,
        dt.toordinal() - date(dt.year, 1, 1).toordinal() + 1,
        dt.hour,
        dt.minute,
        dt.second,
        dt.microsecond * 1000,
        int(offset / 3600),
        offset % 3600,
        offset % 60,
    )


def timestamp_to_filename(timestamp: Timestamp) -> str:
    dt = datetime.fromordinal(timestamp[1])
    return f"{timestamp[0]}_{dt.month:02d}_{dt.day:02d}-{timestamp[2]:02d}_{timestamp[3]:02d}_{timestamp[4]:02d}.{int(timestamp[5] / 1000):06d}.log"


class ChecksumType(str, Enum):
    MD5 = "md5"
    SHA256 = "sha256"


class HoardType(str, Enum):
    ANONYMOUS = "Anonymous"
    NAMED = "Named"


class Direction(str, Enum):
    BACKUP = "backup"
    RESTORE = "restore"


class OperationV1(BaseModel):
    timestamp: Timestamp
    is_backup: bool
    hoard_name: str
    hoard: Dict[HoardType, Union[AnonHoardV1, NamedHoardV1]]

    def path(self, data_dir: Path, uuid: str):
        return data_dir.joinpath("history").joinpath(uuid).joinpath(self.hoard_name).joinpath(timestamp_to_filename(self.timestamp))


V2FileMap = Dict[Path, Dict[ChecksumType, str]]


class PileV2(BaseModel):
    created: V2FileMap
    modified: V2FileMap
    deleted: List[Path]
    unmodified: V2FileMap


class OperationV2(BaseModel):
    timestamp: Timestamp
    direction: Direction
    hoard: str
    files: Union[PileV2, Dict[str, PileV2]]

    def path(self, data_dir: Path, uuid: str):
        return data_dir.joinpath("history").joinpath(uuid).joinpath(self.hoard).joinpath(timestamp_to_filename(self.timestamp))
