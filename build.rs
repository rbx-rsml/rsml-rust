use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const TAILWIND_COLORS: &[u8] = include_bytes!("colors/tailwind.json");
const SKIN_COLORS: &[u8] = include_bytes!("colors/skin.json");
const CSS_COLORS: &[u8] = include_bytes!("colors/css.json");
const BRICK_COLORS: &[u8] = include_bytes!("colors/brick.json");


fn alphanumeric(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
    .filter(|s| !s.is_empty())
    .collect()
}

fn write_phf_for_colors(name: &str, colors: &[u8]) -> String {
    let parsed_colors: serde_json::Value = serde_json::from_slice(colors).expect("Invalid JSON");

    let mut phf_map = phf_codegen::Map::<String>::new();
    let mut statics = "".to_owned();

    if let serde_json::Value::Object(map) = parsed_colors {
        for (key, value) in map {
            if let serde_json::Value::Array(arr) = value {
                if arr.len() == 3 {
                    let name = format!("LOCK_{}", alphanumeric(&key).to_uppercase());
                    let value = &format!(
                        "LazyLock::new(|| palette::Oklab::new({}f32, {}f32, {}f32));",
                        arr[0], arr[1], arr[2]
                    );

                    statics += &format!("\nstatic {}: LazyLock<palette::Oklab> = {}", name, value);

                    phf_map.entry(key, &format!("&{}", name));
                }
            }
        }
    } else {
        panic!("Expected a JSON object");
    }

    return format!("{}\npub static {}: phf::Map<&'static str, &LazyLock<palette::Oklab>> = {};", statics, name, phf_map.build())
}

fn main() {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("colors.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    write!(
        &mut file,
        "use std::sync::LazyLock;\n{}\n\n{}\n\n{}\n\n{}",
        write_phf_for_colors("TAILWIND_COLORS", TAILWIND_COLORS),
        write_phf_for_colors("SKIN_COLORS", SKIN_COLORS),
        write_phf_for_colors("CSS_COLORS", CSS_COLORS),
        write_phf_for_colors("BRICK_COLORS", BRICK_COLORS)
    )
    .unwrap();
}