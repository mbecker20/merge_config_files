use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
  #[serde(rename = "name")]
  _name: String,
  #[serde(rename = "addresses")]
  _addresses: Vec<String>,
  #[serde(rename = "values")]
  _values: HashMap<String, String>,
}

fn main() {
  let config = merge_config_files::parse_config_paths::<Config>(
    &[PathBuf::from("./config").as_path()],
    None,
    true,
    true,
  )
  .unwrap();
  println!("{config:#?}");
}
