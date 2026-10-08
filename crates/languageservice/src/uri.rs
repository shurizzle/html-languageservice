//! Minimal port of [vscode-uri](https://github.com/microsoft/vscode-uri)'s `URI`
//! (`src/uri.ts`), covering the subset used by `services/htmlLinks.ts`.
//!
//! Unlike the TypeScript `URI.parse`, which throws, [`Uri::parse`] returns a
//! [`Result`]; the caller treats `Err` like the thrown exception.

const EMPTY: &str = "";
const SLASH: &str = "/";

/// Uniform Resource Identifier components (scheme/authority/path/query/fragment).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Uri {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub query: String,
    pub fragment: String,
}

/// Returns true when `scheme` matches `/^\w[\w\d+.-]*$/`.
fn is_valid_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '.' || c == '-')
}

fn scheme_fix(scheme: &str, strict: bool) -> String {
    if scheme.is_empty() && !strict {
        return "file".to_string();
    }
    scheme.to_string()
}

/// Port of `_referenceResolution`.
fn reference_resolution(scheme: &str, path: &str) -> String {
    match scheme {
        "https" | "http" | "file" => {
            if path.is_empty() {
                SLASH.to_string()
            } else if !path.starts_with('/') {
                format!("/{path}")
            } else {
                path.to_string()
            }
        }
        _ => path.to_string(),
    }
}

/// Port of `_validateUri`; returns the error message the TS version throws.
fn validate_uri(ret: &Uri, strict: bool) -> Result<(), String> {
    if ret.scheme.is_empty() && strict {
        return Err(format!(
            "[UriError]: Scheme is missing: {{scheme: \"\", authority: \"{}\", path: \"{}\", query: \"{}\", fragment: \"{}\"}}",
            ret.authority, ret.path, ret.query, ret.fragment
        ));
    }
    if !ret.scheme.is_empty() && !is_valid_scheme(&ret.scheme) {
        return Err("[UriError]: Scheme contains illegal characters.".to_string());
    }
    if !ret.path.is_empty() {
        if !ret.authority.is_empty() {
            if !ret.path.starts_with('/') {
                return Err("[UriError]: If a URI contains an authority component, then the path component must either be empty or begin with a slash (\"/\") character".to_string());
            }
        } else if ret.path.starts_with("//") {
            return Err("[UriError]: If a URI does not contain an authority component, then the path cannot begin with two slash characters (\"//\")".to_string());
        }
    }
    Ok(())
}

/// Splits a stringified URI into raw components, mirroring `_regexp`.
fn split_components(value: &str) -> (String, String, String, String, String) {
    let bytes = value.as_bytes();
    let mut i = 0;

    // scheme: `(([^:/?#]+?):)?`
    let mut scheme = String::new();
    if let Some(colon) = value.find(':') {
        // Non-greedy: the run before ':' must not contain '/', '?' or '#'.
        if !value[..colon].contains(['/', '?', '#']) {
            scheme = value[..colon].to_string();
            i = colon + 1;
        }
    }

    // authority: `(//([^/?#]*))?`
    let mut authority = String::new();
    if bytes[i..].starts_with(b"//") {
        let start = i + 2;
        let end = value[start..]
            .find(['/', '?', '#'])
            .map(|p| start + p)
            .unwrap_or(value.len());
        authority = value[start..end].to_string();
        i = end;
    }

    // path: `([^?#]*)`
    let path_end = value[i..]
        .find(['?', '#'])
        .map(|p| i + p)
        .unwrap_or(value.len());
    let path = value[i..path_end].to_string();
    i = path_end;

    // query: `(\?([^#]*))?`
    let mut query = String::new();
    if bytes.get(i) == Some(&b'?') {
        let start = i + 1;
        let end = value[start..]
            .find('#')
            .map(|p| start + p)
            .unwrap_or(value.len());
        query = value[start..end].to_string();
        i = end;
    }

    // fragment: `(#(.*))?`
    let mut fragment = String::new();
    if bytes.get(i) == Some(&b'#') {
        fragment = value[i + 1..].to_string();
    }

    (scheme, authority, path, query, fragment)
}

