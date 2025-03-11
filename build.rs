use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

static BRICK_COLORS: &[u8] = include_bytes!("colors/brick.json");
static CSS_COLORS: &[u8] = include_bytes!("colors/css.json");
static TAILWIND_COLORS: &[u8] = include_bytes!("colors/tailwind.json");

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
                    if let (Some(r), Some(g), Some(b)) = (arr[0].as_f64(), arr[1].as_f64(), arr[2].as_f64()) {
                        let name = format!("LOCK_{}", alphanumeric(&key).to_uppercase());
                        let value = &format!(
                            "LazyLock::new(|| rbx_types::Color3::new({} as f32, {} as f32, {} as f32));",
                            r / 255.0, g / 255.0, b / 255.0
                        );

                        statics += &format!("\nstatic {}: LazyLock<rbx_types::Color3> = {}", name, value);

                        phf_map.entry(key, &format!("&{}", name));
                    }
                }
            }
        }
    } else {
        panic!("Expected a JSON object");
    }

    return format!("{}\npub static {}: phf::Map<&'static str, &LazyLock<rbx_types::Color3>> = {};", statics, name, phf_map.build())
}

fn main() {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("colors.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    write!(
        &mut file,
        "use std::sync::LazyLock;\n{}\n\n{}\n\n{}",
        write_phf_for_colors("BRICK_COLORS", BRICK_COLORS),
        write_phf_for_colors("CSS_COLORS", CSS_COLORS),
        write_phf_for_colors("TAILWIND_COLORS", TAILWIND_COLORS),
    )
    .unwrap();
}