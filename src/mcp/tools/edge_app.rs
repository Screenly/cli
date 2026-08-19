//! Edge App MCP tools.

use std::fs;

use serde_json::json;

use crate::authentication::Authentication;
use crate::commands;
use crate::commands::edge_app::manifest::{
    EdgeAppManifest, Entrypoint, EntrypointType, MANIFEST_VERSION,
};
use crate::commands::edge_app::EdgeAppCommand;

/// Marker attribute so wrap is idempotent when HTML is published again.
pub(crate) const THEME_BOOTSTRAP_MARKER: &str = "data-screenly-mcp-theme";

const SCREENLY_JS_SRC: &str = "screenly.js?version=1";

/// Theme bootstrap aligned with `@screenly/edge-apps` `setupTheme()`:
/// map Screenly branding settings onto CSS custom properties, then signal ready.
const THEME_BOOTSTRAP_SCRIPT: &str = r#"<script data-screenly-mcp-theme="1">
(function () {
  var settings = (window.screenly && screenly.settings) || {};
  var root = document.documentElement;
  function setVar(name, value) {
    if (value) root.style.setProperty(name, value);
  }
  var accent = settings.screenly_color_accent;
  var light = settings.screenly_color_light;
  var dark = settings.screenly_color_dark;
  setVar("--screenly-color-accent", accent);
  setVar("--screenly-color-light", light);
  setVar("--screenly-color-dark", dark);
  setVar("--theme-color-accent", accent);
  setVar("--theme-color-light", light);
  setVar("--theme-color-dark", dark);
  if (window.screenly && typeof screenly.signalReady === "function") {
    try { screenly.signalReady(); } catch (e) {}
  }
})();
</script>"#;

/// Edge App tools for the MCP server.
pub struct EdgeAppTools;

