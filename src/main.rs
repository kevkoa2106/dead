use cargo_toml::Manifest;
use std::{collections::HashSet, env, fs};
use syn::visit::{self, Visit};
use walkdir::WalkDir;

struct UsageScanner {
    referenced_crates: HashSet<String>,
}

impl<'ast> Visit<'ast> for UsageScanner {
    // 1. Catches: use crate_name::something;
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        self.extract_from_use_tree(&i.tree);
        visit::visit_item_use(self, i);
    }

    // 2. Catches: crate_name::function();
    fn visit_path(&mut self, i: &'ast syn::Path) {
        if let Some(segment) = i.segments.first() {
            self.referenced_crates.insert(segment.ident.to_string());
        }
        visit::visit_path(self, i);
    }

    // 3. Catches: #[derive(CrateName)] or #[serde(...)]
    fn visit_attribute(&mut self, i: &'ast syn::Attribute) {
        if let Some(segment) = i.path().segments.first() {
            self.referenced_crates.insert(segment.ident.to_string());
        }
        visit::visit_attribute(self, i);
    }
}

impl UsageScanner {
    fn extract_from_use_tree(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                self.referenced_crates.insert(path.ident.to_string());
            }
            syn::UseTree::Name(name) => {
                self.referenced_crates.insert(name.ident.to_string());
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.extract_from_use_tree(item);
                }
            }
            syn::UseTree::Rename(rename) => {
                self.referenced_crates.insert(rename.ident.to_string());
            }
            syn::UseTree::Glob(_) => {}
        }
    }
}

fn main() {
    let current_dir = env::current_dir().unwrap();
    let manifest_path = current_dir.join("Cargo.toml");
    let src_path = current_dir.join("src");

    if !manifest_path.exists() {
        eprintln!("❌ No Cargo.toml found in {:?}", current_dir);
        return;
    }

    let manifest = Manifest::from_path(&manifest_path).unwrap();
    let declared_deps: HashSet<String> = manifest
        .dependencies
        .keys()
        .map(|k| k.replace("-", "_")) // MUST normalize hyphens
        .collect();

    //Identify Local Modules (To avoid false positives)
    let mut local_modules = HashSet::new();
    if let Ok(entries) = fs::read_dir(&src_path) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_stem().and_then(|s| s.to_str()) {
                local_modules.insert(name.to_string());
            }
        }
    }

    // Scan All Files
    let mut scanner = UsageScanner {
        referenced_crates: HashSet::new(),
    };
    for entry in WalkDir::new(&src_path).into_iter().flatten() {
        if entry.path().extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            if let Ok(file) = syn::parse_file(&content) {
                scanner.visit_file(&file);
            }
        }
    }

    //Comparison
    let ignored: HashSet<&str> = ["std", "core", "alloc", "crate", "self", "super"]
        .into_iter()
        .collect();

    println!("Checking dependencies for: {}\n", current_dir.display());

    let mut has_unused = false;
    for dep in &declared_deps {
        // A dep is unused if it's NOT in referenced_crates AND not a local module
        if !scanner.referenced_crates.contains(dep)
            && !local_modules.contains(dep)
            && !ignored.contains(dep.as_str())
        {
            println!("⚠️  Possibly Unused: {}", dep);
            has_unused = true;
        }
    }

    if !has_unused {
        println!("✅ Everything looks used!");
    }
}
