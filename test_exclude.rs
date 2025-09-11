use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;

fn main() {
    let mut builder = OverrideBuilder::new(".");
    builder.add("!.git").unwrap();
    builder.add("!*.git/**").unwrap();
    let overrides = builder.build().unwrap();
    
    let walker = WalkBuilder::new(".")
        .overrides(overrides)
        .build();
    
    for entry in walker {
        if let Ok(e) = entry {
            println!("{}", e.path().display());
        }
    }
}
