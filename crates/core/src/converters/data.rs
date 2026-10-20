use super::external::io_error;
use crate::converter::DetectedInput;
use crate::{
    detect_format, Availability, CancelToken, Converter, Format, Options, Result, VertexError,
};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use serde_json::{Map, Value};
use std::io::Read;
use std::path::Path;

const FORMATS: &[Format] = &[
    Format::Csv,
    Format::Tsv,
    Format::Json,
    Format::Yaml,
    Format::Toml,
    Format::Xml,
];
const MAX_DATA_SIZE: u64 = 64 * 1024 * 1024;
const MAX_XML_DEPTH: usize = 128;

pub struct DataConverter;
fn invalid(error: impl std::fmt::Display) -> VertexError {
    VertexError::Encoding(error.to_string())
}

impl Converter for DataConverter {
    fn name(&self) -> &'static str {
        "DataConverter"
    }
    fn sources(&self) -> &[Format] {
        FORMATS
    }
    fn targets(&self, _: Format) -> Vec<Format> {
        FORMATS.to_vec()
    }
    fn available(&self) -> Availability {
        Availability::Ok
    }
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        self.convert_detected(
            DetectedInput {
                path: input,
                format: detect_format(input)?,
            },
            output,
            opts,
            progress,
            cancel,
        )
    }
    fn convert_detected(
        &self,
        input: DetectedInput<'_>,
        output: &Path,
        _: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        crate::path_utils::ensure_distinct(input.path, output)?;
        let target = Format::from_path_extension(output)
            .ok_or_else(|| VertexError::UnrecognizedFormat(output.into()))?;
        if !FORMATS.contains(&input.format) || !FORMATS.contains(&target) {
            return Err(invalid("Unsupported data conversion"));
        }
        let file = std::fs::File::open(input.path).map_err(|e| io_error(input.path, e))?;
        let mut text = String::new();
        file.take(MAX_DATA_SIZE + 1)
            .read_to_string(&mut text)
            .map_err(|e| io_error(input.path, e))?;
        if text.len() as u64 > MAX_DATA_SIZE {
            return Err(invalid(
                "Structured data is limited to 64 MiB per conversion",
            ));
        }
        let text = text.trim_start_matches('\u{feff}');
        let value: Value = match input.format {
            Format::Json => serde_json::from_str(text).map_err(invalid)?,
            Format::Yaml => {
                let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).map_err(invalid)?;
                validate_yaml(&yaml, 0, cancel)?;
                serde_json::to_value(yaml).map_err(invalid)?
            }
            Format::Toml => {
                let toml: toml::Value = toml::from_str(text).map_err(invalid)?;
                validate_toml(&toml, 0, cancel)?;
                serde_json::to_value(toml).map_err(invalid)?
            }
            Format::Csv | Format::Tsv => read_table(text, input.format, cancel)?,
            Format::Xml => read_xml(text, cancel)?,
            _ => unreachable!(),
        };
        cancel.check()?;
        progress(0.5);
        let staged = crate::path_utils::stage_output(output)?;
        let bytes = match target {
            Format::Json => serde_json::to_vec_pretty(&value).map_err(invalid)?,
            Format::Yaml => serde_yaml_ng::to_string(&value)
                .map_err(invalid)?
                .into_bytes(),
            Format::Toml => toml::to_string_pretty(&value)
                .map_err(invalid)?
                .into_bytes(),
            Format::Csv | Format::Tsv => write_table(&value, target, cancel)?,
            Format::Xml => write_xml(&value, cancel)?,
            _ => unreachable!(),
        };
        std::fs::write(&staged, bytes).map_err(|e| io_error(output, e))?;
        cancel.check()?;
        crate::path_utils::publish_exact(staged, output)?;
        progress(1.0);
        Ok(())
    }
}

