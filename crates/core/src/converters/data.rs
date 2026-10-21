use super::external::io_error;
use crate::converter::DetectedInput;
use crate::{
    detect_format, Availability, CancelToken, Converter, Format, Options, Result, VertexError,
};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use serde_json::{Map, Value};
use std::io::{BufWriter, Read, Write};
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
        let capacity = file
            .metadata()
            .ok()
            .map_or(0, |m| m.len().min(MAX_DATA_SIZE + 1) as usize);
        let mut text = String::with_capacity(capacity);
        file.take(MAX_DATA_SIZE + 1)
            .read_to_string(&mut text)
            .map_err(|e| io_error(input.path, e))?;
        if text.len() as u64 > MAX_DATA_SIZE {
            return Err(invalid(
                "Structured data is limited to 64 MiB per conversion",
            ));
        }
        let text = text.trim_start_matches('\u{feff}');
        if matches!(input.format, Format::Csv | Format::Tsv)
            && matches!(target, Format::Csv | Format::Tsv | Format::Json)
        {
            let staged = crate::path_utils::stage_output(output)?;
            let file = std::fs::File::create(&staged).map_err(|e| io_error(output, e))?;
            transcode_table(text, input.format, target, file, cancel)?;
            cancel.check()?;
            crate::path_utils::publish_exact(staged, output)?;
            progress(1.0);
            return Ok(());
        }
        if input.format == Format::Json && matches!(target, Format::Csv | Format::Tsv) {
            if let Some(bytes) = try_json_to_table(text, target, cancel)? {
                cancel.check()?;
                progress(0.5);
                let staged = crate::path_utils::stage_output(output)?;
                std::fs::write(&staged, bytes).map_err(|e| io_error(output, e))?;
                cancel.check()?;
                crate::path_utils::publish_exact(staged, output)?;
                progress(1.0);
                return Ok(());
            }
        }
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
    let mut row = csv::StringRecord::new();
    while reader.read_record(&mut row).map_err(invalid)? {
        cancel.check()?;
        rows.push(Value::Object(
            headers
                .iter()
                .zip(row.iter())
                .map(|(h, v)| (h.into(), Value::String(v.into())))
                .collect(),
        ));
    }
    Ok(Value::Array(rows))
}

/// Keep the historical sorted column order and strict validation, but avoid
/// building a JSON object (and copying its keys/cells) for every table row.
fn transcode_table(
    text: &str,
    from: Format,
    to: Format,
    output: impl Write,
    cancel: &CancelToken,
) -> Result<()> {
    let mut reader = csv::ReaderBuilder::new()
        .buffer_capacity(64 * 1024)
        .delimiter(if from == Format::Tsv { b'\t' } else { b',' })
        .from_reader(text.as_bytes());
    let headers = reader.headers().map_err(invalid)?.clone();
    let mut order: Vec<_> = (0..headers.len()).collect();
    order.sort_unstable_by_key(|&i| &headers[i]);
    if order
        .windows(2)
        .any(|pair| headers[pair[0]] == headers[pair[1]])
    {
        return Err(invalid(
            "Duplicate column names cannot be represented as JSON objects",
        ));
    }
    if to == Format::Json {
        let mut key_prefixes: Vec<Vec<u8>> = Vec::with_capacity(order.len());
        for (pos, &i) in order.iter().enumerate() {
            let mut prefix = Vec::with_capacity(headers[i].len() + 12);
            prefix.extend_from_slice(if pos == 0 { b"{\n    " } else { b",\n    " });
            serde_json::to_writer(&mut prefix, &headers[i]).map_err(invalid)?;
            prefix.extend_from_slice(b": ");
            key_prefixes.push(prefix);
        }
        let mut writer = BufWriter::with_capacity(64 * 1024, output);
        let mut row = csv::ByteRecord::new();
        let mut first_row = true;
        while reader.read_byte_record(&mut row).map_err(invalid)? {
            cancel.check()?;
            writer
                .write_all(if first_row { b"[\n  " } else { b",\n  " })
                .map_err(invalid)?;
            first_row = false;
            if key_prefixes.is_empty() {
                writer.write_all(b"{}").map_err(invalid)?;
            } else {
                for (prefix, &i) in key_prefixes.iter().zip(&order) {
                    writer.write_all(prefix).map_err(invalid)?;
                    let field = &row[i];
                    if !field.iter().any(|&b| b < 0x20 || b == b'"' || b == b'\\') {
                        writer.write_all(b"\"").map_err(invalid)?;
                        writer.write_all(field).map_err(invalid)?;
                        writer.write_all(b"\"").map_err(invalid)?;
                    } else {
                        let s = std::str::from_utf8(field).map_err(invalid)?;
                        serde_json::to_writer(&mut writer, s).map_err(invalid)?;
                    }
                }
                writer.write_all(b"\n  }").map_err(invalid)?;
            }
        }
        writer
            .write_all(if first_row { b"[]" } else { b"\n]" })
            .map_err(invalid)?;
        return writer.flush().map_err(invalid);
    }
    let mut writer = csv::WriterBuilder::new()
        .buffer_capacity(64 * 1024)
        .delimiter(if to == Format::Tsv { b'\t' } else { b',' })
        .from_writer(BufWriter::with_capacity(64 * 1024, output));
    let mut row = csv::ByteRecord::new();
    let is_sorted = order.iter().enumerate().all(|(idx, &col)| idx == col);
    // Header-only input historically maps to [] and therefore an empty output.
    if reader.read_byte_record(&mut row).map_err(invalid)? {
        writer
            .write_record(order.iter().map(|&i| &headers[i]))
            .map_err(invalid)?;
        loop {
            cancel.check()?;
            if is_sorted {
                writer.write_byte_record(&row).map_err(invalid)?;
            } else {
                writer
                    .write_record(order.iter().map(|&i| &row[i]))
                    .map_err(invalid)?;
            }
            if !reader.read_byte_record(&mut row).map_err(invalid)? {
                break;
            }
        }
    }
    writer.flush().map_err(invalid)
}

