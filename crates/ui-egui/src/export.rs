//! Export a notebook as one ZIP: the notebook folder as it is on disk, so unzipping it gives
//! the same notebook back.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Writes `notebook` and everything under it to `destination`, with the notebook's own name as
/// the top folder. Returns how many files went in. Links to elsewhere are not followed.
pub fn notebook_zip(notebook: &Path, destination: &Path) -> io::Result<usize> {
    let top = notebook.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "notebook".into());
    let mut files = Vec::new();
    collect(notebook, &mut files)?;
    files.sort();
    let mut zip = zip::ZipWriter::new(File::create(destination)?);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for file in &files {
        let relative = file.strip_prefix(notebook).map_err(io::Error::other)?;
        let name = std::iter::once(top.as_str().to_owned())
            .chain(relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()))
            .collect::<Vec<_>>()
            .join("/");
        zip.start_file(name, options).map_err(io::Error::other)?;
        io::copy(&mut File::open(file)?, &mut zip)?;
    }
    zip.finish().map_err(io::Error::other)?.flush()?;
    Ok(files.len())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), out)?;
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn a_notebook_comes_back_out_of_its_zip() {
        let root = std::env::temp_dir().join(format!("notecraft-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let notebook = root.join("My notes");
        std::fs::create_dir_all(notebook.join("Section/page/assets")).expect("dirs");
        std::fs::write(notebook.join("notebook.toml"), "title = \"My notes\"").expect("write");
        std::fs::write(notebook.join("Section/page/page.md"), "# Hello").expect("write");
        std::fs::write(notebook.join("Section/page/assets/a.bin"), [0u8, 1, 2]).expect("write");
        let zip_path = root.join("out.zip");

        assert_eq!(notebook_zip(&notebook, &zip_path).expect("export"), 3);

        let mut archive = zip::ZipArchive::new(File::open(&zip_path).expect("open")).expect("zip");
        let mut names: Vec<String> = (0..archive.len()).filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string())).collect();
        names.sort();
        assert_eq!(names, ["My notes/Section/page/assets/a.bin", "My notes/Section/page/page.md", "My notes/notebook.toml"]);
        let mut text = String::new();
        archive.by_name("My notes/Section/page/page.md").expect("entry").read_to_string(&mut text).expect("read");
        assert_eq!(text, "# Hello");
        let _ = std::fs::remove_dir_all(&root);
    }
}
