//! Edge App MCP tools.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::authentication::Authentication;
use crate::commands;
use crate::commands::edge_app::app::app_id_override;
use crate::commands::edge_app::manifest::{
    EdgeAppManifest, Entrypoint, EntrypointType, MANIFEST_VERSION,
};
use crate::commands::edge_app::utils::{
    transform_edge_app_path_to_manifest, transform_instance_path_to_instance_manifest,
};
use crate::commands::edge_app::EdgeAppCommand;

/// Override path for the local name → app/instance cache (used in tests).
const MCP_EDGE_APPS_PATH_ENV: &str = "SCREENLY_MCP_EDGE_APPS_PATH";

/// Directory next to the `~/.screenly` *token file* — that path is a regular
/// file (`authentication.rs`), so we cannot store JSON under `~/.screenly/`.
const MCP_EDGE_APPS_DIR_NAME: &str = ".screenly.d";
const MCP_EDGE_APPS_FILE_NAME: &str = "mcp-edge-apps.json";

/// Default Edge App icon for Claude Artifact publishes (screenly.yml `icon`).
const DEFAULT_CLAUDE_APP_ICON: &str =
    "https://playground.srly.io/edge-apps/icons/claude-app-default.svg";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct McpEdgeAppRecord {
    app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct McpEdgeAppRegistry {
    /// `{api_url}|{token fingerprint}` → display name → last published ids.
    #[serde(default)]
    scopes: BTreeMap<String, BTreeMap<String, McpEdgeAppRecord>>,
}

/// Marker attribute so wrap is idempotent when HTML is published again.
const THEME_BOOTSTRAP_MARKER: &str = "data-screenly-mcp-theme";
const SIGNAGE_STYLE_MARKER: &str = "data-screenly-mcp-signage";

const SCREENLY_JS_SRC: &str = "screenly.js?version=1";

const SIGNAGE_STYLE: &str = r#"<style data-screenly-mcp-signage="1">
html, body { height: 100%; margin: 0; }
html { -webkit-user-select: none; user-select: none; }
a, button, [role="tab"], [role="button"] { cursor: default; }
</style>"#;

/// Theme CSS variables, then rotate only explicit slideshow markers
/// (`[role="tabpanel"]`, `.carousel-item`, `[data-slide]`, `[data-screenly-page]`).
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

  var DWELL_MS = 8000;

  function showOnly(items, index) {
    items.forEach(function (el, i) {
      var on = i === index;
      if (!el.hasAttribute("data-screenly-orig-display")) {
        el.setAttribute("data-screenly-orig-display", el.style.display);
      }
      var orig = el.getAttribute("data-screenly-orig-display");
      if (on) {
        el.removeAttribute("hidden");
        el.style.display = orig;
        if (window.getComputedStyle(el).display === "none") el.style.display = "block";
      } else {
        el.setAttribute("hidden", "");
        el.style.display = "none";
      }
      el.setAttribute("aria-hidden", on ? "false" : "true");
      el.classList.toggle("active", on);
      el.classList.toggle("show", on);
    });
  }

  function collectPages() {
    var selectors = [
      '[role="tabpanel"]',
      ".carousel-item",
      "[data-slide]",
      "[data-screenly-page]"
    ];
    for (var i = 0; i < selectors.length; i++) {
      var found = document.querySelectorAll(selectors[i]);
      if (found.length > 1) return Array.prototype.slice.call(found);
    }
    return [];
  }

  function startSignage() {
    var tabs = document.querySelectorAll('[role="tab"]');
    if (tabs.length > 1) {
      var tabIndex = 0;
      try { tabs[0].click(); } catch (e) {}
      setInterval(function () {
        tabIndex = (tabIndex + 1) % tabs.length;
        try { tabs[tabIndex].click(); } catch (e) {}
      }, DWELL_MS);
      return;
    }

    var pages = collectPages();
    if (pages.length > 1) {
      var pageIndex = 0;
      showOnly(pages, 0);
      setInterval(function () {
        pageIndex = (pageIndex + 1) % pages.length;
        showOnly(pages, pageIndex);
      }, DWELL_MS);
      return;
    }

    var scroller = document.scrollingElement || document.documentElement;
    if (scroller && scroller.scrollHeight > scroller.clientHeight + 40) {
      var dir = 1;
      setInterval(function () {
        var max = scroller.scrollHeight - scroller.clientHeight;
        scroller.scrollTop += dir;
        if (scroller.scrollTop >= max) dir = -1;
        if (scroller.scrollTop <= 0) dir = 1;
      }, 40);
    }
  }

  function onReady(fn) {
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", fn);
    } else {
      fn();
    }
  }

  onReady(function () {
    try { startSignage(); } catch (e) {}
    if (!window.screenly) return;
    if (typeof screenly.signalReadyForRendering === "function") {
      try { screenly.signalReadyForRendering(); } catch (e) {}
    } else if (typeof screenly.signalReady === "function") {
      try { screenly.signalReady(); } catch (e) {}
    }
  });
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
    /// Omitting `app_id` creates a new app unless the same `name` was published
    /// before on this machine (`~/.screenly.d/mcp-edge-apps.json`). Passing
    /// `app_id` always deploys a new revision for that app.
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
        let ready_signal = ready_signal_for_html(&wrapped);
        let dir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create temporary Edge App directory: {}", e))?;
        let dir_path = dir.path();
        let (manifest_path, instance_manifest_path, path) = publish_dir_paths(dir_path)?;

        fs::write(dir_path.join("index.html"), &wrapped)
            .map_err(|e| format!("Failed to write index.html: {}", e))?;

        let explicit_id = app_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned);
        let remembered = lookup_remembered_app(auth, name);
        let from_memory = explicit_id.is_none() && remembered.is_some();
        let existing_id = explicit_id
            .clone()
            .or_else(|| remembered.as_ref().map(|r| r.app_id.clone()));
        let preferred_instance_id = remembered.as_ref().and_then(|r| {
            if existing_id.as_deref() == Some(r.app_id.as_str()) {
                r.instance_id.clone()
            } else {
                None
            }
        });

        let created = existing_id.is_none();
        let manifest = EdgeAppManifest {
            syntax: MANIFEST_VERSION.to_owned(),
            id: existing_id,
            description: description
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(ToOwned::to_owned),
            icon: Some(DEFAULT_CLAUDE_APP_ICON.to_owned()),
            ready_signal: Some(ready_signal),
            entrypoint: Some(Entrypoint {
                entrypoint_type: EntrypointType::File,
                uri: None,
            }),
            ..Default::default()
        };

        EdgeAppManifest::save_to_file(&manifest, &manifest_path)
            .map_err(|e| format!("Failed to write {}: {}", manifest_path.display(), e))?;

        let command = edge_app_command(auth);
        if created {
            command
                .create_in_place(name, &manifest_path)
                .map_err(|e| format!("Failed to create Edge App: {}", e))?;
        }

        let app_id = EdgeAppManifest::new(&manifest_path)
            .map_err(|e| format!("Failed to read Edge App id: {}", e))?
            .id
            .or_else(app_id_override)
            .ok_or_else(|| "Edge App id missing after create".to_string())?;

        let revision = command
            .deploy(Some(app_id.clone()), Some(path), Some(false))
            .map_err(|e| format!("Failed to deploy Edge App: {}", e))?;

        // Create/deploy already happened. Instance + local memory must not hide app_id.
        let mut warnings: Vec<String> = Vec::new();
        let (instance_id, instance_created) = match ensure_instance(
            auth,
            &app_id,
            name,
            preferred_instance_id.as_deref(),
            &instance_manifest_path,
        ) {
            Ok(pair) => pair,
            Err(e) => {
                warnings.push(e);
                (None, false)
            }
        };

        let saved_to_memory =
            match remember_published_app(auth, name, &app_id, instance_id.as_deref()) {
                Ok(()) => true,
                Err(e) => {
                    warnings.push(format!("Failed to save app_id locally: {}", e));
                    false
                }
            };

        serialize_publish_success(PublishFromHtmlResponse {
            app_id,
            instance_id,
            instance_created,
            name: name.to_string(),
            revision,
            created,
            resolved_from_memory: from_memory,
            saved_to_memory,
            warnings,
            message: publish_follow_up_message(saved_to_memory).to_string(),
        })
    }
}

