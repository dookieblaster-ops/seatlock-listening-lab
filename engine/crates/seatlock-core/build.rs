use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut files = Vec::new();
    for folder in ["catalog", "targets"] {
        let dir = root.join(folder);
        println!("cargo:rerun-if-changed={}", dir.display());
        walk(&dir, &root, &mut files);
    }
    files.sort();

    let mut code = String::from("pub static EMBEDDED: &[(&str, &str)] = &[\n");
    for rel in &files {
        let abs = root.join(rel);
        println!("cargo:rerun-if-changed={}", abs.display());
        code.push_str(&format!(
            "    ({rel:?}, include_str!({})),\n",
            serde_quote(&abs.canonicalize().unwrap_or(abs).display().to_string())
        ));
    }
    code.push_str("];\n");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded_catalog.rs");
    fs::write(out, code).unwrap();
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
}

fn serde_quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "/").replace('"', "\\\""))
}
