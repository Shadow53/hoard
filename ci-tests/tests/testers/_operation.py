from typing import Dict, List, Tuple, Union
from datetime import datetime
from enum import Enum
from pathlib import Path

from pydantic import BaseModel


AnonHoardV1 = Dict[str, str]
NamedHoardV1 = Dict[str, AnonHoardV1]


class ChecksumType(str, Enum):
    MD5 = "md5"
    SHA256 = "sha256"


class Direction(str, Enum):
    BACKUP = "backup"
    RESTORE = "restore"


class OperationV1(BaseModel):
    timestamp: List[int]
    is_backup: bool
    hoard_name: str
    hoard: Union[AnonHoardV1, NamedHoardV1]


V2FileMap = Dict[Path, Dict[ChecksumType, str]]


class PileV2(BaseModel):
    created: V2FileMap
    modified: V2FileMap
    deleted: List[Path]
    unmodified: V2FileMap


class OperationV2(BaseModel):
    timestamp: List[int]
    direction: Direction
    hoard: str
    files: PileV2
