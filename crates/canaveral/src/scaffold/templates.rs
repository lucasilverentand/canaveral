use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::errors::ScaffoldError;

pub fn template_root() -> anyhow::Result<PathBuf> {
    if let Ok(explicit) = std::env::var("CANAVERAL_TEMPLATE_DIR") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Some(path) = find_from_ancestors(std::env::current_dir()?.as_path()) {
        return Ok(path);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(path) = find_from_ancestors(exe_path.as_path()) {
            return Ok(path);
        }
    }

    let manifest_based = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("templates")
        .join("scaffold");
    if manifest_based.exists() {
        return Ok(manifest_based);
    }

    Err(ScaffoldError::TemplateRootNotFound.into())
}

fn find_from_ancestors(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        let candidate = dir.join("templates").join("scaffold");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }

    None
}

pub fn load_template(relative_path: &str) -> anyhow::Result<String> {
    let root = template_root()?;
    let full_path = root.join(relative_path);
    if !full_path.exists() {
        return Err(ScaffoldError::TemplateFileMissing(relative_path.to_string()).into());
    }
    Ok(std::fs::read_to_string(&full_path)?)
}

pub fn render_template(input: &str, vars: &HashMap<String, String>) -> String {
    let mut out = input.to_string();
    for (key, value) in vars {
        let token = format!("{{{{{key}}}}}");
        out = out.replace(&token, value);
    }
    out
}

pub fn list_template_files(base: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    visit_dir(base, base, &mut out)?;
    out.sort();
    Ok(out)
}

fn visit_dir(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| anyhow::anyhow!("failed to strip template prefix"))?;
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_replaces_tokens() {
        let mut vars = HashMap::new();
        vars.insert("project_name".to_string(), "demo".to_string());
        let out = render_template("hello {{project_name}}", &vars);
        assert_eq!(out, "hello demo");
    }
}
