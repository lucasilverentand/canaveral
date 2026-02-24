//! Docker registry operations: build, push, tag

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use canaveral_core::error::{AdapterError, Result};

/// Build a Docker image with the given tag.
pub fn build_image(
    path: &Path,
    tag: &str,
    build_args: &HashMap<String, String>,
    platforms: &[String],
) -> Result<()> {
    let mut cmd = Command::new("docker");
    cmd.arg("build");
    cmd.arg("-t").arg(tag);

    for (key, value) in build_args {
        cmd.arg("--build-arg").arg(format!("{}={}", key, value));
    }

    if !platforms.is_empty() {
        cmd.arg("--platform").arg(platforms.join(","));
    }

    cmd.arg(".");
    cmd.current_dir(path);

    let output = cmd.output().map_err(|e| AdapterError::CommandFailed {
        command: "docker build".to_string(),
        reason: e.to_string(),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AdapterError::CommandFailed {
            command: "docker build".to_string(),
            reason: stderr.to_string(),
        }
        .into());
    }

    Ok(())
}

/// Push a Docker image to its registry.
pub fn push_image(tag: &str) -> Result<()> {
    let output = Command::new("docker")
        .args(["push", tag])
        .output()
        .map_err(|e| AdapterError::CommandFailed {
            command: "docker push".to_string(),
            reason: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(
            AdapterError::PublishFailed(format!("Failed to push {}: {}", tag, stderr)).into(),
        );
    }

    Ok(())
}

/// Tag a Docker image from `source` to `target`.
pub fn tag_image(source: &str, target: &str) -> Result<()> {
    let output = Command::new("docker")
        .args(["tag", source, target])
        .output()
        .map_err(|e| AdapterError::CommandFailed {
            command: "docker tag".to_string(),
            reason: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AdapterError::CommandFailed {
            command: "docker tag".to_string(),
            reason: stderr.to_string(),
        }
        .into());
    }

    Ok(())
}

/// Format a fully-qualified image tag for a given registry.
pub fn format_tag(registry: &str, name: &str, tag: &str) -> String {
    if registry == "docker.io" {
        format!("{}:{}", name, tag)
    } else {
        format!("{}/{}:{}", registry, name, tag)
    }
}

/// Format a base image reference (without tag) for a given registry.
pub fn format_base(registry: &str, name: &str) -> String {
    if registry == "docker.io" {
        name.to_string()
    } else {
        format!("{}/{}", registry, name)
    }
}
