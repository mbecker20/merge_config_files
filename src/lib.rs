use std::{borrow::Borrow, fs::File, io::Read, path::PathBuf};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub mod error;

use error::MergeConfigError::*;

pub type MergeConfigResult<T> = Result<T, error::MergeConfigError>;

/// will layer later dirs on top of previous
/// will sort file paths alphabetically
/// will only include file paths with names that contain all match keywords
/// if match_keywords empty, will include all files in the directory
pub fn parse_config_dirs<T: DeserializeOwned>(
    dir_paths: impl IntoIterator<Item = impl Borrow<str>>,
    match_keywords: impl IntoIterator<Item = impl Borrow<str>>,
    merge_nested: bool,
    extend_array: bool,
) -> MergeConfigResult<T> {
    let paths = dir_files_matching_kw(dir_paths, match_keywords)?;
    parse_config_files(paths, merge_nested, extend_array)
}

pub fn dir_files_matching_kw(
    dir_paths: impl IntoIterator<Item = impl Borrow<str>>,
    match_keywords: impl IntoIterator<Item = impl Borrow<str>>,
) -> MergeConfigResult<Vec<String>> {
    let kws = match_keywords
        .into_iter()
        .map(|s| s.borrow().to_string())
        .collect::<Vec<String>>();
    let files = dir_paths
        .into_iter()
        .map(|dir_path| {
            let path = dir_path.borrow();
            PathBuf::from(path)
        })
        .map(|dir_path| {
            let mut files = std::fs::read_dir(&dir_path)
                .map_err(|e| ReadDirError {
                    path: dir_path.clone(),
                    e,
                })?
                .map(|file| {
                    let file = file.map_err(|e| DirFileError {
                        e,
                        path: dir_path.clone(),
                    })?;
                    let path = file.path();
                    let name = file
                        .file_name()
                        .to_str()
                        .ok_or(GetFileNameError {
                            path: dir_path.join(&path),
                        })?
                        .to_string();
                    Ok((name, path))
                })
                .collect::<MergeConfigResult<Vec<_>>>()?
                .into_iter()
                .filter(|(name, _)| {
                    for kw in &kws {
                        if !name.contains(kw) {
                            return false;
                        }
                    }
                    true
                })
                .map(|(_, path)| path.as_path().display().to_string())
                .collect::<Vec<String>>();
            files.sort();
            Ok(files)
        })
        .collect::<MergeConfigResult<Vec<Vec<String>>>>()?
        .into_iter()
        .flatten()
        .map(|path| {
            println!("{path}");
            path
        })
        .collect();
    Ok(files)
}

/// parses multiple config files
pub fn parse_config_files<T: DeserializeOwned>(
    paths: impl IntoIterator<Item = impl Borrow<str>>,
    merge_nested: bool,
    extend_array: bool,
) -> MergeConfigResult<T> {
    let mut target = Map::new();

    for path in paths {
        target = merge_objects(
            target,
            parse_config_file(path.borrow())?,
            merge_nested,
            extend_array,
        )?;
    }

    serde_json::from_str(&serde_json::to_string(&target).unwrap())
        .map_err(|e| ParseFinalJsonError { e })
}

/// parses a single config file
pub fn parse_config_file<T: DeserializeOwned>(path: impl Borrow<str>) -> MergeConfigResult<T> {
    let path: &str = path.borrow();
    let mut file = File::open(&path).map_err(|e| FileOpenError {
        e,
        path: path.to_string(),
    })?;
    let config = if path.ends_with("toml") {
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| ReadFileContentsError {
                e,
                path: path.to_string(),
            })?;
        toml::from_str(&contents).map_err(|e| ParseTomlError {
            e,
            path: path.to_string(),
        })?
    } else if path.ends_with("json") {
        serde_json::from_reader(file).map_err(|e| ParseJsonError {
            e,
            path: path.to_string(),
        })?
    } else {
        return Err(UnsupportedFileType {
            path: path.to_string(),
        });
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
) -> MergeConfigResult<Map<String, Value>> {
    for (key, value) in source {
        let curr = target.remove(&key);
        if curr.is_none() {
            target.insert(key, value);
            continue;
        }
        let curr = curr.unwrap();
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
                        return Err(ObjectFieldTypeMismatch {
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
                        return Err(ArrayFieldTypeMismatch {
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
