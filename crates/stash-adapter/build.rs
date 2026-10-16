fn main() {
    let schema = "../../schemas/stash-v0.31.1.graphql";
    println!("cargo:rerun-if-changed={schema}");
    cynic_codegen::register_schema("stash")
        .from_sdl_file(schema)
        .expect("pinned Stash schema must be readable")
        .as_default()
        .expect("pinned Stash schema must be valid SDL");
}
