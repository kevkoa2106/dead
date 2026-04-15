use cargo_toml::{Dependency, Manifest};
use std::collections::HashSet;
use syn::{
    ItemUse, UseTree,
    visit::{self, Visit},
};

struct DependencyVisitor {
    found_crates: HashSet<String>,
}

impl<'ast> Visit<'ast> for DependencyVisitor {
    // This function is called whenever the parser encounters a 'use' statement
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.extract_crate_from_tree(&i.tree);
        // Continue walking the tree
        visit::visit_item_use(self, i);
    }
}

impl DependencyVisitor {
    fn extract_crate_from_tree(&mut self, tree: &UseTree) {
        match tree {
            UseTree::Path(path) => {
                // The first identifier in 'use serde::Deserialize' is 'serde'
                self.found_crates.insert(path.ident.to_string());
            }
            UseTree::Group(group) => {
                // Handles 'use {a, b}'
                for inner in &group.items {
                    self.extract_crate_from_tree(inner);
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let code = std::fs::read_to_string(file!()).unwrap();
    let toml = std::fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml");

    let manifest = Manifest::from_str(&toml).expect("Failed to parse TOML");

    let syntax_tree = syn::parse_file(code.as_str()).expect("Unable to parse file");
    let mut visitor = DependencyVisitor {
        found_crates: HashSet::new(),
    };

    visitor.visit_file(&syntax_tree);

    println!("Detected crates: {:?}", visitor.found_crates);

    for (name, dep) in manifest.dependencies {
        match dep {
            Dependency::Simple(version) => {
                println!("{}: version {}", name, version);
            }
            Dependency::Detailed(details) => {
                println!(
                    "{}: detailed (version: {:?}, path: {:?})",
                    name, details.version, details.features
                );
            }
            _ => println!("{}: other dependency type", name),
        }
    }
}