#[derive(Serialize)]
struct PublishFromHtmlResponse {
    app_id: String,
    instance_id: Option<String>,
    instance_created: bool,
    name: String,
    revision: u32,
    created: bool,
    resolved_from_memory: bool,
    saved_to_memory: bool,
    warnings: Vec<String>,
    message: String,
}

fn publish_follow_up_message(saved_to_memory: bool) -> &'static str {
    if saved_to_memory {
        "IDs are saved locally under ~/.screenly.d/mcp-edge-apps.json for this name. Later, call this tool again with the same name (and updated HTML) to deploy a new revision; app_id is optional when the name is remembered."
    } else {
        "Keep this app_id. Local memory could not be updated; pass app_id on the next publish to update the same app."
    }
}

fn serialize_publish_success(response: PublishFromHtmlResponse) -> Result<String, String> {
    serde_json::to_string_pretty(&response)
        .map_err(|e| format!("Failed to serialize response: {}", e))
}

fn publish_dir_paths(dir_path: &Path) -> Result<(PathBuf, PathBuf, String), String> {
    let path = dir_path
        .to_str()
        .ok_or_else(|| "Edge App path is not valid UTF-8".to_string())?
        .to_string();
    let manifest_path = transform_edge_app_path_to_manifest(&Some(path.clone()))
        .map_err(|e| format!("Failed to resolve Edge App manifest path: {}", e))?;
    let instance_path = transform_instance_path_to_instance_manifest(&Some(path.clone()))
        .map_err(|e| format!("Failed to resolve Edge App instance path: {}", e))?;
    Ok((manifest_path, instance_path, path))
}

