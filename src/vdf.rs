//! Steam's binary key-value format, as used by `shortcuts.vdf`.
//!
//! The file is a tree. Each item starts with a type byte and a NUL-terminated key; a map
//! then holds more items until an end byte. Order is kept and every type is written back as
//! it was read, so what the launcher does not understand survives a rewrite. Anything it
//! cannot read exactly is an error: the file is then left alone.

const MAP: u8 = 0x00;
const STRING: u8 = 0x01;
const U32: u8 = 0x02;
const FLOAT: u8 = 0x03;
const U64: u8 = 0x07;
const END: u8 = 0x08;
const I64: u8 = 0x0A;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Map(Vec<(String, Value)>),
    Str(String),
    U32(u32),
    /// The bits of a 32-bit float, so a round trip is exact.
    Float(u32),
    U64(u64),
    I64(i64),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Value::Map(items) => items.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn items(&self) -> &[(String, Value)] {
        match self {
            Value::Map(items) => items,
            _ => &[],
        }
    }
}

/// Reads a whole file as an unnamed root map, which ends with an end byte like any other.
pub fn parse(bytes: &[u8]) -> Result<Value, String> {
    let mut at = 0;
    let items = read_items(bytes, &mut at)?;
    if at != bytes.len() {
        return Err("data after the end of the file".to_owned());
    }
    Ok(Value::Map(items))
}

/// Writes a root map the way Steam does, items followed by the end byte.
pub fn write(root: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_items(&mut out, root.items());
    out
}

fn read_items(bytes: &[u8], at: &mut usize) -> Result<Vec<(String, Value)>, String> {
    let mut items = Vec::new();
    loop {
        let Some(&kind) = bytes.get(*at) else {
            return Err("a map is not closed".to_owned());
        };
        *at += 1;
        if kind == END {
            return Ok(items);
        }
        let key = read_string(bytes, at)?;
        let value = match kind {
            MAP => Value::Map(read_items(bytes, at)?),
            STRING => Value::Str(read_string(bytes, at)?),
            U32 => Value::U32(u32::from_le_bytes(take(bytes, at)?)),
            FLOAT => Value::Float(u32::from_le_bytes(take(bytes, at)?)),
            U64 => Value::U64(u64::from_le_bytes(take(bytes, at)?)),
            I64 => Value::I64(i64::from_le_bytes(take(bytes, at)?)),
            other => return Err(format!("unknown item type {other:#04x}")),
        };
        items.push((key, value));
    }
}

fn take<const N: usize>(bytes: &[u8], at: &mut usize) -> Result<[u8; N], String> {
    let end = *at + N;
    let slice = bytes.get(*at..end).ok_or("the file ends inside a number")?;
    *at = end;
    Ok(slice.try_into().expect("slice has length N"))
}

fn read_string(bytes: &[u8], at: &mut usize) -> Result<String, String> {
    let rest = bytes.get(*at..).unwrap_or_default();
    let len = rest
        .iter()
        .position(|b| *b == 0)
        .ok_or("the file ends inside a string")?;
    let text = std::str::from_utf8(&rest[..len]).map_err(|_| "text is not UTF-8".to_owned())?;
    *at += len + 1;
    Ok(text.to_owned())
}

fn write_items(out: &mut Vec<u8>, items: &[(String, Value)]) {
    for (key, value) in items {
        out.push(match value {
            Value::Map(_) => MAP,
            Value::Str(_) => STRING,
            Value::U32(_) => U32,
            Value::Float(_) => FLOAT,
            Value::U64(_) => U64,
            Value::I64(_) => I64,
        });
        out.extend(key.as_bytes());
        out.push(0);
        match value {
            Value::Map(inner) => write_items(out, inner),
            Value::Str(s) => {
                out.extend(s.as_bytes());
                out.push(0);
            }
            Value::U32(n) | Value::Float(n) => out.extend(n.to_le_bytes()),
            Value::U64(n) => out.extend(n.to_le_bytes()),
            Value::I64(n) => out.extend(n.to_le_bytes()),
        }
    }
    out.push(END);
}