impl Uri {
    /// Constructs a URI from components, applying `_schemeFix`,
    /// `_referenceResolution` and validation (non-strict).
    fn from_components(
        scheme: String,
        authority: String,
        path: String,
        query: String,
        fragment: String,
        strict: bool,
    ) -> Result<Uri, String> {
        let scheme = scheme_fix(&scheme, strict);
        let path = reference_resolution(&scheme, &path);
        let uri = Uri {
            scheme,
            authority,
            path,
            query,
            fragment,
        };
        validate_uri(&uri, strict)?;
        Ok(uri)
    }

    /// Port of `URI.parse(value, false)`.
    pub fn parse(value: &str) -> Result<Uri, String> {
        let (scheme, authority, path, query, fragment) = split_components(value);
        Uri::from_components(
            scheme,
            percent_decode(&authority),
            percent_decode(&path),
            percent_decode(&query),
            percent_decode(&fragment),
            false,
        )
    }

    /// Port of `uri.with({ query })` where `None` maps to the empty string.
    pub fn with_query(&self, query: Option<&str>) -> Result<Uri, String> {
        Uri::from_components(
            self.scheme.clone(),
            self.authority.clone(),
            self.path.clone(),
            query.unwrap_or(EMPTY).to_string(),
            self.fragment.clone(),
            false,
        )
    }

    /// Port of `uri.with({ fragment })` where `None` maps to the empty string.
    pub fn with_fragment(&self, fragment: Option<&str>) -> Result<Uri, String> {
        Uri::from_components(
            self.scheme.clone(),
            self.authority.clone(),
            self.path.clone(),
            self.query.clone(),
            fragment.unwrap_or(EMPTY).to_string(),
            false,
        )
    }

    /// Port of `uri.toString(skipEncoding)`.
    pub fn to_string_with(&self, skip_encoding: bool) -> String {
        as_formatted(self, skip_encoding)
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string_with(false))
    }
}

/// Encoder selection used by [`as_formatted`].
fn enc(value: &str, is_path: bool, is_authority: bool, skip_encoding: bool) -> String {
    if skip_encoding {
        encode_minimal(value)
    } else {
        encode_fast(value, is_path, is_authority)
    }
}

/// Port of `_asFormatted`.
fn as_formatted(uri: &Uri, skip_encoding: bool) -> String {
    let mut res = String::new();

    if !uri.scheme.is_empty() {
        res.push_str(&uri.scheme);
        res.push(':');
    }
    if !uri.authority.is_empty() || uri.scheme == "file" {
        res.push_str("//");
    }

    let mut authority = uri.authority.clone();
    if !authority.is_empty() {
        if let Some(idx) = authority.find('@') {
            let userinfo = authority[..idx].to_string();
            authority = authority[idx + 1..].to_string();
            match userinfo.rfind(':') {
                None => res.push_str(&enc(&userinfo, false, false, skip_encoding)),
                Some(i) => {
                    res.push_str(&enc(&userinfo[..i], false, false, skip_encoding));
                    res.push(':');
                    res.push_str(&enc(&userinfo[i + 1..], false, true, skip_encoding));
                }
            }
            res.push('@');
        }
        authority = authority.to_lowercase();
        match authority.rfind(':') {
            None => res.push_str(&enc(&authority, false, true, skip_encoding)),
            Some(idx) => {
                res.push_str(&enc(&authority[..idx], false, true, skip_encoding));
                res.push_str(&authority[idx..]);
            }
        }
    }

    if !uri.path.is_empty() {
        let mut path = uri.path.clone();
        // Lower-case windows drive letters in /C:/fff or C:/fff.
        let bytes = path.as_bytes();
        if path.len() >= 3 && bytes[0] == b'/' && bytes[2] == b':' {
            let code = bytes[1];
            if code.is_ascii_uppercase() {
                path = format!("/{}:{}", (code + 32) as char, &path[3..]);
            }
        } else if path.len() >= 2 && bytes[1] == b':' {
            let code = bytes[0];
            if code.is_ascii_uppercase() {
                path = format!("{}:{}", (code + 32) as char, &path[2..]);
            }
        }
        res.push_str(&enc(&path, true, false, skip_encoding));
    }

    if !uri.query.is_empty() {
        res.push('?');
        res.push_str(&enc(&uri.query, false, false, skip_encoding));
    }

    if !uri.fragment.is_empty() {
        res.push('#');
        if skip_encoding {
            res.push_str(&uri.fragment);
        } else {
            res.push_str(&encode_fast(&uri.fragment, false, false));
        }
    }

    res
}