fn registry_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var(MCP_EDGE_APPS_PATH_ENV) {
        let path = path.trim();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = dirs::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
    Ok(home
        .join(MCP_EDGE_APPS_DIR_NAME)
        .join(MCP_EDGE_APPS_FILE_NAME))
}

fn load_registry() -> Result<McpEdgeAppRegistry, String> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(McpEdgeAppRegistry::default());
    }

    let data = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    if data.trim().is_empty() {
        return Ok(McpEdgeAppRegistry::default());
    }

    serde_json::from_str(&data).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

fn save_registry(registry: &McpEdgeAppRegistry) -> Result<(), String> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }

    let data = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Failed to serialize Edge App memory: {}", e))?;
    fs::write(&path, format!("{data}\n"))
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn registry_scope(auth: &Authentication) -> String {
    let url = auth.config.url.trim().trim_end_matches('/');
    format!("{url}|{}", token_fingerprint(&auth.token))
}

fn token_fingerprint(token: &str) -> String {
    hex::encode(Sha256::digest(token.trim().as_bytes()))
}

fn lookup_in_registry(auth: &Authentication, name: &str) -> Option<McpEdgeAppRecord> {
    load_registry()
        .ok()?
        .scopes
        .get(&registry_scope(auth))?
        .get(name)
        .cloned()
}

/// Returns the remembered ids for this API host + token, or `None` if the app
/// is gone (entry is then dropped). Network errors keep the cached id.
fn lookup_remembered_app(auth: &Authentication, name: &str) -> Option<McpEdgeAppRecord> {
    let record = lookup_in_registry(auth, name)?;
    match app_exists_in_account(auth, &record.app_id) {
        Ok(true) => Some(record),
        Ok(false) => {
            let _ = forget_published_app(auth, name);
            None
        }
        Err(_) => Some(record),
    }
}

fn remember_published_app(
    auth: &Authentication,
    name: &str,
    app_id: &str,
    instance_id: Option<&str>,
) -> Result<(), String> {
    let mut registry = load_registry()?;
    registry
        .scopes
        .entry(registry_scope(auth))
        .or_default()
        .insert(
            name.to_string(),
            McpEdgeAppRecord {
                app_id: app_id.to_string(),
                instance_id: instance_id.map(ToOwned::to_owned),
            },
        );
    save_registry(&registry)
}

fn forget_published_app(auth: &Authentication, name: &str) -> Result<(), String> {
    let mut registry = load_registry()?;
    let scope = registry_scope(auth);
    let empty = if let Some(apps) = registry.scopes.get_mut(&scope) {
        apps.remove(name);
        apps.is_empty()
    } else {
        false
    };
    if empty {
        registry.scopes.remove(&scope);
    }
    save_registry(&registry)
}

fn app_exists_in_account(auth: &Authentication, app_id: &str) -> Result<bool, String> {
    let endpoint = format!("v4/edge-apps?select=id&id=eq.{app_id}&deleted=eq.false");
    let result = commands::get(auth, &endpoint)
        .map_err(|e| format!("Failed to look up Edge App {}: {}", app_id, e))?;
    Ok(result
        .as_array()
        .map(|rows| !rows.is_empty())
        .unwrap_or(false))
}

