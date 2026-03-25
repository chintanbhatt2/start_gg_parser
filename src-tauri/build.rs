fn main() {
    cynic_codegen::register_schema("startgg")
        .from_sdl_file("schemas/startgg.graphql")
        .unwrap()
        .as_default()
        .unwrap();

    tauri_build::build()
}