fn encode_table(code: u8) -> Option<&'static str> {
    Some(match code {
        b':' => "%3A",
        b'/' => "%2F",
        b'?' => "%3F",
        b'#' => "%23",
        b'[' => "%5B",
        b']' => "%5D",
        b'@' => "%40",
        b'!' => "%21",
        b'$' => "%24",
        b'&' => "%26",
        b'\'' => "%27",
        b'(' => "%28",
        b')' => "%29",
        b'*' => "%2A",
        b'+' => "%2B",
        b',' => "%2C",
        b';' => "%3B",
        b'=' => "%3D",
        b' ' => "%20",
        _ => return None,
    })
}

/// Percent-encodes like JavaScript's `encodeURIComponent`.
fn encode_uri_component(s: &str) -> String {
    let mut res = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            res.push(b as char);
        } else {
            res.push_str(&format!("%{b:02X}"));
        }
    }
    res
}

/// Port of `encodeURIComponentFast`.
fn encode_fast(component: &str, is_path: bool, is_authority: bool) -> String {
    let mut res: Option<String> = None;
    let mut native_pos: Option<usize> = None;
    let bytes = component.as_bytes();

    let mut pos = 0;
    while pos < bytes.len() {
        let code = bytes[pos];
        let unreserved = code.is_ascii_alphanumeric()
            || matches!(code, b'-' | b'.' | b'_' | b'~')
            || (is_path && code == b'/')
            || (is_authority && code == b'[')
            || (is_authority && code == b']')
            || (is_authority && code == b':');

        if unreserved {
            if let Some(np) = native_pos.take() {
                res.get_or_insert_with(String::new)
                    .push_str(&encode_uri_component(&component[np..pos]));
            }
            if let Some(r) = res.as_mut() {
                r.push(code as char);
            }
        } else {
            if res.is_none() {
                res = Some(component[..pos].to_string());
            }
            if let Some(escaped) = encode_table(code) {
                if let Some(np) = native_pos.take() {
                    res.as_mut()
                        .unwrap()
                        .push_str(&encode_uri_component(&component[np..pos]));
                }
                res.as_mut().unwrap().push_str(escaped);
            } else if native_pos.is_none() {
                native_pos = Some(pos);
            }
        }
        pos += 1;
    }

    if let Some(np) = native_pos {
        res.as_mut()
            .unwrap()
            .push_str(&encode_uri_component(&component[np..]));
    }

    res.unwrap_or_else(|| component.to_string())
}

/// Port of `encodeURIComponentMinimal`: escapes only `#` and `?`.
fn encode_minimal(path: &str) -> String {
    let mut res: Option<String> = None;
    for (pos, ch) in path.char_indices() {
        if ch == '#' || ch == '?' {
            let r = res.get_or_insert_with(|| path[..pos].to_string());
            r.push_str(encode_table(ch as u8).unwrap());
        } else if let Some(r) = res.as_mut() {
            r.push(ch);
        }
    }
    res.unwrap_or_else(|| path.to_string())
}

/// Port of `decodeURIComponentGraceful`.
fn decode_uri_component_graceful(s: &str) -> String {
    match decode_uri_component(s) {
        Ok(decoded) => decoded,
        Err(()) => {
            if s.len() > 3 {
                format!("{}{}", &s[..3], decode_uri_component_graceful(&s[3..]))
            } else {
                s.to_string()
            }
        }
    }
}

/// Approximates `decodeURIComponent`, returning `Err` where it would throw.
fn decode_uri_component(s: &str) -> Result<String, ()> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        // Literal char: push its UTF-8 encoding.
        let ch = s[i..].chars().next().unwrap();
        let mut buf = [0u8; 4];
        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        i += ch.len_utf8();
    }
    String::from_utf8(out).map_err(|_| ())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Port of `percentDecode`, replacing each run matching
/// `(%[0-9A-Za-z][0-9A-Za-z])+`.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_alphanumeric()
            && bytes[i + 2].is_ascii_alphanumeric()
        {
            let start = i;
            while i + 2 < bytes.len()
                && bytes[i] == b'%'
                && bytes[i + 1].is_ascii_alphanumeric()
                && bytes[i + 2].is_ascii_alphanumeric()
            {
                i += 3;
            }
            out.push_str(&decode_uri_component_graceful(&s[start..i]));
            changed = true;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    if changed { out } else { s.to_string() }
}
