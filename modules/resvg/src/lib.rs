//! Rust-native first-stage compatibility module for `@resvg/resvg-js`.

use jjs_module_api::{
    FontFaceId, FontRequest, FontStyle, ModuleCallResult, ModuleContext, ModuleContinuation,
    ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest, ModuleObjectKind,
    ModuleValueKind, NativeModule, ValueHandle, FONT_REGISTRY_CONTRACT_V1, MODULE_API_VERSION,
};
use std::collections::BTreeMap;

const RESVG: ModuleFunctionKey = ModuleFunctionKey(1);
const RENDER: ModuleFunctionKey = ModuleFunctionKey(2);
const AS_PNG: ModuleFunctionKey = ModuleFunctionKey(3);
const RESVG_INSTANCE: ModuleObjectKind = ModuleObjectKind(1);
const RENDERED_IMAGE: ModuleObjectKind = ModuleObjectKind(2);

const SVG_SOURCE: u32 = 1;
const PNG_BYTES: u32 = 2;
const DEFAULT_FONT_FAMILY: u32 = 3;
const SANS_SERIF_FAMILY: u32 = 4;

const MAX_SVG_BYTES: usize = 1_048_576;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: u64 = 16_777_216;

pub struct ResvgModule {
    manifest: ModuleManifest,
}

impl Default for ResvgModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.resvg".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-resvg-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["@resvg/resvg-js".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: vec![RESVG.0, RENDER.0, AS_PNG.0],
                object_kind_keys: vec![RESVG_INSTANCE.0, RENDERED_IMAGE.0],
                deterministic_resources: vec![FONT_REGISTRY_CONTRACT_V1.into()],
            },
        }
    }
}

fn throw(name: &str, message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: name.into(),
        message: message.into(),
    }
}

fn attach_function(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    key: ModuleFunctionKey,
) -> Result<(), ModuleError> {
    let function = context.function(key)?;
    context.set_property(object, name, function)
}

fn validate_known_properties(
    context: &dyn ModuleContext,
    value: ValueHandle,
    allowed: &[&str],
) -> Result<(), String> {
    let names = context
        .own_property_names(value)
        .map_err(|error| error.to_string())?;
    if let Some(name) = names.iter().find(|name| !allowed.contains(&name.as_str())) {
        return Err(format!("resvg_option_unsupported:{name}"));
    }
    Ok(())
}

#[derive(Clone)]
struct FontOptions {
    default_family: String,
    sans_serif_family: String,
}

impl Default for FontOptions {
    fn default() -> Self {
        Self {
            default_family: "Times New Roman".into(),
            sans_serif_family: "Arial".into(),
        }
    }
}

fn validate_snappy_options(
    context: &mut dyn ModuleContext,
    options: ValueHandle,
) -> Result<FontOptions, String> {
    if context
        .value_kind(options)
        .map_err(|error| error.to_string())?
        != ModuleValueKind::Object
    {
        return Err("resvg_options_must_be_an_object".into());
    }
    validate_known_properties(context, options, &["fitTo", "font", "background"])?;

    let background = context
        .get_property(options, "background")
        .map_err(|error| error.to_string())?;
    if context
        .value_kind(background)
        .map_err(|error| error.to_string())?
        != ModuleValueKind::Undefined
    {
        return Err("resvg_background_unsupported".into());
    }

    let fit_to = context
        .get_property(options, "fitTo")
        .map_err(|error| error.to_string())?;
    if context
        .value_kind(fit_to)
        .map_err(|error| error.to_string())?
        != ModuleValueKind::Object
    {
        return Err("resvg_fit_to_original_required".into());
    }
    validate_known_properties(context, fit_to, &["mode", "value"])?;
    let mode = context
        .get_property(fit_to, "mode")
        .and_then(|value| context.as_string(value))
        .map_err(|_| "resvg_fit_to_original_required".to_string())?;
    if mode != "original" {
        return Err("resvg_fit_to_original_required".into());
    }
    let fit_value = context
        .get_property(fit_to, "value")
        .map_err(|error| error.to_string())?;
    if context
        .value_kind(fit_value)
        .map_err(|error| error.to_string())?
        != ModuleValueKind::Undefined
    {
        return Err("resvg_fit_to_original_value_unsupported".into());
    }

    let font = context
        .get_property(options, "font")
        .map_err(|error| error.to_string())?;
    if context
        .value_kind(font)
        .map_err(|error| error.to_string())?
        != ModuleValueKind::Object
    {
        return Err("resvg_font_options_required".into());
    }
    validate_known_properties(
        context,
        font,
        &[
            "loadSystemFonts",
            "defaultFontFamily",
            "sansSerifFamily",
            "fontFiles",
            "fontDirs",
        ],
    )?;
    let load_system_fonts = context
        .get_property(font, "loadSystemFonts")
        .and_then(|value| context.as_bool(value))
        .map_err(|_| "resvg_load_system_fonts_must_be_false".to_string())?;
    if load_system_fonts {
        return Err("resvg_load_system_fonts_unsupported".into());
    }
    let default_family = context
        .get_property(font, "defaultFontFamily")
        .and_then(|value| context.as_string(value))
        .map_err(|_| "resvg_defaultFontFamily_must_be_a_string".to_string())?;
    let sans_serif_family = context
        .get_property(font, "sansSerifFamily")
        .and_then(|value| context.as_string(value))
        .map_err(|_| "resvg_sansSerifFamily_must_be_a_string".to_string())?;
    if default_family.is_empty() || sans_serif_family.is_empty() {
        return Err("resvg_font_family_must_not_be_empty".into());
    }
    for name in ["fontFiles", "fontDirs"] {
        let value = context
            .get_property(font, name)
            .map_err(|error| error.to_string())?;
        if context
            .value_kind(value)
            .map_err(|error| error.to_string())?
            != ModuleValueKind::Undefined
        {
            return Err(format!("resvg_{name}_unsupported"));
        }
    }
    Ok(FontOptions {
        default_family,
        sans_serif_family,
    })
}

