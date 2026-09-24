//! The widget compiler (milestone 46) — `render(page: Node) -> String`,
//! the only renderer `import ui` exposes. See
//! `docs/milestones/46-widgets/SPEC.md`.
//!
//! An AINT program builds a tree of abstract widgets (`Column`, `Row`,
//! `Box`, `Text`, `Button`, ...) with structured, unit-less style props
//! (`padding: 24`, not `padding: "24px"`) — no HTML tag name, CSS
//! property, selector, or class string appears anywhere in that tree.
//! `render` walks it once, compiling every widget's resolved style into
//! a deduplicated, generated CSS class (two widgets with identical
//! resolved styling anywhere in the tree share one class — the same
//! idea atomic-CSS/CSS-in-JS systems use internally, kept entirely
//! behind this one function) and produces one complete HTML document
//! string: doctype, head, the generated stylesheet, and body.

use crate::error::RuntimeError;
use crate::value::{PropValue, Value};
use aint_ast::Span;

/// The one fixed breakpoint `Responsive` compiles against — not an
/// author-chosen pixel value. See SPEC.md's "Responsive design".
const BREAKPOINT_PX: u32 = 720;

/// The fixed set of theme color fields a `Light`/`Dark` palette can
/// set, and the only strings a color-valued prop (`background`,
/// `color`, `border`) resolves as a theme reference rather than a
/// literal color. `on_accent` is the readable-on-`accent` color (a
/// button's label color, typically) — kept explicit rather than
/// computed, since contrast computation is its own can of worms.
/// `muted` (milestone 47) is a real secondary-text tone, distinct from
/// `border` — before this, a project's only way to get dimmer-than-
/// `text` color was to reuse the hairline-divider color for it too,
/// a real compromise `aint-website`'s own `layout.an` had to make and
/// document. `success`/`warning`/`danger` are status colors with no
/// consumer yet (a future Badge/alert widget, not this milestone) but
/// belong at the token layer regardless, the same way every other
/// color a widget might reference does.
const THEME_TOKENS: &[&str] = &[
    "accent",
    "background",
    "surface",
    "text",
    "muted",
    "border",
    "on_accent",
    "success",
    "warning",
    "danger",
];

#[derive(Clone)]
struct Palette {
    accent: String,
    background: String,
    surface: String,
    text: String,
    muted: String,
    border: String,
    on_accent: String,
    success: String,
    warning: String,
    danger: String,
}

impl Palette {
    fn light_default() -> Self {
        Palette {
            accent: "#5847d1".to_string(),
            background: "#faf9f7".to_string(),
            surface: "#ffffff".to_string(),
            text: "#1b1b1f".to_string(),
            muted: "#7c7568".to_string(),
            border: "#e6e2d8".to_string(),
            on_accent: "#ffffff".to_string(),
            success: "#1a7f4f".to_string(),
            warning: "#9a6400".to_string(),
            danger: "#c02b3c".to_string(),
        }
    }

    fn dark_default() -> Self {
        Palette {
            accent: "#9285ff".to_string(),
            background: "#0a0a0c".to_string(),
            surface: "#111113".to_string(),
            text: "#f2f1ec".to_string(),
            muted: "#9b968a".to_string(),
            border: "#1c1c1f".to_string(),
            on_accent: "#0a0a0c".to_string(),
            success: "#3ecf7e".to_string(),
            warning: "#facc15".to_string(),
            danger: "#f87171".to_string(),
        }
    }

    /// `(field name, value)` for every field, in a fixed order — the
    /// one place that order is decided; `Theme::css()` never hand-lists
    /// fields itself, so adding a field here is the only step a future
    /// milestone needs (plus the matching `set` arm below).
    fn as_pairs(&self) -> [(&'static str, &str); 10] {
        [
            ("accent", &self.accent),
            ("background", &self.background),
            ("surface", &self.surface),
            ("text", &self.text),
            ("muted", &self.muted),
            ("border", &self.border),
            ("on_accent", &self.on_accent),
            ("success", &self.success),
            ("warning", &self.warning),
            ("danger", &self.danger),
        ]
    }

    fn set(&mut self, field: &str, color: String) -> bool {
        match field {
            "accent" => self.accent = color,
            "background" => self.background = color,
            "surface" => self.surface = color,
            "text" => self.text = color,
            "muted" => self.muted = color,
            "border" => self.border = color,
            "on_accent" => self.on_accent = color,
            "success" => self.success = color,
            "warning" => self.warning = color,
            "danger" => self.danger = color,
            _ => return false,
        }
        true
    }
}

struct Theme {
    light: Palette,
    dark: Palette,
}

impl Theme {
    fn default_theme() -> Self {
        Theme {
            light: Palette::light_default(),
            dark: Palette::dark_default(),
        }
    }