fn ensure_instance(
    auth: &Authentication,
    app_id: &str,
    name: &str,
    preferred_instance_id: Option<&str>,
    instance_manifest_path: &Path,
) -> Result<(Option<String>, bool), String> {
    let command = edge_app_command(auth);
    let listed = command
        .list_instances(app_id)
        .map_err(|e| format!("Failed to list Edge App instances: {}", e))?;

    let rows = listed.value.as_array().cloned().unwrap_or_default();
    if let Some(id) = pick_existing_instance(&rows, preferred_instance_id, name) {
        return Ok((Some(id), false));
    }

    let instance_id = command
        .create_instance(instance_manifest_path, app_id, name)
        .map_err(|e| format!("Failed to create Edge App instance: {}", e))?;
    Ok((Some(instance_id), true))
}

fn pick_existing_instance(
    rows: &[serde_json::Value],
    preferred_instance_id: Option<&str>,
    name: &str,
) -> Option<String> {
    if let Some(preferred) = preferred_instance_id.filter(|id| !id.is_empty()) {
        if rows
            .iter()
            .any(|row| row.get("id").and_then(|v| v.as_str()) == Some(preferred))
        {
            return Some(preferred.to_string());
        }
    }

    if let Some(id) = rows.iter().find_map(|row| {
        if row.get("name").and_then(|v| v.as_str()) == Some(name) {
            row.get("id")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        } else {
            None
        }
    }) {
        return Some(id);
    }

    if rows.len() == 1 {
        return rows[0]
            .get("id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
    }

    None
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
            "<!DOCTYPE html>\n\
             <html lang=\"en\">\n\
             <head>\n\
             <meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n\
             {screenly_script}\n\
             {SIGNAGE_STYLE}\n\
             </head>\n\
             <body>\n\
             {html}\n\
             {THEME_BOOTSTRAP_SCRIPT}\n\
             </body>\n\
             </html>\n"
        ));
    }

    let mut out = html.to_string();

    if !has_screenly_js_script(&out) {
        out = inject_before_tag(&out, "</head>", &format!("{screenly_script}\n"))
            .or_else(|| inject_after_tag(&out, "<head>", &format!("\n{screenly_script}\n")))
            .ok_or_else(|| {
                "HTML document is missing a <head> element to inject screenly.js".to_string()
            })?;
    }

    if !out.contains(SIGNAGE_STYLE_MARKER) {
        out = inject_before_tag(&out, "</head>", &format!("{SIGNAGE_STYLE}\n"))
            .or_else(|| inject_after_tag(&out, "<head>", &format!("\n{SIGNAGE_STYLE}\n")))
            .ok_or_else(|| {
                "HTML document is missing a <head> element to inject signage styles".to_string()
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

/// True when an opening `<script src=…>` tag points at `screenly.js`.
/// Comments and inline script text that merely mention the filename do not count.
fn has_screenly_js_script(html: &str) -> bool {
    for_each_tag_outside_comments(html, |tag| {
        opening_tag_name(tag) == "script" && tag.contains("src=") && tag.contains("screenly.js")
    })
}

/// `<base href>` makes relative `screenly.js` resolve off-origin, so the player
/// never gets `window.screenly` and must not wait on `ready_signal`.
fn html_has_base_tag(html: &str) -> bool {
    for_each_tag_outside_comments(html, |tag| opening_tag_name(tag) == "base")
}

fn ready_signal_for_html(html: &str) -> bool {
    has_screenly_js_script(html) && !html_has_base_tag(html)
}

fn opening_tag_name(tag: &str) -> &str {
    tag.trim_start()
        .split(|c: char| c.is_ascii_whitespace() || c == '/')
        .next()
        .unwrap_or("")
}

/// Walks opening-tag interiors (`foo bar=baz` from `<foo bar=baz>`), skipping `<!-- -->`.
fn for_each_tag_outside_comments(html: &str, mut on_tag: impl FnMut(&str) -> bool) -> bool {
    let lower = html.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while !rest.is_empty() {
        if let Some(comment_at) = rest.find("<!--") {
            let before = &rest[..comment_at];
            if scan_tags(before, &mut on_tag) {
                return true;
            }
            rest = &rest[comment_at + 4..];
            match rest.find("-->") {
                Some(end) => rest = &rest[end + 3..],
                None => break,
            }
            continue;
        }
        return scan_tags(rest, &mut on_tag);
    }
    false
}

fn scan_tags(html: &str, on_tag: &mut impl FnMut(&str) -> bool) -> bool {
    let mut rest = html;
    while let Some(idx) = rest.find('<') {
        let after = &rest[idx + 1..];
        if after.starts_with('!') || after.starts_with('/') {
            rest = after;
            continue;
        }
        let tag_end = after.find('>').unwrap_or(after.len());
        if on_tag(&after[..tag_end]) {
            return true;
        }
        rest = &after[tag_end.min(after.len())..];
    }
    false
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
        assert!(wrapped.contains(SIGNAGE_STYLE_MARKER));
        assert!(wrapped.contains("[role=\"tab\"]"));
        assert!(wrapped.contains("DWELL_MS"));
        assert!(wrapped.contains("signalReadyForRendering"));
        assert!(wrapped.contains("data-screenly-orig-display"));
        assert!(wrapped.contains("getComputedStyle(el).display === \"none\""));
        assert!(!wrapped.contains(".tile"));
        assert!(!wrapped.contains("querySelectorAll(\"dialog\")"));
        assert!(!wrapped.contains("querySelectorAll(\"body *\")"));
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
        assert_eq!(twice.matches(SIGNAGE_STYLE_MARKER).count(), 1);
    }

    #[test]
    fn wrap_rejects_empty_html() {
        assert!(wrap_html_for_edge_app("   ").is_err());
    }

    #[test]
    fn wrap_injects_screenly_js_when_filename_only_appears_in_a_comment() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<!-- loads screenly.js on the player -->
<title>Board</title>
</head>
<body><p>News</p></body>
</html>"#;
        let wrapped = wrap_html_for_edge_app(html).unwrap();
        assert_eq!(wrapped.matches(SCREENLY_JS_SRC).count(), 1);
        assert!(has_screenly_js_script(&wrapped));
    }

    #[test]
    fn wrap_skips_inject_when_script_src_already_loads_screenly_js() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<script src="screenly.js?version=1"></script>
<title>Board</title>
</head>
<body><p>News</p></body>
</html>"#;
        let wrapped = wrap_html_for_edge_app(html).unwrap();
        assert_eq!(wrapped.matches("screenly.js").count(), 1);
        assert!(has_screenly_js_script(&wrapped));
    }

    #[test]
    fn has_screenly_js_script_ignores_inline_script_text() {
        let html = r#"<script>console.log("screenly.js")</script>"#;
        assert!(!has_screenly_js_script(html));
    }

    #[test]
    fn wrap_injects_when_screenly_js_script_is_only_inside_a_comment() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<!-- <script src="screenly.js"></script> -->
<title>Board</title>
</head>
<body><p>News</p></body>
</html>"#;
        assert!(!has_screenly_js_script(html));
        let wrapped = wrap_html_for_edge_app(html).unwrap();
        assert_eq!(wrapped.matches(SCREENLY_JS_SRC).count(), 1);
        assert!(has_screenly_js_script(&wrapped));
        assert!(ready_signal_for_html(&wrapped));
    }

    #[test]
    fn ready_signal_is_off_when_document_has_a_base_tag() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<base href="https://example.com/">