#[derive(Clone, Copy)]
enum CellSlot {
    Empty,
    True,
    False,
    Bytes(usize, usize),
}

fn write_slot<W: Write>(
    writer: &mut csv::Writer<W>,
    slot: CellSlot,
    scratch: &[u8],
) -> std::result::Result<(), csv::Error> {
    match slot {
        CellSlot::Empty => writer.write_field(b""),
        CellSlot::True => writer.write_field(b"true"),
        CellSlot::False => writer.write_field(b"false"),
        CellSlot::Bytes(start, end) => writer.write_field(&scratch[start..end]),
    }
}

struct CellSeed<'a> {
    scratch: &'a mut Vec<u8>,
    fallback: &'a mut bool,
}

impl<'de> serde::de::DeserializeSeed<'de> for CellSeed<'_> {
    type Value = CellSlot;
    fn deserialize<D: serde::de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for CellSeed<'_> {
    type Value = CellSlot;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a scalar value")
    }
    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(CellSlot::Empty)
    }
    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(CellSlot::Empty)
    }
    fn visit_bool<E>(self, v: bool) -> std::result::Result<Self::Value, E> {
        Ok(if v { CellSlot::True } else { CellSlot::False })
    }
    fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E> {
        let start = self.scratch.len();
        let _ = write!(self.scratch, "{v}");
        Ok(CellSlot::Bytes(start, self.scratch.len()))
    }
    fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E> {
        let start = self.scratch.len();
        let _ = write!(self.scratch, "{v}");
        Ok(CellSlot::Bytes(start, self.scratch.len()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
        let Some(n) = serde_json::Number::from_f64(v) else {
            *self.fallback = true;
            return Err(E::custom("fallback"));
        };
        let start = self.scratch.len();
        let _ = write!(self.scratch, "{n}");
        Ok(CellSlot::Bytes(start, self.scratch.len()))
    }
    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E> {
        let start = self.scratch.len();
        self.scratch.extend_from_slice(v.as_bytes());
        Ok(CellSlot::Bytes(start, self.scratch.len()))
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<Self::Value, E> {
        self.visit_str(&v)
    }
}

struct KeySeed<'a> {
    headers: &'a [String],
    expected_order: &'a [usize],
    step: usize,
    fallback: &'a mut bool,
}

impl<'de> serde::de::DeserializeSeed<'de> for KeySeed<'_> {
    type Value = usize;
    fn deserialize<D: serde::de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl<'de> serde::de::Visitor<'de> for KeySeed<'_> {
    type Value = usize;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an object key")
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
        if let Some(&idx) = self.expected_order.get(self.step) {
            if self.headers[idx] == v {
                return Ok(idx);
            }
        }
        match self.headers.binary_search_by(|h| h.as_str().cmp(v)) {
            Ok(idx) => Ok(idx),
            Err(_) => {
                *self.fallback = true;
                Err(E::custom("fallback"))
            }
        }
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<Self::Value, E> {
        self.visit_str(&v)
    }
}

