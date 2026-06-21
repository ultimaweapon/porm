use porm_parser::parse_for_build_script;

fn main() {
    parse_for_build_script("sql", |p| {
        p.file_stem()
            .unwrap()
            .to_str()
            .ok_or("file stem is not UTF-8")?
            .parse::<u32>()
            .map_err(|e| e.into())
    })
    .unwrap();
}