impl EdgeAppTools {
    /// List all Edge Apps.
    pub fn list(auth: &Authentication) -> Result<String, String> {
        let result = commands::get(auth, "v4/edge-apps?select=id,name&deleted=eq.false")
            .map_err(|e| format!("Failed to list Edge Apps: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// List settings for an Edge App.
    pub fn list_settings(auth: &Authentication, app_uuid: &str) -> Result<String, String> {
        let endpoint = format!(
            "v4.1/edge-apps/settings?app_id=eq.{}&select=name,type,default_value,optional,title,help_text&order=name.asc",
            app_uuid
        );
        let result = commands::get(auth, &endpoint)
            .map_err(|e| format!("Failed to list Edge App settings: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// List instances of an Edge App.
    pub fn list_instances(auth: &Authentication, app_uuid: &str) -> Result<String, String> {
        let endpoint = format!(
            "v4.1/edge-apps/installations?select=id,name&app_id=eq.{}",
            app_uuid
        );
        let result = commands::get(auth, &endpoint)
            .map_err(|e| format!("Failed to list Edge App instances: {}", e))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    /// Create or update an Edge App from HTML (Claude Artifact / webpage).
    ///
    /// Omitting `app_id` creates a new app. Passing `app_id` deploys a new revision,
    /// the same as `screenly edge-app deploy`.
    pub fn publish_from_html(
        auth: &Authentication,
        name: &str,
        html: &str,
        app_id: Option<&str>,
        description: Option<&str>,
    ) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("name is required".to_string());
        }

        let wrapped = wrap_html_for_edge_app(html)?;
        let dir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create temporary Edge App directory: {e}"))?;
        let dir_path = dir.path();

        fs::write(dir_path.join("index.html"), &wrapped)
            .map_err(|e| format!("Failed to write index.html: {e}"))?;

        let existing_id = app_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned);

        let created = existing_id.is_none();
        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: existing_id.clone(),
            description: description
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(ToOwned::to_owned),
            ready_signal: Some(true),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            ..Default::default()
        };

        let manifest_path = dir_path.join("screenly.yml");
        EdgeAppManifest::save_to_file(&manifest, &manifest_path)
            .map_err(|e| format!("Failed to write screenly.yml: {e}"))?;

        let command = edge_app_command(auth);
        if created {
            command
                .create_in_place(name, &manifest_path)
                .map_err(|e| format!("Failed to create Edge App: {e}"))?;
        }

        let app_id = EdgeAppManifest::new(&manifest_path)
            .map_err(|e| format!("Failed to read Edge App id: {e}"))?
            .id
            .ok_or_else(|| "Edge App id missing after create".to_string())?;

        let path = dir_path
            .to_str()
            .ok_or_else(|| "Edge App path is not valid UTF-8".to_string())?
            .to_string();

        let revision = command
            .deploy(Some(path), Some(false))
            .map_err(|e| format!("Failed to deploy Edge App: {e}"))?;

        serde_json::to_string_pretty(&json!({
            "app_id": app_id,
            "name": name,
            "revision": revision,
            "created": created,
            "message": "Reuse app_id with this tool to publish an HTML update as a new Edge App revision.",
        }))
        .map_err(|e| format!("Failed to serialize response: {e}"))
    }
}

fn edge_app_command(auth: &Authentication) -> EdgeAppCommand {
    EdgeAppCommand::new(Authentication {
        config: crate::authentication::Config {
            url: auth.config.url.clone(),
        },
        token: auth.token.clone(),
    })
}

/// Turn a Claude Artifact / HTML fragment into a player-ready Edge App document.
pub(crate) fn wrap_html_for_edge_app(html: &str) -> Result<String, String> {
    let html = html.trim();
    if html.is_empty() {
        return Err("html is empty".to_string());
    }

    let screenly_script = format!(r#"<script src="{SCREENLY_JS_SRC}"></script>"#);
    let lower = html.to_ascii_lowercase();
    let has_html_shell = lower.contains("<html") && lower.contains("</html>");

    if !has_html_shell {
        return Ok(format!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n{screenly_script}\n</head>\n<body>\n{html}\n{THEME_BOOTSTRAP_SCRIPT}\n</body>\n</html>\n"
        ));
    }

    let mut out = html.to_string();

    if !lower.contains("screenly.js") {
        out = inject_before_tag(&out, "</head>", &format!("{screenly_script}\n"))
            .or_else(|| inject_after_tag(&out, "<head>", &format!("\n{screenly_script}\n")))
            .ok_or_else(|| {
                "HTML document is missing a <head> element to inject screenly.js".to_string()
            })?;
    }

    if !out.contains(THEME_BOOTSTRAP_MARKER) {
        out = inject_before_tag(&out, "</body>", &format!("{THEME_BOOTSTRAP_SCRIPT}\n"))
            .or_else(|| inject_before_tag(&out, "</html>", &format!("{THEME_BOOTSTRAP_SCRIPT}\n")))
            .ok_or_else(|| {
                "HTML document is missing a </body> or </html> tag to inject theme bootstrap"
                    .to_string()
            })?;
    }

    if !out.to_ascii_lowercase().contains("<!doctype") {
        out = format!("<!DOCTYPE html>\n{out}");
    }

    Ok(out)
}

fn inject_before_tag(html: &str, tag: &str, snippet: &str) -> Option<String> {
    let idx = find_ascii_ignore_case(html, tag)?;
    let mut out = String::with_capacity(html.len() + snippet.len());
    out.push_str(&html[..idx]);
    out.push_str(snippet);
    out.push_str(&html[idx..]);
    Some(out)
}

fn inject_after_tag(html: &str, tag: &str, snippet: &str) -> Option<String> {
    let idx = find_ascii_ignore_case(html, tag)?;
    let insert_at = idx + tag.len();
    let mut out = String::with_capacity(html.len() + snippet.len());
    out.push_str(&html[..insert_at]);
    out.push_str(snippet);
    out.push_str(&html[insert_at..]);
    Some(out)
}

fn find_ascii_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
mod wrap_tests {
    use super::*;

    #[test]
    fn wrap_fragment_adds_shell_screenly_js_and_theme() {
        let wrapped = wrap_html_for_edge_app("<h1>Hello</h1>").unwrap();
        assert!(wrapped.contains("<!DOCTYPE html>"));
        assert!(wrapped.contains(SCREENLY_JS_SRC));
        assert!(wrapped.contains(THEME_BOOTSTRAP_MARKER));
        assert!(wrapped.contains("<h1>Hello</h1>"));
        assert!(wrapped.contains("--screenly-color-accent"));
    }

    #[test]
    fn wrap_full_document_injects_into_existing_head_and_body() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Board</title></head>
<body><p>News</p></body>
</html>"#;
        let wrapped = wrap_html_for_edge_app(html).unwrap();
        assert!(wrapped.contains(SCREENLY_JS_SRC));
        assert!(wrapped.contains("<title>Board</title>"));
        assert!(wrapped.contains("<p>News</p>"));
        let js_pos = wrapped.find(SCREENLY_JS_SRC).unwrap();
        let head_close = wrapped.to_ascii_lowercase().find("</head>").unwrap();
        assert!(js_pos < head_close);
    }

    #[test]
    fn wrap_is_idempotent_for_screenly_js_and_theme() {
        let once = wrap_html_for_edge_app("<div>Hi</div>").unwrap();
        let twice = wrap_html_for_edge_app(&once).unwrap();
        assert_eq!(twice.matches(SCREENLY_JS_SRC).count(), 1);
        assert_eq!(twice.matches(THEME_BOOTSTRAP_MARKER).count(), 1);
    }

    #[test]
    fn wrap_rejects_empty_html() {
        assert!(wrap_html_for_edge_app("   ").is_err());
    }
}
