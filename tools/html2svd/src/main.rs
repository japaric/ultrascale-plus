use std::{env, fs, io, mem, ptr};

use html2svd::Bits;
use svd_parser::{
    bitrange::BitRangeType, encode::Encode, BitRange, Field, Peripheral, Register, RegisterCluster,
    RegisterInfo,
};
use xmltree::Element;

fn main() {
    let html_peripherals = fs::read_dir(env::args_os().nth(1).unwrap())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            let file = p.file_name().unwrap().to_str().unwrap();

            if file.starts_with("mod") {
                Some(html2svd::Peripheral::open(p).unwrap())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let peripherals = html_peripherals
        .iter()
        .flat_map(|p| {
            let mut first: Option<String> = None;

            p.instances.iter().filter_map(move |(name, address)| {
                // HACK we don't generate all peripherals because the generated crate would take too
                // long to compile and require too much RAM (12+ GB)
                const WHITELIST: &[&str] = &["GPIO", "IPI", "TTC0"];

                if !WHITELIST.contains(&&*name.to_uppercase()) {
                    return None;
                }

                // NOTE(unsafe) :-( no other way to construct a Peripheral
                let mut out: Peripheral = unsafe { mem::uninitialized() };

                let registers = if first.is_none() {
                    Some(
                        p.registers
                            .iter()
                            .filter_map(|r| {
                                let rem = r.width % 8;
                                let width = if rem == 0 {
                                    r.width
                                } else {
                                    r.width + 8 - rem
                                };

                                if r.width > 32 {
                                    // XXX ignore 64-bit registers for now
                                    return None;
                                }

                                // NOTE(unsafe) :-( no other way to construct a RegisterInfo
                                let mut info: RegisterInfo = unsafe { mem::uninitialized() };

                                unsafe {
                                    ptr::write(&mut info.name, r.name.clone());
                                    ptr::write(&mut info.alternate_group, None);
                                    ptr::write(&mut info.alternate_register, None);
                                    ptr::write(&mut info.derived_from, None);
                                    ptr::write(&mut info.description, r.description.clone());
                                    ptr::write(&mut info.address_offset, r.address);
                                    ptr::write(&mut info.size, Some(u32::from(width)));
                                    // TODO
                                    ptr::write(&mut info.access, None);
                                    ptr::write(&mut info.reset_value, Some(r.reset_value as u32));
                                    ptr::write(&mut info.reset_mask, None);
                                    let mut fields = vec![];
                                    for field in &r.bit_fields {
                                        // skip reserved fields and placeholders
                                        let fname = field.name.to_lowercase();
                                        if fname == "reserved" || fname == "_" {
                                            continue;
                                        }

                                        // NOTE(unsafe) :-( no other way to construct a RegisterInfo
                                        let mut out: Field = mem::uninitialized();

                                        ptr::write(&mut out.name, field.name.clone());
                                        ptr::write(
                                            &mut out.description,
                                            if field.description.trim().is_empty() {
                                                None
                                            } else {
                                                Some(field.description.clone())
                                            },
                                        );
                                        ptr::write(
                                            &mut out.bit_range,
                                            match &field.bits {
                                                Bits::Single(bit) => BitRange {
                                                    offset: u32::from(*bit),
                                                    width: 1,
                                                    range_type: BitRangeType::OffsetWidth,
                                                },
                                                Bits::Range(r) => BitRange {
                                                    offset: u32::from(*r.start()),
                                                    width: u32::from(r.end() - r.start() + 1),
                                                    range_type: BitRangeType::OffsetWidth,
                                                },
                                            },
                                        );
                                        // TODO
                                        ptr::write(&mut out.access, None);
                                        ptr::write(&mut out.enumerated_values, vec![]);
                                        ptr::write(&mut out.write_constraint, None);
                                        ptr::write(&mut out.modified_write_values, None);

                                        fields.push(out);
                                    }
                                    // FIXME
                                    // ptr::write(&mut info.fields, None);
                                    ptr::write(&mut info.fields, Some(fields));
                                    ptr::write(&mut info.write_constraint, None);
                                    ptr::write(&mut info.modified_write_values, None);
                                }

                                Some(RegisterCluster::Register(Register::Single(info)))
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                };

                unsafe {
                    ptr::write(&mut out.name, name.to_owned());
                    ptr::write(&mut out.version, None);
                    ptr::write(&mut out.display_name, None);
                    ptr::write(&mut out.group_name, None);
                    ptr::write(&mut out.description, None);
                    ptr::write(&mut out.base_address, *address);
                    ptr::write(&mut out.address_block, None);
                    ptr::write(&mut out.interrupt, vec![]);
                    ptr::write(&mut out.registers, registers);
                    ptr::write(&mut out.derived_from, first.as_ref().map(|s| s.to_owned()));
                }

                if first.is_none() {
                    first = Some(name.to_owned());
                }

                Some(out.encode().unwrap())
            })
        })
        .collect::<Vec<_>>();

    let mut children = vec![];
    children.push(Element {
        name: "name".to_owned(),
        text: Some("Ultrascale+".to_owned()),
        attributes: Default::default(),
        children: vec![],
    });
    children.push(Element {
        name: "peripherals".to_string(),
        children: peripherals,
        text: None,
        attributes: Default::default(),
    });
    let out = Element {
        name: "device".to_string(),
        children,
        attributes: Default::default(),
        text: None,
    };

    out.write(io::stdout());
}
