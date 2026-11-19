use std::{
    io,
    path::{Path, PathBuf},
};

pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    #[cfg(target_os = "horizon")]
    return switch_absolute(path);
    #[cfg(not(target_os = "horizon"))]
    std::path::absolute(path)
}

#[cfg(any(test, target_os = "horizon"))]
fn switch_absolute(path: &Path) -> io::Result<PathBuf> {
    // libnx uses device roots such as hres:/ and sdmc:/, which Rust's
    // Unix Path treats as relative. Keep the prefix for libnx to resolve.
    let bytes = path.as_os_str().as_encoded_bytes();
    if let Some(colon) = bytes.iter().position(|&byte| byte == b':')
        && colon != 0
        && !bytes[..colon].contains(&b'/')
        && bytes.get(colon + 1) == Some(&b'/')
    {
        return Ok(path.to_path_buf());
    }
    std::path::absolute(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_startup_preserves_device_roots_and_resolves_relative_paths() {
        for raw in [
            "hres:/",
            "hres:/assets/minecraft",
            "sdmc:/switch/horizon/resources",
        ] {
            let path = Path::new(raw);
            assert_eq!(switch_absolute(path).unwrap(), path);
        }
        let relative = Path::new("resources");
        assert_eq!(
            switch_absolute(relative).unwrap(),
            std::path::absolute(relative).unwrap()
        );
    }
}