struct FirstRowSeed<'a> {
    headers: &'a mut Vec<String>,
    slots: &'a mut Vec<CellSlot>,
    scratch: &'a mut Vec<u8>,
    fallback: &'a mut bool,
}

impl<'de> serde::de::DeserializeSeed<'de> for FirstRowSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> serde::de::Visitor<'de> for FirstRowSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a row object")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            let slot = map.next_value_seed(CellSeed {
                scratch: self.scratch,
                fallback: self.fallback,
            })?;
            self.headers.push(key);
            self.slots.push(slot);
        }
        Ok(())
    }
}

struct NextRowSeed<'a> {
    headers: &'a [String],
    expected_order: &'a [usize],
    slots: &'a mut [CellSlot],
    scratch: &'a mut Vec<u8>,
    fallback: &'a mut bool,
}

impl<'de> serde::de::DeserializeSeed<'de> for NextRowSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> serde::de::Visitor<'de> for NextRowSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a row object")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut step = 0;
        while let Some(col_idx) = map.next_key_seed(KeySeed {
            headers: self.headers,
            expected_order: self.expected_order,
            step,
            fallback: self.fallback,
        })? {
            step += 1;
            let slot = map.next_value_seed(CellSeed {
                scratch: self.scratch,
                fallback: self.fallback,
            })?;
            self.slots[col_idx] = slot;
        }
        Ok(())
    }
}

struct TableSeqVisitor<'a> {
    writer: &'a mut csv::Writer<Vec<u8>>,
    cancel: &'a CancelToken,
    cancelled: &'a mut bool,
    fallback: &'a mut bool,
}

impl<'de> serde::de::Visitor<'de> for TableSeqVisitor<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an array of objects")
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        if self.cancel.check().is_err() {
            *self.cancelled = true;
            return Err(serde::de::Error::custom("cancelled"));
        }
        let mut raw_headers: Vec<String> = Vec::new();
        let mut first_slots: Vec<CellSlot> = Vec::new();
        let mut scratch: Vec<u8> = Vec::with_capacity(256);
        let Some(()) = seq.next_element_seed(FirstRowSeed {
            headers: &mut raw_headers,
            slots: &mut first_slots,
            scratch: &mut scratch,
            fallback: self.fallback,
        })?
        else {
            return Ok(());
        };
        if raw_headers.is_empty() {
            *self.fallback = true;
            return Err(serde::de::Error::custom("fallback"));
        }
        let mut order: Vec<usize> = (0..raw_headers.len()).collect();
        order.sort_unstable_by(|&a, &b| raw_headers[a].cmp(&raw_headers[b]));
        if order
            .windows(2)
            .any(|w| raw_headers[w[0]] == raw_headers[w[1]])
        {
            *self.fallback = true;
            return Err(serde::de::Error::custom("fallback"));
        }
        let mut expected_order = vec![0usize; order.len()];
        let mut sorted_headers = Vec::with_capacity(order.len());
        for (sorted_idx, &raw_pos) in order.iter().enumerate() {
            expected_order[raw_pos] = sorted_idx;
            sorted_headers.push(std::mem::take(&mut raw_headers[raw_pos]));
        }
        self.writer
            .write_record(&sorted_headers)
            .map_err(serde::de::Error::custom)?;
        for &raw_pos in &order {
            write_slot(self.writer, first_slots[raw_pos], &scratch)
                .map_err(serde::de::Error::custom)?;
        }
        self.writer
            .write_record(std::iter::empty::<&[u8]>())
            .map_err(serde::de::Error::custom)?;

        let mut slots = vec![CellSlot::Empty; sorted_headers.len()];
        loop {
            if self.cancel.check().is_err() {
                *self.cancelled = true;
                return Err(serde::de::Error::custom("cancelled"));
            }
            scratch.clear();
            slots.fill(CellSlot::Empty);
            let has_row = seq.next_element_seed(NextRowSeed {
                headers: &sorted_headers,
                expected_order: &expected_order,
                slots: &mut slots,
                scratch: &mut scratch,
                fallback: self.fallback,
            })?;
            if has_row.is_none() {
                break;
            }
            for &slot in &slots {
                write_slot(self.writer, slot, &scratch).map_err(serde::de::Error::custom)?;
            }
            self.writer
                .write_record(std::iter::empty::<&[u8]>())
                .map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}