<title>Board</title>
</head>
<body><p>News</p></body>
</html>"#;
        let wrapped = wrap_html_for_edge_app(html).unwrap();
        assert!(has_screenly_js_script(&wrapped));
        assert!(html_has_base_tag(&wrapped));
        assert!(!ready_signal_for_html(&wrapped));
    }
}

#[cfg(test)]
mod registry_tests {
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::authentication::Config;

    fn test_auth(url: &str, token: &str) -> Authentication {
        Authentication::new_with_config(Config::new(url.to_string()), token)
    }

    fn with_registry<R>(f: impl FnOnce() -> R) -> R {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mcp-edge-apps.json");
        let path_str = path.to_str().unwrap().to_string();
        temp_env::with_var(MCP_EDGE_APPS_PATH_ENV, Some(path_str.as_str()), f)
    }

    #[test]
    fn remember_and_lookup_round_trip() {
        with_registry(|| {
            let auth = test_auth("https://api.example.com", "token-a");
            assert!(lookup_in_registry(&auth, "Lobby Board").is_none());
            remember_published_app(&auth, "Lobby Board", "app-1", Some("inst-1")).unwrap();

            let remembered = lookup_in_registry(&auth, "Lobby Board").unwrap();
            assert_eq!(remembered.app_id, "app-1");
            assert_eq!(remembered.instance_id.as_deref(), Some("inst-1"));

            remember_published_app(&auth, "Lobby Board", "app-1", Some("inst-2")).unwrap();
            let updated = lookup_in_registry(&auth, "Lobby Board").unwrap();
            assert_eq!(updated.instance_id.as_deref(), Some("inst-2"));
        });
    }

