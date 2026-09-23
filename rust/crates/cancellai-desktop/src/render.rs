//! HTML for the dashboard. One page, no script, every engine-supplied string escaped.
//!
//! The page has one form, and it is a `GET` that only changes the query - the same read-only
//! flags the CLI's `status`/`plan` accept. There is no control that cleans, configures or
//! executes: the page says so, and names the terminal command that does.

use cancellai_desktop_api::ToolScope;

use crate::viewmodel::DashboardView;

/// Escapes text for an HTML text node or a double-quoted attribute.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

const STYLE: &str = "body{font:14px system-ui,sans-serif;margin:2rem;max-width:72rem;color:#1d1d1f;background:#fbfbfd}\
h1{font-size:1.4rem}h2{font-size:1.1rem;margin-top:2rem}table{border-collapse:collapse;width:100%}\
th,td{text-align:left;padding:.35rem .6rem;border-bottom:1px solid #ddd;vertical-align:top}\
.warn{background:#fff4e5;border:1px solid #f0b060;padding:.6rem;margin:.8rem 0}\
.note{color:#555}code{background:#eee;padding:0 .25rem}form{margin:1rem 0}\
@media (prefers-color-scheme:dark){body{background:#1c1c1e;color:#eee}th,td{border-color:#444}\
.warn{background:#3a2a10;border-color:#8a6020}.note{color:#aaa}code{background:#333}}";

fn tool_option(current: ToolScope, value: ToolScope, label: &str) -> String {
    let selected = if current == value { " selected" } else { "" };
    format!("<option value=\"{label}\"{selected}>{label}</option>")
}

/// Renders the whole page. `action` is the path the query form submits to (the dashboard's own
/// tokenised path).
pub fn page(view: &DashboardView, action: &str) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<title>cancellAI dashboard</title><style>");
    html.push_str(STYLE);
    html.push_str("</style></head><body>");
    html.push_str("<h1>cancellAI - read-only dashboard</h1>");
    html.push_str(&format!(
        "<p class=\"note\">Engine {} via the desktop API. This page never cleans anything; to act \
         on a plan, run <code>cancellai-cli clean</code> in a terminal.</p>",
        escape(&view.engine_version)
    ));

    let q = view.query;
    html.push_str(&format!(
        "<form method=\"get\" action=\"{}\">Older than <input name=\"days\" type=\"number\" \
         min=\"0\" value=\"{}\"> days, keep latest <input name=\"keep_latest\" type=\"number\" \
         min=\"0\" value=\"{}\">, tool <select name=\"tool\">{}{}{}</select> \
         <label><input name=\"allow_running\" type=\"checkbox\" value=\"true\"{}> preview even \
         while a provider is running</label> <button type=\"submit\">Refresh</button></form>",
        escape(action),
        q.days,
        q.keep_latest,
        tool_option(q.tool, ToolScope::All, "all"),
        tool_option(q.tool, ToolScope::Claude, "claude"),
        tool_option(q.tool, ToolScope::Codex, "codex"),
        if q.allow_running { " checked" } else { "" },
    ));

    if view.scan_incomplete {
        html.push_str(
            "<div class=\"warn\">One or more provider scans were incomplete. Nothing about \
             missing data was assumed; unobserved data is never a cleanup candidate.</div>",
        );
    }
    if !view.plan.withheld_by_root_authority.is_empty() {
        html.push_str(&format!(
            "<div class=\"warn\">Destructive work was withheld for: {} (not the default root).</div>",
            escape(&view.plan.withheld_by_root_authority.join(", "))
        ));
    }

    html.push_str(
        "<h2>Providers</h2><table><tr><th>Provider</th><th>Artifacts</th><th>Bytes</th>\
                   <th>Scan complete</th><th>Root</th><th>Mutation eligible</th></tr>",
    );
    for row in &view.providers {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&row.provider_id),
            row.artifacts,
            row.bytes,
            if row.scan_complete { "yes" } else { "no" },
            escape(&row.root_origin),
            if row.mutation_eligible { "yes" } else { "no" },
        ));
    }
    html.push_str("</table>");

    let plan = &view.plan;
    html.push_str(&format!(
        "<h2>Plan preview</h2><p>{} action(s) proposed: {} delete candidate(s), {} \
         observation(s).</p>",
        plan.total_actions, plan.delete_candidates, plan.observations
    ));
    if !plan.actions.is_empty() {
        html.push_str("<table><tr><th>Action</th><th>Targets</th><th>Reason</th></tr>");
        for action in &plan.actions {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&action.action_class),
                escape(&action.targets.join(", ")),
                escape(&action.reason),
            ));
        }
        html.push_str("</table>");
    }
    html.push_str("</body></html>");
    html
}

