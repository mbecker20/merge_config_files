use std::{
  borrow::Borrow,
  fs::File,
  io::Read,
  path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("types on field {key} do not match | got {value:?}, expected object")]
  ObjectFieldTypeMismatch {
    key: String,
    value: Box<dyn std::fmt::Debug>,
  },

  #[error("types on field {key} do not match | got {value:?}, expected array")]
  ArrayFieldTypeMismatch {
    key: String,
    value: Box<dyn std::fmt::Debug>,
  },

  #[error("failed to open file at {path} | {e:?}")]
  FileOpen { e: std::io::Error, path: PathBuf },

  #[error("failed to read contents of file at {path} | {e:?}")]
  ReadFileContents { e: std::io::Error, path: PathBuf },

  #[error("failed to parse toml file at {path} | {e:?}")]
  ParseToml { e: toml::de::Error, path: PathBuf },

  #[error("failed to parse json file at {path} | {e:?}")]
  ParseJson { e: serde_json::Error, path: PathBuf },

  #[error("unsupported file type at {path}")]
  UnsupportedFileType { path: PathBuf },

  #[error("failed to parse merged config into final type | {e:?}")]
  ParseFinalJson { e: serde_json::Error },

  #[error("failed to serialize merged config to string | {e:?}")]
  SerializeFinalJson { e: serde_json::Error },

  #[error("failed to read directory at {path:?}")]
  ReadDir { path: PathBuf, e: std::io::Error },

  #[error("failed to get file handle for file in directory {path:?}")]
  DirFile { e: std::io::Error, path: PathBuf },

  #[error("failed to get file name for file at {path:?}")]
  GetFileName { path: PathBuf },

  #[error("failed to get metadata for path {path:?} | {e:?}")]
  ReadPathMetaData { path: PathBuf, e: std::io::Error },
}

pub type Result<T> = ::core::result::Result<T, Error>;

/// parse paths that are either directories or files
pub fn parse_config_paths<'a, T: DeserializeOwned>(
  paths: &[&Path],
  match_wildcards: Option<&'a [&'a str]>,
  merge_nested: bool,
  extend_array: bool,
) -> Result<T> {
  let match_wildcards = match_wildcards
    .map(|match_wildcards| {
      match_wildcards
        .into_iter()
        .map(|&kw| kw.to_string())
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();
  let wildcards = match_wildcards
    .iter()
    .flat_map(|kw| match wildcard::Wildcard::new(kw.as_bytes()) {
      Ok(wc) => Some(wc),
      Err(e) => {
        eprintln!("kw '{kw}' is invalid wildcard | {e:?}");
        None
      }
    })
    .collect::<Vec<_>>();
  let paths = paths
    .into_iter()
    .map(|&path| {
      let is_dir = std::fs::metadata(path)
        .map_err(|e| Error::ReadPathMetaData {
          path: path.to_path_buf(),
          e,
        })?
        .is_dir();
      if is_dir {
        file_names_in_dir(path, &wildcards)
      } else {
        Result::Ok(vec![path.to_path_buf()])
      }
    })
    .collect::<Result<Vec<_>>>()?
    .into_iter()
    .flatten()
    .collect::<Vec<PathBuf>>();
  parse_config_files(&paths, merge_nested, extend_array)
}

/// will sort file names alphabetically
fn file_names_in_dir(dir_path: &Path, wildcards: &[wildcard::Wildcard]) -> Result<Vec<PathBuf>> {
  let mut files = std::fs::read_dir(dir_path)
    .map_err(|e| Error::ReadDir {
      path: dir_path.to_path_buf(),
      e,
    })?
    .map(|file| {
      let file = file.map_err(|e| Error::DirFile {
        e,
        path: dir_path.to_path_buf(),
      })?;
      let is_file = file
        .metadata()
        .map_err(|e| Error::ReadPathMetaData {
          e,
          path: dir_path.to_path_buf(),
        })?
        .is_file();
      Result::Ok((file, is_file))
    })
    .collect::<Result<Vec<_>>>()?
    .into_iter()
    .filter(|(_, is_file)| *is_file)
    .map(|(file, _)| {
      let path = file.path();
      let name = file
        .file_name()
        .to_str()
        .ok_or(Error::GetFileName {
          path: dir_path.join(&path),
        })?
        .to_string();
      Ok((name, path))
    })
    .collect::<Result<Vec<_>>>()?
    .into_iter()
    .filter(|(name, _)| wildcards.iter().any(|wc| wc.is_match(name.as_bytes())))
    .map(|(_, path)| path)
    .collect::<Vec<_>>();
  files.sort();
  Ok(files)
}

/// parses multiple config files
pub fn parse_config_files<T: DeserializeOwned>(
  paths: &[PathBuf],
  merge_nested: bool,
  extend_array: bool,
) -> Result<T> {
  let mut target = Map::new();

  for path in paths {
    target = merge_objects(
      target,
      parse_config_file(path.borrow())?,
      merge_nested,
      extend_array,
    )?;
  }

  serde_json::from_str(
    &serde_json::to_string(&target).map_err(|e| Error::SerializeFinalJson { e })?,
  )
  .map_err(|e| Error::ParseFinalJson { e })
}

/// parses a single config file
pub fn parse_config_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
  let mut file = File::open(path).map_err(|e| Error::FileOpen {
    e,
    path: path.to_path_buf(),
  })?;
  let config = match path.extension().and_then(|e| e.to_str()) {
    Some("toml") => {
      let mut contents = String::new();
      file
        .read_to_string(&mut contents)
        .map_err(|e| Error::ReadFileContents {
          e,
          path: path.to_path_buf(),
        })?;
      toml::from_str(&contents).map_err(|e| Error::ParseToml {
        e,
        path: path.to_path_buf(),
      })?
    }
    Some("json") => serde_json::from_reader(file).map_err(|e| Error::ParseJson {
      e,
      path: path.to_path_buf(),
    })?,
    Some(_) | None => {
      return Err(Error::UnsupportedFileType {
        path: path.to_path_buf(),
      });
    }
  };
  Ok(config)
}

/// object is serde_json::Map<String, serde_json::Value>
/// source will overide target
/// will recurse when field is object if merge_object = true, otherwise object will be replaced
/// will extend when field is array if extend_array = true, otherwise array will be replaced
/// will return error when types on source and target fields do not match
fn merge_objects(
  mut target: Map<String, Value>,
  source: Map<String, Value>,
  merge_nested: bool,
  extend_array: bool,
) -> Result<Map<String, Value>> {
  for (key, value) in source {
    let Some(curr) = target.remove(&key) else {
      target.insert(key, value);
      continue;
    };
    match curr {
      Value::Object(target_obj) => {
        if !merge_nested {
          target.insert(key, value);
          continue;
        }
        match value {
          Value::Object(source_obj) => {
            target.insert(
              key,
              Value::Object(merge_objects(
                target_obj,
                source_obj,
                merge_nested,
                extend_array,
              )?),
            );
          }
          _ => {
            return Err(Error::ObjectFieldTypeMismatch {
              key,
              value: Box::new(value),
            })
          }
        }
      }
      Value::Array(mut target_arr) => {
        if !extend_array {
          target.insert(key, value);
          continue;
        }
        match value {
          Value::Array(source_arr) => {
            target_arr.extend(source_arr);
            target.insert(key, Value::Array(target_arr));
          }
          _ => {
            return Err(Error::ArrayFieldTypeMismatch {
              key,
              value: Box::new(value),
            })
          }
        }
      }
      _ => {
        target.insert(key, value);
      }
    }
  }
  Ok(target)
}
