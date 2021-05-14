use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// "hoard": {
//     "hoard_name" Hoard: {
//         "config": Config,
//         "envconf": "path",
//         ...
//     }
// }

#[derive(Serialize, Deserialize)]
pub struct Config {}

#[derive(Serialize, Deserialize)]
pub struct Hoard {
    pub config: Config,
    files: HashMap<String, Entry>,
}

#[derive(Serialize, Deserialize)]
pub struct Entry {
    environments: Vec<String>,
    destination: PathBuf,
}