fn inherited_attribute<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<&'a str> {
    node.ancestors()
        .find_map(|ancestor| ancestor.attribute(name))
}

fn parse_families(value: &str, fonts: &FontOptions) -> Result<Vec<String>, String> {
    let families: Vec<String> = value
        .split(',')
        .map(str::trim)
        .map(|family| family.trim_matches(|c| c == '\'' || c == '"'))
        .map(|family| match family {
            "sans-serif" => fonts.sans_serif_family.as_str(),
            other => other,
        })
        .map(str::to_owned)
        .collect();
    if families.is_empty() || families.iter().any(String::is_empty) {
        return Err("resvg_font_family_invalid".into());
    }
    Ok(families)
}

fn parse_weight(value: Option<&str>) -> Result<u16, String> {
    match value.unwrap_or("normal").trim() {
        "normal" => Ok(400),
        "bold" => Ok(700),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| format!("resvg_font_weight_unsupported:{value}")),
    }
}

fn parse_style(value: Option<&str>) -> Result<FontStyle, String> {
    match value.unwrap_or("normal").trim() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        value => Err(format!("resvg_font_style_unsupported:{value}")),
    }
}

fn collect_registry_fonts(
    svg: &str,
    fonts: &FontOptions,
    context: &dyn ModuleContext,
) -> Result<BTreeMap<FontFaceId, Vec<u8>>, String> {
    let document =
        roxmltree::Document::parse(svg).map_err(|error| format!("resvg_invalid_svg: {error}"))?;
    let has_text = document.descendants().any(|node| {
        node.is_element() && matches!(node.tag_name().name(), "text" | "tspan" | "textPath")
    });
    if !has_text {
        return Ok(BTreeMap::new());
    }
    if document.descendants().any(|node| {
        (node.is_element() && node.tag_name().name() == "style")
            || (node.is_element() && node.attribute("style").is_some())
    }) {
        return Err("resvg_text_css_unsupported".into());
    }

    let mut resolved = BTreeMap::new();
    for node in document.descendants().filter(|node| {
        node.is_element() && matches!(node.tag_name().name(), "text" | "tspan" | "textPath")
    }) {
        let direct_text: String = node.children().filter_map(|child| child.text()).collect();
        if direct_text.is_empty() {
            continue;
        }
        let family_value =
            inherited_attribute(node, "font-family").unwrap_or(fonts.default_family.as_str());
        let families = parse_families(family_value, fonts)?;
        let weight = parse_weight(inherited_attribute(node, "font-weight"))?;
        let style = parse_style(inherited_attribute(node, "font-style"))?;
        if let Some(stretch) = inherited_attribute(node, "font-stretch") {
            if stretch.trim() != "normal" {
                return Err(format!("resvg_font_stretch_unsupported:{stretch}"));
            }
        }
        let face_id = context
            .resolve_font(&FontRequest {
                families,
                weight,
                width: 5,
                style,
                required_text: None,
            })
            .map_err(|error| error.to_string())?;
        let bytes = context
            .font_bytes(&face_id)
            .map_err(|error| error.to_string())?;
        let face = ttf_parser::Face::parse(&bytes, 0)
            .map_err(|_| "resvg_registry_font_invalid".to_string())?;
        for character in direct_text
            .chars()
            .filter(|character| !character.is_control())
        {
            if face.glyph_index(character).is_none() {
                return Err(format!(
                    "resvg_font_glyph_unavailable:U+{:04X}",
                    u32::from(character)
                ));
            }
        }
        resolved.entry(face_id).or_insert_with(|| bytes.to_vec());
    }
    Ok(resolved)
}

