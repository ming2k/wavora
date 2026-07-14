use std::path::{Path, PathBuf};

#[must_use]
pub fn path_to_file_uri(path: &Path) -> String {
    let path = path.to_string_lossy();
    let mut encoded = String::with_capacity(path.len() + 7);
    encoded.push_str("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[must_use]
pub fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_round_trip_handles_unicode_and_spaces() {
        let path = Path::new("/tmp/音乐/Hello World.flac");
        let uri = path_to_file_uri(path);
        assert_eq!(file_uri_to_path(&uri).as_deref(), Some(path));
    }
}