    /// Both palettes as `:root` custom properties — the light palette
    /// as the default, the dark palette under `prefers-color-scheme` —
    /// generated once per `render()` call, not per widget. A widget's
    /// own `background: "accent"` compiles to `var(--accent)`, which is
    /// why the same generated class works correctly in both modes.
    fn css(&self) -> String {
        let declarations = |palette: &Palette| -> String {
            palette
                .as_pairs()
                .iter()
                .map(|(name, value)| format!("--{name}:{value};"))
                .collect()
        };
        format!(
            ":root{{{}}}@media (prefers-color-scheme:dark){{:root{{{}}}}}",
            declarations(&self.light),
            declarations(&self.dark),
        )
    }
}

/// Accumulates the generated stylesheet across one `render()` call.
/// `class_for` is the whole dedup story: two widgets whose *resolved*
/// declarations (and hover treatment, if any) are textually identical
/// share one class, keyed on that text — no hashing, no collision risk,
/// just a lookup.
struct Stylesheet {
    classes: std::collections::HashMap<String, String>,
    rules: Vec<String>,
    next_id: usize,
    used_responsive: bool,
}

impl Stylesheet {
    fn new() -> Self {
        Stylesheet {
            classes: std::collections::HashMap::new(),
            rules: Vec::new(),
            next_id: 1,
            used_responsive: false,
        }
    }