fn render_png(
    svg: &str,
    fonts: &FontOptions,
    context: &dyn ModuleContext,
) -> Result<(Vec<u8>, u32, u32), String> {
    if svg.len() > MAX_SVG_BYTES {
        return Err("resvg_svg_source_too_large".into());
    }

    let registry_fonts = collect_registry_fonts(svg, fonts, context)?;
    let mut options = resvg::usvg::Options::default();
    options.font_family = fonts.default_family.clone();
    options
        .fontdb_mut()
        .set_sans_serif_family(fonts.sans_serif_family.clone());
    for bytes in registry_fonts.into_values() {
        options.fontdb_mut().load_font_data(bytes);
    }
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|error| format!("resvg_invalid_svg: {error}"))?;
    let size = tree.size().to_int_size();
    let width = size.width();
    let height = size.height();
    let pixels = u64::from(width) * u64::from(height);
    if width > MAX_DIMENSION || height > MAX_DIMENSION || pixels > MAX_PIXELS {
        return Err("resvg_output_dimensions_too_large".into());
    }

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "resvg_pixmap_allocation_failed".to_string())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    let png = pixmap
        .encode_png()
        .map_err(|error| format!("resvg_png_encode_failed: {error}"))?;
    Ok((png, width, height))
}

impl NativeModule for ResvgModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        attach_function(context, exports, "Resvg", RESVG)?;
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        _callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key {
            RESVG => {
                let Some(svg) = args.first().copied() else {
                    return Ok(throw("TypeError", "Resvg requires an SVG string"));
                };
                if context.value_kind(svg)? != ModuleValueKind::String {
                    return Ok(throw("TypeError", "Resvg requires an SVG string"));
                }
                let mut font_options = FontOptions::default();
                if let Some(options) = args.get(1).copied() {
                    match context.value_kind(options)? {
                        ModuleValueKind::Undefined | ModuleValueKind::Null => {}
                        _ => match validate_snappy_options(context, options) {
                            Ok(validated) => font_options = validated,
                            Err(message) => return Ok(throw("TypeError", message)),
                        },
                    }
                }
                let instance = context.module_object(RESVG_INSTANCE)?;
                context.set_private(instance, SVG_SOURCE, svg)?;
                let default_family = context.string(&font_options.default_family)?;
                let sans_serif_family = context.string(&font_options.sans_serif_family)?;
                context.set_private(instance, DEFAULT_FONT_FAMILY, default_family)?;
                context.set_private(instance, SANS_SERIF_FAMILY, sans_serif_family)?;
                attach_function(context, instance, "render", RENDER)?;
                Ok(ModuleCallResult::Return(instance))
            }
            RENDER => {
                let svg = context
                    .get_private(receiver, SVG_SOURCE)
                    .and_then(|value| context.as_string(value));
                let svg = match svg {
                    Ok(svg) => svg,
                    Err(_) => {
                        return Ok(throw(
                            "TypeError",
                            "Resvg.prototype.render called on incompatible receiver",
                        ));
                    }
                };
                let default_family = context
                    .get_private(receiver, DEFAULT_FONT_FAMILY)
                    .and_then(|value| context.as_string(value));
                let sans_serif_family = context
                    .get_private(receiver, SANS_SERIF_FAMILY)
                    .and_then(|value| context.as_string(value));
                let fonts = match (default_family, sans_serif_family) {
                    (Ok(default_family), Ok(sans_serif_family)) => FontOptions {
                        default_family,
                        sans_serif_family,
                    },
                    _ => {
                        return Ok(throw(
                            "TypeError",
                            "Resvg.prototype.render called on incompatible receiver",
                        ));
                    }
                };
                let (png, width, height) = match render_png(&svg, &fonts, context) {
                    Ok(rendered) => rendered,
                    Err(message) => return Ok(throw("Error", message)),
                };
                let png_bytes = context.array()?;
                for byte in png {
                    let byte = context.number(f64::from(byte))?;
                    context.array_push(png_bytes, byte)?;
                }
                let rendered = context.module_object(RENDERED_IMAGE)?;
                let width = context.number(f64::from(width))?;
                let height = context.number(f64::from(height))?;
                context.set_property(rendered, "width", width)?;
                context.set_property(rendered, "height", height)?;
                context.set_private(rendered, PNG_BYTES, png_bytes)?;
                attach_function(context, rendered, "asPng", AS_PNG)?;
                Ok(ModuleCallResult::Return(rendered))
            }
            AS_PNG => match context.get_private(receiver, PNG_BYTES) {
                Ok(bytes) => Ok(ModuleCallResult::Return(bytes)),
                Err(_) => Ok(throw(
                    "TypeError",
                    "RenderedImage.prototype.asPng called on incompatible receiver",
                )),
            },
            _ => Err(ModuleError::ContractViolation(
                "unknown resvg module function".into(),
            )),
        }
    }

    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[ValueHandle],
        _: Result<ValueHandle, String>,
        _: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "resvg module has no continuations".into(),
        ))
    }

    fn event(
        &self,
        _: u32,
        _: ValueHandle,
        _: ValueHandle,
        _: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "resvg module has no events".into(),
        ))
    }
}