fn validate_yaml(value: &serde_yaml_ng::Value, depth: usize, cancel: &CancelToken) -> Result<()> {
    cancel.check()?;
    if depth > MAX_XML_DEPTH {
        return Err(invalid("Data nesting exceeds 128 levels"));
    }
    use serde_yaml_ng::Value as Y;
    match value {
        Y::Number(number) if number.as_f64().is_some_and(|n| !n.is_finite()) => {
            return Err(invalid("Non-finite numbers cannot be represented as JSON"))
        }
        Y::Tagged(_) => {
            return Err(invalid(
                "YAML tags cannot be preserved by the structured-data mapping",
            ))
        }
        Y::Sequence(items) => {
            for item in items {
                validate_yaml(item, depth + 1, cancel)?;
            }
        }
        Y::Mapping(map) => {
            for (key, value) in map {
                if !matches!(key, Y::String(_)) {
                    return Err(invalid("Structured-data object keys must be strings"));
                }
                validate_yaml(value, depth + 1, cancel)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn validate_toml(value: &toml::Value, depth: usize, cancel: &CancelToken) -> Result<()> {
    cancel.check()?;
    if depth > MAX_XML_DEPTH {
        return Err(invalid("Data nesting exceeds 128 levels"));
    }
    match value {
        toml::Value::Float(number) if !number.is_finite() => {
            return Err(invalid("Non-finite numbers cannot be represented as JSON"))
        }
        toml::Value::Array(items) => {
            for item in items {
                validate_toml(item, depth + 1, cancel)?;
            }
        }
        toml::Value::Table(map) => {
            for value in map.values() {
                validate_toml(value, depth + 1, cancel)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn read_table(text: &str, format: Format, cancel: &CancelToken) -> Result<Value> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(if format == Format::Tsv { b'\t' } else { b',' })
        .from_reader(text.as_bytes());
    let headers = reader.headers().map_err(invalid)?.clone();
    let mut unique = std::collections::HashSet::new();
    if headers.iter().any(|h| !unique.insert(h)) {
        return Err(invalid(
            "Duplicate column names cannot be represented as JSON objects",
        ));
    }
    let mut rows = Vec::new();
    for row in reader.records() {
        cancel.check()?;
        rows.push(Value::Object(
            headers
                .iter()
                .zip(row.map_err(invalid)?.iter())
                .map(|(h, v)| (h.into(), Value::String(v.into())))
                .collect(),
        ));
    }
    Ok(Value::Array(rows))
}

fn write_table(value: &Value, format: Format, cancel: &CancelToken) -> Result<Vec<u8>> {
    let rows = value
        .as_array()
        .ok_or_else(|| invalid("CSV/TSV requires an array of objects with scalar cells"))?;
    let mut headers = std::collections::BTreeSet::new();
    for row in rows {
        headers.extend(
            row.as_object()
                .ok_or_else(|| invalid("Each table row must be an object"))?
                .keys()
                .cloned(),
        );
    }
    let headers: Vec<_> = headers.into_iter().collect();
    let mut writer = csv::WriterBuilder::new()
        .delimiter(if format == Format::Tsv { b'\t' } else { b',' })
        .from_writer(Vec::new());
    if !headers.is_empty() {
        writer.write_record(&headers).map_err(invalid)?;
    }
    for row in rows {
        cancel.check()?;
        let cells = headers
            .iter()
            .map(|h| match row.get(h) {
                None | Some(Value::Null) => Ok(String::new()),
                Some(Value::String(s)) => Ok(s.clone()),
                Some(Value::Bool(_) | Value::Number(_)) => Ok(row[h].to_string()),
                _ => Err(invalid(
                    "Nested objects and arrays cannot be flattened to CSV/TSV without losing data",
                )),
            })
            .collect::<Result<Vec<_>>>()?;
        writer.write_record(cells).map_err(invalid)?;
    }
    writer.into_inner().map_err(invalid)
}

// Conventional XML mapping: @name attributes, $text content, arrays for repeated
// elements. XML scalars remain strings; JSON arrays use repeated child elements.
fn add_child(map: &mut Map<String, Value>, name: String, value: Value) {
    if let Some(previous) = map.remove(&name) {
        let mut list = match previous {
            Value::Array(list) => list,
            other => vec![other],
        };
        list.push(value);
        map.insert(name, Value::Array(list));
    } else {
        map.insert(name, value);
    }
}

fn read_xml(text: &str, cancel: &CancelToken) -> Result<Value> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<(String, Map<String, Value>)> = Vec::new();
    let mut root = Map::new();
    loop {
        cancel.check()?;
        let event = reader.read_event().map_err(invalid)?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                if stack.is_empty() && !root.is_empty() {
                    return Err(invalid("XML requires exactly one root element"));
                }
                if stack.len() >= MAX_XML_DEPTH {
                    return Err(invalid("XML nesting exceeds 128 levels"));
                }
                let name = e.name().as_ref().to_owned();
                let mut map = Map::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(invalid)?;
                    map.insert(
                        format!("@{}", attr.key.as_ref()),
                        Value::String(
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(invalid)?
                                .into_owned(),
                        ),
                    );
                }
                if empty {
                    let parent = stack.last_mut().map(|(_, m)| m).unwrap_or(&mut root);
                    add_child(parent, name, Value::Object(map));
                } else {
                    stack.push((name, map));
                }
            }
            Event::End(_) => {
                let (name, mut map) = stack
                    .pop()
                    .ok_or_else(|| invalid("Unexpected XML closing tag"))?;
                if map
                    .keys()
                    .any(|key| !key.starts_with('@') && key != "$text")
                {
                    if map
                        .get("$text")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.trim().is_empty())
                    {
                        return Err(invalid("Mixed XML text and child elements cannot be represented by this data mapping without changing their order"));
                    }
                    map.remove("$text");
                }
                let value = if map.len() == 1 && map.contains_key("$text") {
                    map.remove("$text").unwrap()
                } else {
                    Value::Object(map)
                };
                add_child(
                    stack.last_mut().map(|(_, m)| m).unwrap_or(&mut root),
                    name,
                    value,
                );
            }
            Event::Text(e) => {
                let text = quick_xml::escape::unescape(e.as_ref()).map_err(invalid)?;
                if let Some((_, map)) = stack.last_mut() {
                    map.entry("$text")
                        .or_insert_with(|| Value::String(String::new()))
                        .as_str()
                        .ok_or_else(|| invalid("Invalid XML text"))?;
                    if let Some(Value::String(s)) = map.get_mut("$text") {
                        s.push_str(&text);
                    }
                } else if !text.trim().is_empty() {
                    return Err(invalid("Text outside the XML root"));
                }
            }
            Event::CData(e) => {
                if let Some((_, map)) = stack.last_mut() {
                    let Value::String(s) = map
                        .entry("$text")
                        .or_insert_with(|| Value::String(String::new()))
                    else {
                        return Err(invalid("Invalid XML text"));
                    };
                    s.push_str(e.as_ref());
                }
            }
            Event::DocType(_) => {
                return Err(invalid("XML document type declarations are unsupported"))
            }
            Event::GeneralRef(e) => {
                let reference = format!("&{};", e.as_ref());
                let decoded = quick_xml::escape::unescape(&reference).map_err(invalid)?;
                if let Some((_, map)) = stack.last_mut() {
                    let Value::String(s) = map
                        .entry("$text")
                        .or_insert_with(|| Value::String(String::new()))
                    else {
                        return Err(invalid("Invalid XML text"));
                    };
                    s.push_str(&decoded);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() || root.len() != 1 {
        return Err(invalid("XML requires exactly one complete root element"));
    }
    Ok(Value::Object(root))
}

fn xml_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        || !chars.all(|c| c.is_alphanumeric() || "_-.:".contains(c))
    {
        return Err(invalid(format!(
            "Invalid XML element/attribute name: {name}"
        )));
    }
    Ok(())
}
fn xml_text(text: &str) -> Result<&str> {
    if text.chars().any(|c| {
        !(matches!(c, '\t' | '\n' | '\r') || (c >= '\u{20}' && c != '\u{fffe}' && c != '\u{ffff}'))
    }) {
        return Err(invalid("Text contains characters forbidden by XML 1.0"));
    }
    Ok(text)
}
fn write_element(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    name: &str,
    value: &Value,
    depth: usize,
    cancel: &CancelToken,
) -> Result<()> {
    cancel.check()?;
    if depth > MAX_XML_DEPTH {
        return Err(invalid("XML nesting exceeds 128 levels"));
    }
    xml_name(name)?;
    if let Value::Array(items) = value {
        for item in items {
            write_element(writer, name, item, depth + 1, cancel)?;
        }
        return Ok(());
    }
    let mut start = BytesStart::new(name);
    let mut attributes = Vec::new();
    if let Value::Object(map) = value {
        for (key, value) in map {
            if let Some(attr) = key.strip_prefix('@') {
                xml_name(attr)?;
                if value.is_object() || value.is_array() {
                    return Err(invalid("XML attributes must be scalar"));
                }
                attributes.push((
                    attr.to_owned(),
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                ));
            }
        }
    }
    for (key, value) in &attributes {
        xml_text(value)?;
        start.push_attribute((key.as_str(), value.as_str()));
    }
    writer.write_event(Event::Start(start)).map_err(invalid)?;
    match value {
        Value::Object(map) => {
            let has_children = map
                .keys()
                .any(|key| !key.starts_with('@') && key != "$text");
            for (key, value) in map {
                if key == "$text" {
                    let text = value
                        .as_str()
                        .ok_or_else(|| invalid("$text must be a string"))?;
                    if has_children {
                        if !text.trim().is_empty() {
                            return Err(invalid("Mixed XML text and child elements cannot be represented by this data mapping without changing their order"));
                        }
                    } else {
                        writer
                            .write_event(Event::Text(BytesText::new(xml_text(text)?)))
                            .map_err(invalid)?;
                    }
                } else if !key.starts_with('@') {
                    write_element(writer, key, value, depth + 1, cancel)?;
                }
            }
        }
        Value::Null => {}
        scalar => {
            let text = scalar
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| scalar.to_string());
            writer
                .write_event(Event::Text(BytesText::new(xml_text(&text)?)))
                .map_err(invalid)?;
        }
    }
    writer
        .write_event(Event::End(BytesEnd::new(name)))
        .map_err(invalid)?;
    Ok(())
}
fn write_xml(value: &Value, cancel: &CancelToken) -> Result<Vec<u8>> {
    let mut writer = quick_xml::Writer::new(Vec::new());
    if let Value::Object(map) = value {
        if map.len() == 1 && !map.values().next().unwrap().is_array() {
            let (name, value) = map.iter().next().unwrap();
            if !name.starts_with('@') && name != "$text" {
                write_element(&mut writer, name, value, 0, cancel)?;
                return Ok(writer.into_inner());
            }
        }
    }
    if let Value::Array(items) = value {
        writer
            .write_event(Event::Start(BytesStart::new("root")))
            .map_err(invalid)?;
        for item in items {
            write_element(&mut writer, "item", item, 1, cancel)?;
        }
        writer
            .write_event(Event::End(BytesEnd::new("root")))
            .map_err(invalid)?;
    } else {
        write_element(&mut writer, "root", value, 0, cancel)?;
    }
    Ok(writer.into_inner())
}

pub struct TextConverter;
impl Converter for TextConverter {
    fn name(&self) -> &'static str {
        "TextConverter"
    }
    fn sources(&self) -> &[Format] {
        &[Format::Txt, Format::Md]
    }
    fn targets(&self, from: Format) -> Vec<Format> {
        if from == Format::Md {
            vec![Format::Html, Format::Txt]
        } else {
            vec![Format::Html]
        }
    }
    fn available(&self) -> Availability {
        Availability::Ok
    }
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        opts: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        self.convert_detected(
            DetectedInput {
                path: input,
                format: detect_format(input)?,
            },
            output,
            opts,
            progress,
            cancel,
        )
    }
    fn convert_detected(
        &self,
        input: DetectedInput<'_>,
        output: &Path,
        _: &Options,
        progress: &dyn Fn(f32),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        crate::path_utils::ensure_distinct(input.path, output)?;
        let file = std::fs::File::open(input.path).map_err(|e| io_error(input.path, e))?;
        let mut text = String::new();
        file.take(MAX_DATA_SIZE + 1)
            .read_to_string(&mut text)
            .map_err(|e| io_error(input.path, e))?;
        if text.len() as u64 > MAX_DATA_SIZE {
            return Err(invalid("Text input is limited to 64 MiB per conversion"));
        }
        let target = Format::from_path_extension(output);
        let result = if target == Some(Format::Html) {
            let mut body = String::new();
            if input.format == Format::Md {
                pulldown_cmark::html::push_html(
                    &mut body,
                    pulldown_cmark::Parser::new_ext(&text, pulldown_cmark::Options::all()),
                );
            } else {
                body = format!("<pre>{}</pre>", quick_xml::escape::escape(&text));
            }
            format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>{body}</body></html>")
        } else if target == Some(Format::Txt) && input.format == Format::Md {
            let mut body = String::new();
            for event in pulldown_cmark::Parser::new(&text) {
                cancel.check()?;
                match event {
                    pulldown_cmark::Event::Text(s) | pulldown_cmark::Event::Code(s) => {
                        body.push_str(&s)
                    }
                    pulldown_cmark::Event::SoftBreak
                    | pulldown_cmark::Event::HardBreak
                    | pulldown_cmark::Event::End(_) => body.push('\n'),
                    _ => {}
                }
            }
            body
        } else {
            return Err(invalid("Unsupported text conversion"));
        };
        let staged = crate::path_utils::stage_output(output)?;
        std::fs::write(&staged, result).map_err(|e| io_error(output, e))?;
        cancel.check()?;
        crate::path_utils::publish_exact(staged, output)?;
        progress(1.0);
        Ok(())
    }
}