    fn class_for(&mut self, decls: Vec<(&'static str, String)>, hover: Option<&str>) -> String {
        if decls.is_empty() && hover.is_none() {
            return String::new();
        }
        let mut text = String::new();
        for (k, v) in &decls {
            text.push_str(k);
            text.push(':');
            text.push_str(v);
            text.push(';');
        }
        // A NUL separator can never appear in generated declaration
        // text, so this can't collide with a differently-styled widget
        // that happens to share a text prefix.
        let key = match hover {
            Some(h) => format!("{text}\u{0}{h}"),
            None => text.clone(),
        };
        if let Some(existing) = self.classes.get(&key) {
            return existing.clone();
        }
        let name = format!("w{}", self.next_id);
        self.next_id += 1;
        self.classes.insert(key, name.clone());
        let mut rule = format!(".{name}{{{text}}}");
        if let Some(h) = hover {
            rule.push_str(&format!(".{name}:hover{{{h}}}"));
        }
        self.rules.push(rule);
        name
    }

    fn finish(&self, theme_css: &str) -> String {
        let mut out = String::new();
        out.push_str("*{box-sizing:border-box;margin:0;padding:0}");
        out.push_str(
            "body{background:var(--background);color:var(--text);font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;line-height:1.5}",
        );
        out.push_str(theme_css);
        for rule in &self.rules {
            out.push_str(rule);
        }
        if self.used_responsive {
            out.push_str(&format!(
                "@media (max-width:{BREAKPOINT_PX}px){{.w-wide-only{{display:none}}}}"
            ));
            out.push_str(&format!(
                "@media (min-width:{}px){{.w-narrow-only{{display:none}}}}",
                BREAKPOINT_PX + 1
            ));
        }
        out
    }
}

/// `render(page: Node) -> String` — see `NativeFunction::Render`.
pub fn render(page: &Value, span: Span) -> Result<String, RuntimeError> {
    let Value::Node {
        role,
        props,
        children,
    } = page
    else {
        return Err(RuntimeError::TypeMismatch {
            message: format!("render expects a Page, found a {}", page.type_name()),
            span,
        });
    };
    if role != "Page" {
        return Err(RuntimeError::TypeMismatch {
            message: format!("render expects the root widget to be Page, found {role}"),
            span,
        });
    }

    let title = str_prop(props, "title").unwrap_or("");
    let description = str_prop(props, "description").unwrap_or("");

    let mut theme = Theme::default_theme();
    let mut body: Option<&Value> = None;
    for child in children {
        if let Value::Node {
            role: child_role,
            children: theme_children,
            ..
        } = child
        {
            if child_role == "Theme" {
                theme = parse_theme(theme_children, span)?;
                continue;
            }
        }
        if body.is_some() {
            return Err(RuntimeError::TypeMismatch {
                message: "Page can only have one body widget, plus an optional Theme".to_string(),
                span,
            });
        }
        body = Some(child);
    }
    let body = body.ok_or_else(|| RuntimeError::TypeMismatch {
        message: "Page needs a body widget".to_string(),
        span,
    })?;

    let mut sheet = Stylesheet::new();
    let body_html = render_widget(body, &mut sheet, span)?;
    let css = sheet.finish(&theme.css());

    Ok(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{}</title><meta name=\"description\" content=\"{}\">\
         <style>{css}</style></head><body>{body_html}</body></html>",
        escape_html(title),
        escape_html(description),
    ))
}

fn parse_theme(children: &[Value], span: Span) -> Result<Theme, RuntimeError> {
    let mut theme = Theme::default_theme();
    for child in children {
        let Value::Node {
            role: palette_role,
            props,
            ..
        } = child
        else {
            return Err(RuntimeError::TypeMismatch {
                message: "Theme children must be Light/Dark widgets".to_string(),
                span,
            });
        };
        let target = match palette_role.as_str() {
            "Light" => &mut theme.light,
            "Dark" => &mut theme.dark,
            other => {
                return Err(RuntimeError::TypeMismatch {
                    message: format!("Theme only accepts Light/Dark children, found {other}"),
                    span,
                });
            }
        };
        for (name, value) in props {
            let PropValue::Str(color) = value else {
                return Err(RuntimeError::TypeMismatch {
                    message: format!("{name} must be a String color"),
                    span,
                });
            };
            if !is_plausible_css_color(color) {
                return Err(RuntimeError::TypeMismatch {
                    message: format!("{name}: {color:?} isn't a recognized color"),
                    span,
                });
            }
            if !target.set(name, color.clone()) {
                return Err(RuntimeError::TypeMismatch {
                    message: format!("unknown palette field `{name}`"),
                    span,
                });
            }
        }
    }
    Ok(theme)
}

fn render_widget(
    value: &Value,
    sheet: &mut Stylesheet,
    span: Span,
) -> Result<String, RuntimeError> {
    match value {
        Value::String(text) => Ok(escape_html(text)),
        Value::Node { role, children, .. } if role == "Responsive" => {
            render_responsive(children, sheet, span)
        }
        Value::Node {
            role,
            props,
            children,
        } => {
            let mut rendered_children = Vec::with_capacity(children.len());
            for child in children {
                rendered_children.push(render_widget(child, sheet, span)?);
            }
            let inner = rendered_children.concat();
            match role.as_str() {
                "Column" => Ok(wrap_div(sheet, flex_decls("column", props), None, &inner)),
                "Row" => Ok(wrap_div(sheet, flex_decls("row", props), None, &inner)),
                "Box" => Ok(wrap_div(sheet, box_decls(props), None, &inner)),
                "Spacer" => {
                    let class = sheet.class_for(vec![("flex", "1 1 auto".to_string())], None);
                    Ok(format!("<div class=\"{class}\"></div>"))
                }
                "Text" => Ok(wrap_tag("p", sheet, text_decls(props), None, &inner)),
                "Code" => Ok(wrap_tag("pre", sheet, code_decls(props), None, &inner)),
                "Heading" => Ok(render_heading(props, sheet, &inner)),
                "Button" => Ok(wrap_tag(
                    "button",
                    sheet,
                    button_decls(props),
                    Some("opacity:.88".to_string()),
                    &inner,
                )),
                "Link" => Ok(render_link(props, sheet, &inner)),
                "Image" => Ok(render_image(props)),
                "Input" => render_input(props, sheet, span),
                "Label" => Ok(render_label(props, &inner)),
                "List" => Ok(render_list(&rendered_children, sheet)),
                "Form" => Ok(render_form(props, &inner)),
                other => Err(RuntimeError::TypeMismatch {
                    message: format!("`{other}` isn't a widget `render` understands"),
                    span,
                }),
            }
        }
        other => Err(RuntimeError::TypeMismatch {
            message: format!(
                "expected a widget Node or String, found a {}",
                other.type_name()
            ),
            span,
        }),
    }
}

/// `Responsive { Narrow { ... } Wide { ... } }` renders *both* children
/// unconditionally and lets two fixed, globally-shared utility classes
/// (`w-narrow-only`/`w-wide-only`) show exactly one via the one
/// breakpoint's `@media` query — see `Stylesheet::finish`.
fn render_responsive(
    children: &[Value],
    sheet: &mut Stylesheet,
    span: Span,
) -> Result<String, RuntimeError> {
    sheet.used_responsive = true;
    let mut narrow_html = String::new();
    let mut wide_html = String::new();
    for child in children {
        let Value::Node {
            role: child_role,
            children: inner_children,
            ..
        } = child
        else {
            return Err(RuntimeError::TypeMismatch {
                message: "Responsive children must be Narrow/Wide widgets".to_string(),
                span,
            });
        };
        let mut rendered = String::new();
        for inner in inner_children {
            rendered.push_str(&render_widget(inner, sheet, span)?);
        }
        match child_role.as_str() {
            "Narrow" => narrow_html = rendered,
            "Wide" => wide_html = rendered,
            other => {
                return Err(RuntimeError::TypeMismatch {
                    message: format!("Responsive only accepts Narrow/Wide children, found {other}"),
                    span,
                });
            }
        }
    }
    Ok(format!(
        "<div class=\"w-narrow-only\">{narrow_html}</div><div class=\"w-wide-only\">{wide_html}</div>"
    ))
}

fn wrap_div(
    sheet: &mut Stylesheet,
    decls: Vec<(&'static str, String)>,
    hover: Option<String>,
    inner: &str,
) -> String {
    let class = sheet.class_for(decls, hover.as_deref());
    if class.is_empty() {
        format!("<div>{inner}</div>")
    } else {
        format!("<div class=\"{class}\">{inner}</div>")
    }
}

fn wrap_tag(
    tag: &str,
    sheet: &mut Stylesheet,
    decls: Vec<(&'static str, String)>,
    hover: Option<String>,
    inner: &str,
) -> String {
    let class = sheet.class_for(decls, hover.as_deref());
    if class.is_empty() {
        format!("<{tag}>{inner}</{tag}>")
    } else {
        format!("<{tag} class=\"{class}\">{inner}</{tag}>")
    }
}

fn flex_decls(
    direction: &'static str,
    props: &[(String, PropValue)],
) -> Vec<(&'static str, String)> {
    let mut decls = vec![
        ("display", "flex".to_string()),
        ("flex-direction", direction.to_string()),
    ];
    if let Some(gap) = px(props, "gap") {
        decls.push(("gap", gap));
    }
    if let Some(align) = str_prop(props, "align").and_then(align_items_value) {
        decls.push(("align-items", align.to_string()));
    }
    if direction == "row" && flag(props, "wrap") {
        decls.push(("flex-wrap", "wrap".to_string()));
    }
    decls
}

fn box_decls(props: &[(String, PropValue)]) -> Vec<(&'static str, String)> {
    let mut decls = Vec::new();
    if let Some(p) = px(props, "padding") {
        decls.push(("padding", p));
    }
    if let Some(bg) = str_prop(props, "background").and_then(resolve_color) {
        decls.push(("background", bg));
    }
    if let Some(r) = px(props, "corner_radius") {
        decls.push(("border-radius", r));
    }
    if let Some(c) = str_prop(props, "border").and_then(resolve_color) {
        decls.push(("border", format!("1px solid {c}")));
    }
    if let Some(w) = px(props, "width") {
        decls.push(("width", w));
    }
    if let Some(h) = px(props, "height") {
        decls.push(("height", h));
    }
    if flag(props, "grow") {
        decls.push(("flex", "1 1 auto".to_string()));
    }
    decls
}

fn text_decls(props: &[(String, PropValue)]) -> Vec<(&'static str, String)> {
    let mut decls = Vec::new();
    if let Some(s) = px(props, "size") {
        decls.push(("font-size", s));
    }
    if let Some(w) = str_prop(props, "weight") {
        let mapped = match w {
            "medium" => "500",
            "bold" => "700",
            _ => "400",
        };
        decls.push(("font-weight", mapped.to_string()));
    }
    if let Some(c) = str_prop(props, "color").and_then(resolve_color) {
        decls.push(("color", c));
    }
    decls
}

/// `Code { "multi\nline text" }` → `<pre>` — the one widget whose
/// formatting (monospace, preserved whitespace/line breaks,
/// horizontal scroll instead of wrapping mid-token) is fixed and
/// built in rather than left to style props, the same way `Button`'s
/// unstyled look is a default, not a CSS property an author names. A
/// real UI concept (preformatted/code text), not a CSS one — no
/// `white-space`/`font-family` value ever appears in an AINT program.
fn code_decls(props: &[(String, PropValue)]) -> Vec<(&'static str, String)> {
    let mut decls = vec![
        (
            "font-family",
            "'JetBrains Mono','Courier New',monospace".to_string(),
        ),
        ("white-space", "pre-wrap".to_string()),
        ("overflow-x", "auto".to_string()),
    ];
    if let Some(p) = px(props, "padding") {
        decls.push(("padding", p));
    }
    if let Some(bg) = str_prop(props, "background").and_then(resolve_color) {
        decls.push(("background", bg));
    }
    if let Some(r) = px(props, "corner_radius") {
        decls.push(("border-radius", r));
    }
    if let Some(s) = px(props, "size") {
        decls.push(("font-size", s));
    }
    decls
}

fn render_heading(props: &[(String, PropValue)], sheet: &mut Stylesheet, inner: &str) -> String {
    let level = num_prop(props, "level")
        .unwrap_or(2.0)
        .round()
        .clamp(1.0, 6.0) as i64;
    let tag = format!("h{level}");
    let default_px: f64 = match level {
        1 => 40.0,
        2 => 32.0,
        3 => 24.0,
        4 => 20.0,
        5 => 18.0,
        _ => 16.0,
    };
    let size = num_prop(props, "size").unwrap_or(default_px);
    let mut decls: Vec<(&'static str, String)> = vec![
        ("font-size", format!("{}px", fmt_num(size))),
        ("font-weight", "700".to_string()),
    ];
    if let Some(c) = str_prop(props, "color").and_then(resolve_color) {
        decls.push(("color", c));
    }
    wrap_tag(&tag, sheet, decls, None, inner)
}

fn button_decls(props: &[(String, PropValue)]) -> Vec<(&'static str, String)> {
    let padding = px(props, "padding").unwrap_or_else(|| "12px".to_string());
    let background = str_prop(props, "background")
        .and_then(resolve_color)
        .unwrap_or_else(|| "var(--accent)".to_string());
    let color = str_prop(props, "color")
        .and_then(resolve_color)
        .unwrap_or_else(|| "var(--on_accent)".to_string());
    let radius = px(props, "corner_radius").unwrap_or_else(|| "8px".to_string());
    vec![
        ("padding", padding),
        ("background", background),
        ("color", color),
        ("border-radius", radius),
        ("border", "none".to_string()),
        ("cursor", "pointer".to_string()),
        ("font", "inherit".to_string()),
        ("font-weight", "600".to_string()),
        // A no-op on a real `<button>` (never underlined by default),
        // but load-bearing for `Link`: once `padding`/`background` make
        // it look like a button, it's still a real `<a>` underneath,
        // and a bare `<a>` is underlined by the browser by default.
        ("text-decoration", "none".to_string()),
    ]
}

/// `Link { to: "..." "..." }` navigates. With no `padding`/`background`
/// it's a plain inline link (accent-colored, underlines on hover); once
/// either is set it gets the same treatment `Button` does — a widget
/// that both navigates and looks like a call-to-action button is just a
/// `Link` with those props set, not a separate mode/variant.
fn render_link(props: &[(String, PropValue)], sheet: &mut Stylesheet, inner: &str) -> String {
    let to = str_prop(props, "to").unwrap_or("");
    let href = safe_url(to).map(escape_html).unwrap_or_default();
    let looks_like_a_button =
        str_prop(props, "background").is_some() || num_prop(props, "padding").is_some();
    let (decls, hover) = if looks_like_a_button {
        (button_decls(props), "opacity:.88".to_string())
    } else {
        let color = str_prop(props, "color")
            .and_then(resolve_color)
            .unwrap_or_else(|| "var(--accent)".to_string());
        (
            vec![("color", color), ("text-decoration", "none".to_string())],
            "text-decoration:underline".to_string(),
        )
    };
    let class = sheet.class_for(decls, Some(&hover));
    if class.is_empty() {
        format!("<a href=\"{href}\">{inner}</a>")
    } else {
        format!("<a class=\"{class}\" href=\"{href}\">{inner}</a>")
    }
}

fn render_image(props: &[(String, PropValue)]) -> String {
    let src = str_prop(props, "src").unwrap_or("");
    let src = safe_url(src).map(escape_html).unwrap_or_default();
    let alt = str_prop(props, "alt").map(escape_html).unwrap_or_default();
    let mut attrs = String::new();
    if let Some(w) = num_prop(props, "width") {
        attrs.push_str(&format!(" width=\"{}\"", fmt_num(w)));
    }
    if let Some(h) = num_prop(props, "height") {
        attrs.push_str(&format!(" height=\"{}\"", fmt_num(h)));
    }
    format!("<img src=\"{src}\" alt=\"{alt}\"{attrs}>")
}

const INPUT_KINDS: &[&str] = &[
    "text", "checkbox", "email", "password", "search", "tel", "url", "number",
];

/// A text-shaped `Input`'s own box styling - the same style props
/// `Box` takes, with sensible built-in defaults so an unstyled `Input`
/// still looks like a real text field. A `checkbox` gets none of this:
/// padding/border on a native checkbox control looks broken, and its
/// own appearance is meant to stay whatever the browser gives it.
fn input_decls(kind: &str, props: &[(String, PropValue)]) -> Vec<(&'static str, String)> {
    if kind == "checkbox" {
        return Vec::new();
    }
    let padding = px(props, "padding").unwrap_or_else(|| "10px".to_string());
    let border = str_prop(props, "border")
        .and_then(resolve_color)
        .unwrap_or_else(|| "var(--border)".to_string());
    let radius = px(props, "corner_radius").unwrap_or_else(|| "6px".to_string());
    vec![
        ("padding", padding),
        ("border", format!("1px solid {border}")),
        ("border-radius", radius),
        ("font", "inherit".to_string()),
    ]
}

fn render_input(
    props: &[(String, PropValue)],
    sheet: &mut Stylesheet,
    span: Span,
) -> Result<String, RuntimeError> {
    let kind = str_prop(props, "kind").unwrap_or("text");
    if !INPUT_KINDS.contains(&kind) {
        return Err(RuntimeError::TypeMismatch {
            message: format!("Input kind {kind:?} isn't recognized"),
            span,
        });
    }
    let mut attrs = String::new();
    if let Some(id) = str_prop(props, "id") {
        attrs.push_str(&format!(" id=\"{}\"", escape_html(id)));
    }
    if let Some(name) = str_prop(props, "name") {
        attrs.push_str(&format!(" name=\"{}\"", escape_html(name)));
    }
    if let Some(placeholder) = str_prop(props, "placeholder") {
        attrs.push_str(&format!(" placeholder=\"{}\"", escape_html(placeholder)));
    }
    if let Some(value) = str_prop(props, "value") {
        attrs.push_str(&format!(" value=\"{}\"", escape_html(value)));
    }
    if flag(props, "required") {
        attrs.push_str(" required");
    }
    let class = sheet.class_for(input_decls(kind, props), None);
    let class_attr = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class}\"")
    };
    Ok(format!("<input type=\"{kind}\"{class_attr}{attrs}>"))
}