    #[test]
    fn registry_is_scoped_by_api_url_and_token() {
        with_registry(|| {
            let prod_a = test_auth("https://api.example.com", "token-a");
            let prod_b = test_auth("https://api.example.com", "token-b");
            let staging_a = test_auth("https://staging.example.com", "token-a");

            remember_published_app(&prod_a, "Lobby Board", "app-prod-a", None).unwrap();

            assert!(lookup_in_registry(&prod_b, "Lobby Board").is_none());
            assert!(lookup_in_registry(&staging_a, "Lobby Board").is_none());
            assert_eq!(
                lookup_in_registry(&prod_a, "Lobby Board").unwrap().app_id,
                "app-prod-a"
            );
        });
    }

    #[test]
    fn stale_remembered_id_is_forgotten() {
        with_registry(|| {
            let server = MockServer::start();
            server.mock(|when, then| {
                when.method(GET).path("/v4/edge-apps");
                then.status(200).json_body(json!([]));
            });
            let auth = test_auth(&server.base_url(), "token-a");
            remember_published_app(&auth, "Lobby Board", "gone-app", None).unwrap();
            assert!(lookup_in_registry(&auth, "Lobby Board").is_some());
            assert!(lookup_remembered_app(&auth, "Lobby Board").is_none());
            assert!(lookup_in_registry(&auth, "Lobby Board").is_none());
        });
    }

    #[test]
    fn default_registry_path_is_not_the_token_file() {
        temp_env::with_var_unset(MCP_EDGE_APPS_PATH_ENV, || {
            let path = registry_path().unwrap();
            assert_eq!(
                path.file_name().and_then(|n| n.to_str()),
                Some(MCP_EDGE_APPS_FILE_NAME)
            );
            assert_eq!(
                path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str()),
                Some(MCP_EDGE_APPS_DIR_NAME)
            );
        });
    }

    #[test]
    fn success_json_keeps_app_id_when_bookkeeping_fails() {
        let raw = serialize_publish_success(PublishFromHtmlResponse {
            app_id: "app-1".to_string(),
            instance_id: None,
            instance_created: false,
            name: "Lobby Board".to_string(),
            revision: 3,
            created: true,
            resolved_from_memory: false,
            saved_to_memory: false,
            warnings: vec!["Failed to save app_id locally: File exists".to_string()],
            message: publish_follow_up_message(false).to_string(),
        })
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["app_id"], "app-1");
        assert_eq!(value["saved_to_memory"], false);
        assert_eq!(value["warnings"].as_array().unwrap().len(), 1);
        assert!(value["message"]
            .as_str()
            .unwrap()
            .contains("Keep this app_id"));
    }

    #[test]
    fn pick_existing_instance_prefers_remembered_id_then_name() {
        let rows = vec![
            json!({"id": "inst-a", "name": "Other"}),
            json!({"id": "inst-b", "name": "Lobby Board"}),
        ];
        assert_eq!(
            pick_existing_instance(&rows, Some("inst-a"), "Lobby Board").as_deref(),
            Some("inst-a")
        );
        assert_eq!(
            pick_existing_instance(&rows, Some("gone"), "Lobby Board").as_deref(),
            Some("inst-b")
        );
        assert_eq!(
            pick_existing_instance(&rows, None, "Missing").as_deref(),
            None
        );
        assert_eq!(
            pick_existing_instance(&[json!({"id": "only", "name": "X"})], None, "Y").as_deref(),
            Some("only")
        );
    }

    #[test]
    fn publish_dir_paths_honours_manifest_and_instance_env() {
        let dir = tempdir().unwrap();
        temp_env::with_vars(
            [
                ("MANIFEST_FILE_NAME", Some("custom.yml")),
                ("INSTANCE_FILE_NAME", Some("inst.yml")),
            ],
            || {
                let (manifest, instance, _) = publish_dir_paths(dir.path()).unwrap();
                assert_eq!(
                    manifest.file_name().and_then(|n| n.to_str()),
                    Some("custom.yml")
                );
                assert_eq!(
                    instance.file_name().and_then(|n| n.to_str()),
                    Some("inst.yml")
                );
            },
        );
    }
}
