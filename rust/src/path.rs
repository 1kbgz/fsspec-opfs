pub(crate) fn normalize(path: &str) -> Result<String, &'static str> {
    let path = path
        .strip_prefix("opfs://")
        .unwrap_or(path)
        .trim_matches('/');
    if path.contains(['\\', '\0']) || path.split('/').any(|p| p == "." || p == "..") {
        return Err("OPFS paths cannot contain dot components, backslashes, or NUL");
    }
    Ok(path
        .split('/')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("/"))
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn normalizes_paths_and_rejects_unsafe_components() {
        for (input, expected) in [
            ("", ""),
            ("/", ""),
            ("opfs://", ""),
            ("opfs:///data//rows.parquet/", "data/rows.parquet"),
            ("/data///rows.parquet", "data/rows.parquet"),
            (".hidden/report..csv", ".hidden/report..csv"),
            ("data/café.csv", "data/café.csv"),
        ] {
            assert_eq!(normalize(input), Ok(expected.into()), "{input:?}");
        }
        for input in [
            ".",
            "..",
            "data/./file",
            "data/../file",
            "opfs://../file",
            "data\\file",
            "data\0file",
        ] {
            assert!(normalize(input).is_err(), "{input:?}");
        }
    }
}