fn render_label(props: &[(String, PropValue)], inner: &str) -> String {
    let target = str_prop(props, "target")
        .map(escape_html)
        .unwrap_or_default();
    let for_attr = if target.is_empty() {
        String::new()
    } else {
        format!(" for=\"{target}\"")
    };
    format!("<label{for_attr}>{inner}</label>")
}

/// `Form { action: "..." method: "get" children }` - a plain HTML form
/// submission (a full-page navigation carrying its inputs as a query
/// string or a POST body), not client-side interactivity; still
/// squarely in scope for a server-rendered page. `method` defaults to,
/// and is clamped to, `"get"`/`"post"` - the only two an HTML `<form>`
/// itself supports.
fn render_form(props: &[(String, PropValue)], inner: &str) -> String {
    let method = if str_prop(props, "method") == Some("post") {
        "post"
    } else {
        "get"
    };
    let action = str_prop(props, "action")
        .map(escape_html)
        .unwrap_or_default();
    format!("<form method=\"{method}\" action=\"{action}\">{inner}</form>")
}

fn render_list(rendered_children: &[String], sheet: &mut Stylesheet) -> String {
    let class = sheet.class_for(vec![("margin-left", "20px".to_string())], None);
    let items: String = rendered_children
        .iter()
        .map(|child| format!("<li>{child}</li>"))
        .collect();
    format!("<ul class=\"{class}\">{items}</ul>")
}

