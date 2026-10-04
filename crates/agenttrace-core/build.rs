fn main() {
    // rust-i18n embeds locales/*.yml at compile time; rebuild when they change.
    println!("cargo:rerun-if-changed=locales");
}