/// A minimal error page. `message` is escaped.
pub fn error_page(message: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>cancellAI \
         dashboard</title><style>{STYLE}</style></head><body><h1>cancellAI - read-only \
         dashboard</h1><div class=\"warn\">{}</div></body></html>",
        escape(message)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewmodel::{ActionRow, PlanView, ProviderRow};
    use cancellai_desktop_api::Query;

    fn view(reason: &str) -> DashboardView {
        DashboardView {
            engine_version: "0.1.0".to_string(),
            query: Query::default(),
            providers: vec![ProviderRow {
                provider_id: "claude-code".to_string(),
                artifacts: 2,
                bytes: 10,
                scan_complete: true,
                root_origin: "default".to_string(),
                mutation_eligible: true,
            }],
            scan_incomplete: false,
            plan: PlanView {
                total_actions: 1,
                delete_candidates: 1,
                observations: 0,
                withheld_by_root_authority: Vec::new(),
                actions: vec![ActionRow {
                    action_class: "delete".to_string(),
                    targets: vec!["artifact-1".to_string()],
                    reason: reason.to_string(),
                }],
            },
        }
    }

    #[test]
    fn engine_text_cannot_inject_markup() {
        let html = page(
            &view("<script>alert(1)</script><img src=x onerror=\"y\">'"),
            "/t",
        );
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;y&quot;"));
        assert!(html.contains("&#39;"));
    }

    #[test]
    fn the_page_has_no_script_and_only_a_get_form() {
        let html = page(&view("stale"), "/token");
        assert!(!html.to_ascii_lowercase().contains("<script"));
        assert_eq!(html.matches("<form").count(), 1);
        assert!(html.contains("method=\"get\""));
        assert!(!html.to_ascii_lowercase().contains("method=\"post\""));
        assert!(html.contains("never cleans anything"));
        assert!(html.contains("1 action(s) proposed: 1 delete candidate(s), 0 observation(s)."));
    }

    #[test]
    fn warnings_render_when_the_engine_reports_them() {
        let mut v = view("stale");
        assert!(!page(&v, "/t").contains("class=\"warn\""));
        v.scan_incomplete = true;
        v.plan.withheld_by_root_authority = vec!["claude-code".to_string()];
        let html = page(&v, "/t");
        assert!(html.contains("scans were incomplete"));
        assert!(html.contains("withheld for: claude-code"));
    }

    #[test]
    fn the_form_reflects_the_query() {
        let mut v = view("stale");
        v.query = Query {
            days: 30,
            keep_latest: 5,
            tool: ToolScope::Codex,
            allow_running: true,
        };
        v.plan.actions.clear();
        let html = page(&v, "/t");
        assert!(html.contains("value=\"30\""));
        assert!(html.contains("value=\"5\""));
        assert!(html.contains("<option value=\"codex\" selected>"));
        assert!(html.contains("value=\"true\" checked"));
        assert!(!html.contains("<th>Reason</th>"));
    }

    #[test]
    fn the_error_page_escapes_its_message() {
        let html = error_page("<b>engine down</b>");
        assert!(html.contains("&lt;b&gt;engine down&lt;/b&gt;"));
    }
}