fn try_json_to_table(text: &str, format: Format, cancel: &CancelToken) -> Result<Option<Vec<u8>>> {
    use serde::de::Deserializer as _;
    let mut writer = csv::WriterBuilder::new()
        .buffer_capacity(64 * 1024)
        .delimiter(if format == Format::Tsv { b'\t' } else { b',' })
        .from_writer(Vec::with_capacity(text.len() / 2));
    let mut cancelled = false;
    let mut fallback = false;
    let mut de = serde_json::Deserializer::from_str(text);
    let res = de.deserialize_seq(TableSeqVisitor {
        writer: &mut writer,
        cancel,
        cancelled: &mut cancelled,
        fallback: &mut fallback,
    });
    match res {
        Ok(()) => {
            de.end().map_err(invalid)?;
            Ok(Some(writer.into_inner().map_err(invalid)?))
        }
        Err(_) if cancelled => {
            cancel.check()?;
            Err(VertexError::Cancelled)
        }
        Err(e) if fallback || e.classify() == serde_json::error::Category::Data => Ok(None),
        Err(e) => Err(invalid(e)),
    }
}

fn write_scalar_field<W: Write>(
    writer: &mut csv::Writer<W>,
    cell: Option<&Value>,
    num_buf: &mut String,
) -> Result<()> {
    use std::fmt::Write as _;
    match cell {
        None | Some(Value::Null) => writer.write_field(b""),
        Some(Value::String(s)) => writer.write_field(s.as_bytes()),
        Some(Value::Bool(b)) => writer.write_field(if *b { &b"true"[..] } else { &b"false"[..] }),
        Some(Value::Number(n)) => {
            num_buf.clear();
            let _ = write!(num_buf, "{n}");
            writer.write_field(num_buf.as_bytes())
        }
        _ => {
            return Err(invalid(
                "Nested objects and arrays cannot be flattened to CSV/TSV without losing data",
            ))
        }
    }
    .map_err(invalid)
}

