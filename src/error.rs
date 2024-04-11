use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MergeConfigError {
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
  FileOpenError { e: std::io::Error, path: String },

  #[error("failed to read contents of file at {path} | {e:?}")]
  ReadFileContentsError { e: std::io::Error, path: String },

  #[error("failed to parse toml file at {path} | {e:?}")]
  ParseTomlError { e: toml::de::Error, path: String },

  #[error("failed to parse json file at {path} | {e:?}")]
  ParseJsonError { e: serde_json::Error, path: String },

  #[error("unsupported file type at {path}")]
  UnsupportedFileType { path: String },

  #[error("failed to parse merged config into final type | {e:?}")]
  ParseFinalJsonError { e: serde_json::Error },

  #[error("failed to read directory at {path:?}")]
  ReadDirError { path: PathBuf, e: std::io::Error },

  #[error("failed to get file handle for file in directory {path:?}")]
  DirFileError { e: std::io::Error, path: PathBuf },

  #[error("failed to get file name for file at {path:?}")]
  GetFileNameError { path: PathBuf },

  #[error("failed to get metadata for path {path:?} | {e:?}")]
  ReadPathMetaDataError { path: PathBuf, e: std::io::Error },
}
