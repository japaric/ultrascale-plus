use std::{collections::HashMap, fs, ops::RangeInclusive, path::Path, str::FromStr};

use select::{document::Document, predicate::Name};

// macro_rules! t {
//     ($e:expr) => {
//         $e.expect(&format!("{}: {}", line!(), stringify!($e)))
//     };
// }

macro_rules! t {
    ($e:expr) => {
        $e?
    };
}

#[derive(Debug)]
pub struct Peripheral {
    pub description: String,
    /// (Instance Name) -> Address
    pub instances: HashMap<String, u32>,
    pub registers: Vec<Register>,
}

impl Peripheral {
    pub fn open<P>(path: P) -> Result<Self, ()>
    where
        P: AsRef<Path>,
    {
        Self::open_(path.as_ref())
    }

    fn open_(path: &Path) -> Result<Self, ()> {
        let html = Document::from(&*t!(fs::read_to_string(path).map_err(drop)));

        let mut tables = html.find(Name("table"));
        let header = t!(tables.next().ok_or(()));
        let mut rows = header.find(Name("tr"));

        let description = t!(t!(rows.next().ok_or(())).find(Name("td")).next().ok_or(())).text();

        let row = t!(t!(rows.nth(1).ok_or(())).find(Name("td")).next().ok_or(())).text();
        let instances = row
            .split(')')
            .filter_map(|mut s| {
                s = s.trim();

                if s.is_empty() {
                    None
                } else {
                    let mut parts = s.split('(');
                    let address = u32::from_str_radix(
                        parts.next().unwrap().trim_start_matches("0x").trim_end(),
                        16,
                    )
                    .unwrap();
                    let name = parts.next().unwrap().to_owned();

                    Some((name, address))
                }
            })
            .collect();

        let table = t!(tables.next().ok_or(()));
        let root = t!(path.parent().ok_or(()));
        let registers = table
            .find(Name("tr"))
            .skip(1)
            .map(|row| {
                let link = t!(row.find(Name("a")).next().ok_or(()));
                let file = t!(t!(link.attr("onclick").ok_or(()))
                    .split('"')
                    .nth(1)
                    .ok_or(()));

                t!(fs::read_to_string(root.join(file)).map_err(drop))
                    .parse::<Register>()
                    .map_err(drop)
            })
            .collect::<Result<Vec<Register>, ()>>()?;

        Ok(Peripheral {
            description,
            instances,
            registers,
        })
    }
}

#[derive(Debug)]
pub struct Register {
    /// Register Name
    pub name: String,
    /// Relative Address
    pub address: u32,
    /// Width
    pub width: u8,
    /// Description
    pub description: String,
    /// Reset Value
    pub reset_value: u64,
    /// Detailed description
    pub detailed_description: Option<String>,
    pub bit_fields: Vec<BitField>,
}

impl FromStr for Register {
    type Err = ();

    fn from_str(html: &str) -> Result<Self, ()> {
        let doc = Document::from(html);
        let body = t!(doc.find(Name("body")).next().ok_or(()));
        let mut tables = body.find(Name("table"));
        let register = t!(tables.next().ok_or(()));
        let fields = t!(tables.next().ok_or(()));

        let mut name = Err(());
        let mut address = Err(());
        let mut width = Err(());
        let mut reset_value = Err(());
        let mut description = Err(());
        for row in register.find(Name("tr")) {
            let key = row.find(Name("th")).next().unwrap();
            let value = row.find(Name("td")).next().unwrap().text();

            match &*key.text() {
                "Register Name" => {
                    if name.is_ok() {
                        return Err(());
                    }

                    name = Ok(value);
                }
                "Relative Address" => {
                    if address.is_ok() {
                        return Err(());
                    }

                    address = Ok(t!(
                        u32::from_str_radix(value.trim_start_matches("0x"), 16).map_err(drop)
                    ));
                }
                "Width" => {
                    if width.is_ok() {
                        return Err(());
                    }

                    width = Ok(t!(value.trim().parse().map_err(drop)));
                }
                "Reset Value" => {
                    if reset_value.is_ok() {
                        return Err(());
                    }

                    reset_value = Ok(t!(
                        u64::from_str_radix(value.trim_start_matches("0x"), 16).map_err(drop)
                    ));
                }
                "Description" => {
                    if description.is_ok() {
                        return Err(());
                    }

                    description = Ok(value);
                }
                _ => {}
            }
        }

        let detailed_description = body
            .find(Name("p"))
            .filter(|n| n.attr("class").is_none())
            .next()
            .map(|n| n.text());

        let mut bit_fields = vec![];
        for row in fields.find(Name("tr")).skip(1 /* header */) {
            let mut columns = row.find(Name("td"));
            let name = t!(columns.next().ok_or(())).text();
            let text = t!(columns.next().ok_or(())).text();
            let bits = if text.contains(':') {
                let mut parts = text.splitn(2, ':');
                let end = t!(t!(parts.next().ok_or(())).trim().parse().map_err(drop));
                let start = t!(t!(parts.next().ok_or(())).trim().parse().map_err(drop));
                Bits::Range(start..=end)
            } else {
                Bits::Single(t!(text.trim().parse().map_err(drop)))
            };
            let type_ = t!(t!(columns.next().ok_or(())).text().parse());
            let reset_value = u32::from_str_radix(
                columns.next().ok_or(())?.text().trim_start_matches("0x"),
                16,
            )
            .map_err(drop)?;
            let description = columns.next().ok_or(())?.text();

            bit_fields.push(BitField {
                name,
                bits,
                type_,
                reset_value,
                description,
            })
        }

        Ok(Register {
            name: name?,
            address: address?,
            width: width?,
            reset_value: reset_value?,
            description: description?,
            detailed_description,
            bit_fields,
        })
    }
}

#[derive(Debug)]
pub struct BitField {
    /// Field Name
    pub name: String,
    /// Bits
    pub bits: Bits,
    /// Type
    pub type_: Type,
    /// Reset Value
    pub reset_value: u32,
    /// Description
    pub description: String,
}

#[derive(Debug)]
pub enum Bits {
    Single(u8),
    Range(RangeInclusive<u8>),
}

#[derive(Debug)]
pub enum Type {
    ReadAsZero,
    ReadOnly,
    ReadWrite,
    ReadWriteSetOnly,
    ReadableClearOnRead,
    ReadableClearOnWrite,
    WriteAsZero,
    WriteOnly,
    WriteToClear,
}

impl FromStr for Type {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "clronrd" => Type::ReadableClearOnRead,
            "clronwr" => Type::ReadableClearOnWrite,
            "raz" => Type::ReadAsZero,
            "ro" => Type::ReadOnly,
            "rw" => Type::ReadWrite,
            "rwso" => Type::ReadWriteSetOnly,
            "wo" => Type::WriteOnly,
            "waz" => Type::WriteAsZero,
            "wtc" => Type::WriteToClear,
            x => panic!("Unknown: {}", x),
        })
    }
}