fn write_table(value: &Value, format: Format, cancel: &CancelToken) -> Result<Vec<u8>> {
    let rows = value
        .as_array()
        .ok_or_else(|| invalid("CSV/TSV requires an array of objects with scalar cells"))?;
    let mut headers = std::collections::BTreeSet::new();
    for row in rows {
        let obj = row
            .as_object()
            .ok_or_else(|| invalid("Each table row must be an object"))?;
        if headers.is_empty()
            || obj.len() != headers.len()
            || !obj.keys().eq(headers.iter().copied())
        {
            headers.extend(obj.keys());
        }
    }
    let headers: Vec<_> = headers.into_iter().collect();
    let mut writer = csv::WriterBuilder::new()
        .buffer_capacity(64 * 1024)
        .delimiter(if format == Format::Tsv { b'\t' } else { b',' })
        .from_writer(Vec::new());
    if !headers.is_empty() {
        writer.write_record(&headers).map_err(invalid)?;
    }
    let mut num_buf = String::with_capacity(32);
    for row in rows {
        cancel.check()?;
        let obj = row.as_object().unwrap();
        if obj.len() == headers.len() {
            for val in obj.values() {
                write_scalar_field(&mut writer, Some(val), &mut num_buf)?;
            }
        } else {
            let mut iter = obj.iter().peekable();
            for &h in &headers {
                let cell = if iter.peek().is_some_and(|(k, _)| *k == h) {
                    iter.next().map(|(_, v)| v)
                } else {
                    None
                };
                write_scalar_field(&mut writer, cell, &mut num_buf)?;
            }
        }
        writer
            .write_record(std::iter::empty::<&[u8]>())
            .map_err(invalid)?;
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

struct XmlFrame {
    name: String,
    map: Map<String, Value>,
    text: Option<String>,
    has_elements: bool,
}

fn read_xml(text: &str, cancel: &CancelToken) -> Result<Value> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<XmlFrame> = Vec::new();
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
                    let raw_key = attr.key.as_ref();
                    let mut key = String::with_capacity(1 + raw_key.len());
                    key.push('@');
                    key.push_str(raw_key);
                    map.insert(
                        key,
                        Value::String(
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(invalid)?
                                .into_owned(),
                        ),
                    );
                }
                if empty {
                    if let Some(parent) = stack.last_mut() {
                        parent.has_elements = true;
                        add_child(&mut parent.map, name, Value::Object(map));
                    } else {
                        add_child(&mut root, name, Value::Object(map));
                    }
                } else {
                    stack.push(XmlFrame {
                        name,
                        map,
                        text: None,
                        has_elements: false,
                    });
                }
            }
            Event::End(_) => {
                let frame = stack
                    .pop()
                    .ok_or_else(|| invalid("Unexpected XML closing tag"))?;
                let mut map = frame.map;
                let value = if frame.has_elements {
                    if frame.text.as_deref().is_some_and(|s| !s.trim().is_empty()) {
                        return Err(invalid("Mixed XML text and child elements cannot be represented by this data mapping without changing their order"));
                    }
                    Value::Object(map)
                } else if let Some(text) = frame.text {
                    if map.is_empty() {
                        Value::String(text)
                    } else {
                        map.insert("$text".into(), Value::String(text));
                        Value::Object(map)
                    }
                } else {
                    Value::Object(map)
                };
                if let Some(parent) = stack.last_mut() {
                    parent.has_elements = true;
                    add_child(&mut parent.map, frame.name, value);
                } else {
                    add_child(&mut root, frame.name, value);
                }
            }
            Event::Text(e) => {
                let text = quick_xml::escape::unescape(e.as_ref()).map_err(invalid)?;
                if let Some(frame) = stack.last_mut() {
                    match &mut frame.text {
                        Some(s) => s.push_str(&text),
                        slot @ None => *slot = Some(text.into_owned()),
                    }
                } else if !text.trim().is_empty() {
                    return Err(invalid("Text outside the XML root"));
                }
            }
            Event::CData(e) => {
                if let Some(frame) = stack.last_mut() {
                    frame
                        .text
                        .get_or_insert_with(String::new)
                        .push_str(e.as_ref());
                }
            }
            Event::DocType(_) => {
                return Err(invalid("XML document type declarations are unsupported"))
            }
            Event::GeneralRef(e) => {
                let reference = format!("&{};", e.as_ref());
                let decoded = quick_xml::escape::unescape(&reference).map_err(invalid)?;
                if let Some(frame) = stack.last_mut() {
                    frame
                        .text
                        .get_or_insert_with(String::new)
                        .push_str(&decoded);
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
    let valid = if name.is_ascii() {
        let bytes = name.as_bytes();
        bytes
            .first()
            .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'_')
            && bytes[1..]
                .iter()
                .all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
    } else {
        let mut chars = name.chars();
        chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
            && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
    };
    if !valid {
        return Err(invalid(format!(
            "Invalid XML element/attribute name: {name}"
        )));
    }
    Ok(())
}
fn xml_text(text: &str) -> Result<&str> {
    let invalid_char = if text.is_ascii() {
        text.bytes()
            .any(|b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r'))
    } else {
        text.chars().any(|c| {
            !(matches!(c, '\t' | '\n' | '\r')
                || (c >= '\u{20}' && c != '\u{fffe}' && c != '\u{ffff}'))
        })
    };
    if invalid_char {
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
    if let Value::Object(map) = value {
        for (key, value) in map {
            if let Some(attr) = key.strip_prefix('@') {
                xml_name(attr)?;
                match value {
                    Value::String(s) => {
                        start.push_attribute((attr, xml_text(s)?));
                    }
                    Value::Bool(b) => {
                        start.push_attribute((attr, if *b { "true" } else { "false" }));
                    }
                    Value::Number(n) => {
                        let s = n.to_string();
                        start.push_attribute((attr, xml_text(&s)?));
                    }
                    Value::Null => {
                        start.push_attribute((attr, "null"));
                    }
                    Value::Object(_) | Value::Array(_) => {
                        return Err(invalid("XML attributes must be scalar"));
                    }
                }
            }
        }
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
        Value::String(s) => {
            writer
                .write_event(Event::Text(BytesText::new(xml_text(s)?)))
                .map_err(invalid)?;
        }
        Value::Bool(b) => {
            writer
                .write_event(Event::Text(BytesText::new(if *b {
                    "true"
                } else {
                    "false"
                })))
                .map_err(invalid)?;
        }
        Value::Number(n) => {
            let text = n.to_string();
            writer
                .write_event(Event::Text(BytesText::new(xml_text(&text)?)))
                .map_err(invalid)?;
        }
        Value::Array(_) => unreachable!(),
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
        let text = super::text::read_unicode(input.path, cancel)?;
        let target = Format::from_path_extension(output);
        let staged = crate::path_utils::stage_output(output)?;
        let io = |e| io_error(output, e);
        if target == Some(Format::Html) {
            if input.format == Format::Md {
                let mut html = String::with_capacity(text.len() + text.len() / 2 + 128);
                html.push_str("<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>");
                pulldown_cmark::html::push_html(
                    &mut html,
                    pulldown_cmark::Parser::new_ext(&text, pulldown_cmark::Options::all()),
                );
                html.push_str("</body></html>");
                std::fs::write(&staged, html.as_bytes()).map_err(io)?;
            } else {
                let mut writer = BufWriter::with_capacity(
                    64 * 1024,
                    std::fs::File::create(&staged).map_err(io)?,
                );
                // HTML strips the first newline after <pre>; supply that newline
                // ourselves so an input's leading blank line remains visible.
                writer
                    .write_all(
                        b"<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body><pre>\n",
                    )
                    .map_err(io)?;
                let bytes = text.as_bytes();
                let mut last = 0;
                for (i, &b) in bytes.iter().enumerate() {
                    let escaped: &[u8] = match b {
                        b'<' => b"&lt;",
                        b'>' => b"&gt;",
                        b'&' => b"&amp;",
                        b'\'' => b"&apos;",
                        b'"' => b"&quot;",
                        _ => continue,
                    };
                    if last < i {
                        writer.write_all(&bytes[last..i]).map_err(io)?;
                    }
                    writer.write_all(escaped).map_err(io)?;
                    last = i + 1;
                }
                if last < bytes.len() {
                    writer.write_all(&bytes[last..]).map_err(io)?;
                }
                writer.write_all(b"</pre></body></html>").map_err(io)?;
                writer.flush().map_err(io)?;
            }
        } else if target == Some(Format::Txt) && input.format == Format::Md {
            let mut writer =
                BufWriter::with_capacity(64 * 1024, std::fs::File::create(&staged).map_err(io)?);
            for event in pulldown_cmark::Parser::new(&text) {
                cancel.check()?;
                match event {
                    pulldown_cmark::Event::Text(s) | pulldown_cmark::Event::Code(s) => {
                        writer.write_all(s.as_bytes()).map_err(io)?;
                    }
                    pulldown_cmark::Event::SoftBreak
                    | pulldown_cmark::Event::HardBreak
                    | pulldown_cmark::Event::End(_) => writer.write_all(b"\n").map_err(io)?,
                    _ => {}
                }
            }
            writer.flush().map_err(io)?;
        } else {
            return Err(invalid("Unsupported text conversion"));
        }
        cancel.check()?;
        crate::path_utils::publish_exact(staged, output)?;
        progress(1.0);
        Ok(())
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    #[test]
    fn streamed_json_matches_the_original_value_serializer() {
        let cancel = CancelToken::new();
        for text in [
            "",
            "z,a\n",
            "z,a\n001,true\n\"line\n😀\",Tiếng Việt\n",
            "\"\",a\n,\"tab\tand \\\"\n",
        ] {
            let value = read_table(text, Format::Csv, &cancel).unwrap();
            let mut output = Vec::new();
            transcode_table(text, Format::Csv, Format::Json, &mut output, &cancel).unwrap();
            assert_eq!(output, serde_json::to_vec_pretty(&value).unwrap());
        }
    }

    #[test]
    fn streamed_json_to_table_matches_write_table() {
        let cancel = CancelToken::new();
        for json in [
            "[]",
            r#"[{"z": 1, "a": "hello"}, {"z": 2, "a": "world"}]"#,
            r#"[{"z": null, "a": true, "m": 1.5}, {"a": false, "z": "line\n😀"}]"#,
        ] {
            let value: Value = serde_json::from_str(json).unwrap();
            let expected = write_table(&value, Format::Csv, &cancel).unwrap();
            let actual = try_json_to_table(json, Format::Csv, &cancel)
                .unwrap()
                .unwrap();
            assert_eq!(actual, expected);
        }
        // Sparse table introducing a new column after row 0 falls back cleanly.
        let sparse = r#"[{"a": 1}, {"a": 2, "b": 3}]"#;
        assert!(try_json_to_table(sparse, Format::Csv, &cancel)
            .unwrap()
            .is_none());
    }
}
