use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum TypeExportError<E> {
    Io(std::io::Error),
    Export(E),
}

impl<E: fmt::Display> fmt::Display for TypeExportError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error during type export: {error}"),
            Self::Export(error) => write!(f, "type export failed: {error}"),
        }
    }
}

impl<E> std::error::Error for TypeExportError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Export(error) => Some(error),
        }
    }
}

pub fn export_types_atomically<E>(
    out_path: &Path,
    export: impl FnOnce(&Path) -> Result<(), E>,
) -> Result<(), TypeExportError<E>> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(TypeExportError::Io)?;
    }

    let tmp_path = temporary_output_path(out_path);
    remove_file_if_exists(&tmp_path).map_err(TypeExportError::Io)?;

    match export(&tmp_path) {
        Ok(()) => {
            replace_file(&tmp_path, out_path).map_err(TypeExportError::Io)?;
            Ok(())
        }
        Err(error) => {
            remove_file_if_exists(&tmp_path).map_err(TypeExportError::Io)?;
            Err(TypeExportError::Export(error))
        }
    }
}

fn temporary_output_path(out_path: &Path) -> PathBuf {
    let file_name = out_path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "types.ts".into());
    out_path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn replace_file(tmp_path: &Path, out_path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    remove_file_if_exists(out_path)?;
    std::fs::rename(tmp_path, out_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;

    #[test]
    fn export_types_atomically_preserves_existing_output_when_export_fails() {
        let dir = tempfile::tempdir().unwrap();
        let out_path = dir.path().join("events.ts");
        fs::write(&out_path, "original").unwrap();

        let result = export_types_atomically(&out_path, |tmp_path| -> io::Result<()> {
            fs::write(tmp_path, "partial").unwrap();
            Err(io::Error::other("export failed"))
        });

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&out_path).unwrap(), "original");
        let leftovers = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path != &out_path)
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "failed export should clean temporary files: {leftovers:?}"
        );
    }

    #[test]
    fn export_types_atomically_replaces_existing_output_after_success() {
        let dir = tempfile::tempdir().unwrap();
        let out_path = dir.path().join("commands.ts");
        fs::write(&out_path, "old").unwrap();

        export_types_atomically(&out_path, |tmp_path| -> io::Result<()> {
            fs::write(tmp_path, "new")
        })
        .unwrap();

        assert_eq!(fs::read_to_string(&out_path).unwrap(), "new");
    }
}
