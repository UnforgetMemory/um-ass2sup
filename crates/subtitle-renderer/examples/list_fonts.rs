use subtitle_renderer::font::registry::FontRegistry;

fn main() {
    let mut registry = FontRegistry::new();
    let count = registry.load_system_fonts();
    eprintln!("Loaded {count} system fonts");
    
    let families = registry.list_families();
    eprintln!("Families ({}):", families.len());
    for f in &families {
        eprintln!("  - {f}");
    }
}