fn align_items_value(raw: &str) -> Option<&'static str> {
    match raw {
        "start" => Some("flex-start"),
        "center" => Some("center"),
        "end" => Some("flex-end"),
        "stretch" => Some("stretch"),
        _ => None,
    }
}

/// A color-valued prop resolves against the active theme by name
/// (`"accent"` -> `var(--accent)`, so the same generated class stays
/// correct across a light/dark switch) or, failing that, as a literal
/// CSS color — validated by `is_plausible_css_color` first, so a
/// malformed or adversarial string (from an `infer -> Node` response,
/// same threat model as milestone 44/45's HTML renderer had to
/// consider) can never break out of the generated CSS declaration it's
/// spliced into. An unrecognized value is dropped, not an error — the
/// same "degrade safely, don't fail the whole render" stance the old
/// renderer took on a bad prop.
fn resolve_color(raw: &str) -> Option<String> {
    if THEME_TOKENS.contains(&raw) {
        return Some(format!("var(--{raw})"));
    }
    if is_plausible_css_color(raw) {
        return Some(raw.to_string());
    }
    None
}

fn is_plausible_css_color(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if let Some(hex) = s.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let lower = s.to_ascii_lowercase();
    for prefix in ["rgb(", "rgba(", "hsl(", "hsla("] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            if let Some(inner) = rest.strip_suffix(')') {
                return inner
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ' ' | '%' | '-'));
            }
        }
    }
    matches!(s, "transparent" | "currentcolor" | "white" | "black")
}

/// The other half of the safety story `resolve_color` covers for CSS: a
/// `javascript:` URL is the classic way an attribute *value* becomes
/// code execution, checked wherever `to`/`src` is used. `None` — the
/// attribute is omitted, not emitted empty. `data:` is deliberately left
/// alone, same reasoning as milestone 44/45's `safe_url`.
fn safe_url(value: &str) -> Option<&str> {
    if value.trim().to_ascii_lowercase().starts_with("javascript:") {
        None
    } else {
        Some(value)
    }
}

fn escape_html(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn str_prop<'a>(props: &'a [(String, PropValue)], name: &str) -> Option<&'a str> {
    props
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.as_str())
}

fn num_prop(props: &[(String, PropValue)], name: &str) -> Option<f64> {
    props
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.as_num())
}

fn flag(props: &[(String, PropValue)], name: &str) -> bool {
    props
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.as_bool())
        .unwrap_or(false)
}

fn px(props: &[(String, PropValue)], name: &str) -> Option<String> {
    num_prop(props, name).map(|n| format!("{}px", fmt_num(n)))
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}
